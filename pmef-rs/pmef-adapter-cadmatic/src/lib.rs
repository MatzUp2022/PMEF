//! # pmef-adapter-cadmatic
//!
//! PMEF adapter for **CADMATIC Plant Design** — bidirectional translation
//! between CADMATIC's REST Web API and PMEF NDJSON packages.
//!
//! ## CADMATIC REST API overview
//!
//! CADMATIC exposes a Swagger-documented REST API (port 8080 by default):
//!
//! ```text
//! GET  /api/v1/projects                          — list projects
//! GET  /api/v1/projects/{projectId}/pipelines    — all piping lines
//! GET  /api/v1/pipelines/{lineId}/components     — components in a line
//! GET  /api/v1/equipment                         — all equipment
//! GET  /api/v1/equipment/{id}/connections        — nozzles + connections
//! GET  /api/v1/export/3ddx                       — full 3DDX geometry export
//! POST /api/v1/import/pmef                       — import PMEF objects
//! ```
//!
//! Authentication: HTTP Basic or Bearer token (configured via [`CadmaticConfig`]).
//!
//! ## Usage
//!
//! ```rust,no_run
//! use pmef_adapter_cadmatic::{CadmaticAdapter, CadmaticConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = CadmaticConfig::builder()
//!         .base_url("http://cadmatic-server:8080")
//!         .project_id("EAF_2026")
//!         .bearer_token("my-api-token")
//!         .build()?;
//!
//!     let adapter = CadmaticAdapter::new(config).await?;
//!     let stats = adapter.export_to_pmef("output.ndjson").await?;
//!     println!("Exported {} objects", stats.objects_ok);
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod api;
pub mod client;
pub mod config;
pub mod mapping;

pub use api::{
    CadmaticComponent, CadmaticConnection, CadmaticEquipment,
    CadmaticLine, CadmaticNozzle, CadmaticProject,
};
pub use client::CadmaticClient;
pub use config::{CadmaticConfig, CadmaticConfigBuilder};
pub use mapping::{
    component_class_map, equipment_class_map, material_map,
    CadmaticFieldMapper, MappingStats,
};

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};

/// The CADMATIC → PMEF adapter.
pub struct CadmaticAdapter {
    client: CadmaticClient,
    config: CadmaticConfig,
    mapper: CadmaticFieldMapper,
}

impl CadmaticAdapter {
    /// Create a new adapter, establishing and testing the API connection.
    pub async fn new(config: CadmaticConfig) -> Result<Self, AdapterError> {
        let client = CadmaticClient::new(config.clone())
            .map_err(|e| AdapterError::Connection(e.to_string()))?;
        // Test connectivity
        client.ping().await
            .map_err(|e| AdapterError::Connection(
                format!("Cannot reach CADMATIC at {}: {e}", config.base_url)
            ))?;
        Ok(Self {
            client,
            config: config.clone(),
            mapper: CadmaticFieldMapper::new(config.project_code.clone()),
        })
    }

    /// Export the full CADMATIC plant model to a PMEF NDJSON file.
    pub async fn export_to_pmef(
        &self,
        output_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        use std::fs::File;
        use std::io::BufWriter;

        let file = File::create(output_path)
            .map_err(|e| AdapterError::Io(e))?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());
        let mut stats = AdapterStats::default();
        let t0 = std::time::Instant::now();

        tracing::info!(
            "Starting CADMATIC export: project='{}' → {}",
            self.config.project_id, output_path
        );

        // 1. FileHeader + Plant + Unit
        let header = self.mapper.make_file_header(&self.config.project_id);
        writer.write_value(&header).map_err(|e| AdapterError::Json(e.into()))?;

        let plant = self.mapper.make_plant(&self.config.project_id);
        writer.write_value(&plant).map_err(|e| AdapterError::Json(e.into()))?;

        let unit = self.mapper.make_unit(&self.config.project_id);
        writer.write_value(&unit).map_err(|e| AdapterError::Json(e.into()))?;
        stats.objects_ok += 3;

        // 2. Equipment
        tracing::info!("Fetching equipment...");
        match self.client.get_equipment().await {
            Ok(equipment_list) => {
                for equip in &equipment_list {
                    match self.mapper.map_equipment(equip) {
                        Ok(pmef_obj) => {
                            writer.write_value(&pmef_obj)
                                .map_err(|e| AdapterError::Json(e.into()))?;
                            // Write HasEquivalentIn
                            let equiv = self.mapper.make_has_equivalent_in(
                                &pmef_obj["@id"].as_str().unwrap_or(""),
                                &equip.object_guid,
                            );
                            writer.write_value(&equiv)
                                .map_err(|e| AdapterError::Json(e.into()))?;
                            stats.objects_ok += 2;
                        }
                        Err(e) => {
                            tracing::warn!("Equipment mapping failed for {}: {e}", equip.tag_number);
                            stats.objects_failed += 1;
                        }
                    }
                }
                tracing::info!("Equipment: {} objects", equipment_list.len());
            }
            Err(e) => {
                tracing::warn!("Could not fetch equipment: {e}");
                stats.objects_skipped += 1;
            }
        }

        // 3. Piping lines and components
        tracing::info!("Fetching piping lines...");
        match self.client.get_pipelines(&self.config.project_id).await {
            Ok(lines) => {
                for line in &lines {
                    // Map the line itself
                    match self.mapper.map_pipeline(line) {
                        Ok(pmef_line) => {
                            writer.write_value(&pmef_line)
                                .map_err(|e| AdapterError::Json(e.into()))?;
                            stats.objects_ok += 1;
                        }
                        Err(e) => {
                            tracing::warn!("Line mapping failed for {}: {e}", line.line_number);
                            stats.objects_failed += 1;
                            continue;
                        }
                    }

                    // Fetch and map components
                    match self.client.get_components(&line.line_id).await {
                        Ok(components) => {
                            let (seg, comp_objs, rels) = self.mapper.map_segment_and_components(
                                line, &components,
                            );
                            writer.write_value(&seg)
                                .map_err(|e| AdapterError::Json(e.into()))?;
                            stats.objects_ok += 1;
                            for obj in &comp_objs {
                                writer.write_value(obj)
                                    .map_err(|e| AdapterError::Json(e.into()))?;
                                stats.objects_ok += 1;
                            }
                            for rel in &rels {
                                writer.write_value(rel)
                                    .map_err(|e| AdapterError::Json(e.into()))?;
                                stats.objects_ok += 1;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Could not fetch components for line {}: {e}", line.line_number);
                            stats.objects_skipped += 1;
                        }
                    }
                }
                tracing::info!("Piping: {} lines processed", lines.len());
            }
            Err(e) => {
                tracing::warn!("Could not fetch pipelines: {e}");
                stats.objects_skipped += 1;
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;

        tracing::info!(
            "Export complete: {} ok, {} failed, {} skipped in {}ms",
            stats.objects_ok, stats.objects_failed, stats.objects_skipped, stats.duration_ms
        );
        Ok(stats)
    }

    /// Import a PMEF NDJSON file into CADMATIC (Level 2).
    ///
    /// Uses the `POST /api/v1/import/pmef` endpoint. Only objects whose
    /// `HasEquivalentIn.targetSystem = "CADMATIC"` are updated in-place;
    /// new objects are created.
    pub async fn import_from_pmef(
        &self,
        input_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonReader, PmefPackageIndex, ReaderConfig};
        use std::fs::File;
        use std::io::BufReader;

        let file = File::open(input_path).map_err(AdapterError::Io)?;
        let reader = NdjsonReader::new(BufReader::new(file), ReaderConfig::default());
        let objects: Vec<_> = reader
            .collect::<Result<_, _>>()
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        let idx = PmefPackageIndex::from_iter(objects.into_iter());
        let mut stats = AdapterStats::default();

        // Find all HasEquivalentIn → CADMATIC relationships
        let equiv_rels = idx.by_type("pmef:HasEquivalentIn");
        let mut cadmatic_ids: std::collections::HashMap<String, String> = Default::default();

        for rel in &equiv_rels {
            if rel.value.get("targetSystem")
                .and_then(|v| v.as_str())
                == Some("CADMATIC")
            {
                if let (Some(src), Some(tgt)) = (
                    rel.value.get("sourceId").and_then(|v| v.as_str()),
                    rel.value.get("targetSystemId").and_then(|v| v.as_str()),
                ) {
                    cadmatic_ids.insert(src.to_owned(), tgt.to_owned());
                }
            }
        }

        tracing::info!(
            "Import: {} CADMATIC identity mappings found in package",
            cadmatic_ids.len()
        );

        // Post the full package via the bulk import endpoint
        let all_objects: Vec<serde_json::Value> = idx
            .objects
            .values()
            .map(|o| o.value.clone())
            .collect();

        match self.client.post_import_pmef(&all_objects).await {
            Ok(response) => {
                stats.objects_ok = response.get("created")
                    .and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                stats.objects_failed = response.get("failed")
                    .and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                tracing::info!("Import response: {:?}", response);
            }
            Err(e) => {
                return Err(AdapterError::Other(format!("Import API call failed: {e}")));
            }
        }

        Ok(stats)
    }
}

impl PmefAdapter for CadmaticAdapter {
    fn name(&self) -> &str { "pmef-adapter-cadmatic" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "CADMATIC" }
    fn supported_domains(&self) -> &[&str] { &["piping", "equipment"] }
    fn conformance_level(&self) -> u8 { 2 }
    fn description(&self) -> &str {
        "CADMATIC Plant Design → PMEF adapter. Uses the CADMATIC REST Web API \
         for semantic data and 3DDX export for geometry. Level 2 conformance."
    }
}
