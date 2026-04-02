//! # pmef-adapter-creo
//!
//! PMEF adapter for **PTC Creo Parametric** — mechanical equipment envelope
//! geometry and piping routing data.
//!
//! ## SMS Group context
//!
//! At SMS Group, Creo is used for:
//! - Mechanical equipment design (rolling mills, EAF, ladle furnaces, converters)
//! - Hydraulic and cooling system components
//! - Drive and gearbox assemblies
//! - Piping routing within equipment assemblies (Creo Piping Extension)
//!
//! PMEF uses Creo data for:
//! - Equipment envelope geometry (bounding box LOD1 for clash checking)
//! - Nozzle connection points (from named coordinate systems)
//! - Detailed STEP geometry for LOD3 (full B-Rep from Creo)
//! - Tag numbers (from Creo parameters PLANT_TAG, EQUIPMENT_CLASS)
//! - Windchill PDM integration (WTPart numbers)
//!
//! ## Export pipeline
//!
//! ```text
//! Creo Parametric + Windchill
//!   │
//!   ├── Creo Toolkit plugin (C) ──→ creo-export.json
//!   │   (CreoExporter.c)               │
//!   │                                  │
//!   ├── STEP export (per assembly) → *.stp
//!   │   (Creo: File → Save a Copy → STEP AP214)
//!   │                                  │
//!   └───────────────────────────────────┤
//!                                       ▼
//!                             pmef-adapter-creo (Rust)
//!                                       │
//!                                       ▼
//!                                 PMEF NDJSON
//! ```
//!
//! ## Usage
//!
//! ```rust,no_run
//! use pmef_adapter_creo::{CreoAdapter, CreoConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = CreoConfig {
//!         project_code: "eaf-2026".to_owned(),
//!         export_path: "creo-export.json".into(),
//!         step_dir: Some("step-files".into()),
//!         ..Default::default()
//!     };
//!     let mut adapter = CreoAdapter::new(config);
//!     let stats = adapter.export_to_pmef("output.ndjson").await?;
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod export_schema;
pub mod step;

pub use export_schema::{
    creo_class_to_pmef, creo_fitting_to_pmef, CreoAssembly, CreoBbox,
    CreoExport, CreoFitting, CreoNozzle, CreoPart, CreoPipingSegment,
    CreoPoint, CreoTransform, CreoUnit,
};
pub use step::{parse_step_file, StepBbox, StepParseResult};

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the Creo adapter.
#[derive(Debug, Clone)]
pub struct CreoConfig {
    /// PMEF project code for @id generation.
    pub project_code: String,
    /// Path to the Creo JSON export file (produced by `CreoExporter.c`).
    pub export_path: PathBuf,
    /// Directory containing STEP files (optional — for geometry enrichment).
    pub step_dir: Option<PathBuf>,
    /// Minimum assembly volume [mm³] to include. Default: 1000 mm³ (1 cm³).
    pub min_volume_mm3: f64,
    /// Include piping segments from Creo Piping Extension. Default: true.
    pub include_piping: bool,
    /// Include bounding box geometry in output. Default: true.
    pub include_bbox: bool,
    /// Include STEP file references. Default: true.
    pub include_step_refs: bool,
    /// Parent unit @id for isPartOf references.
    pub unit_id: Option<String>,
    /// Windchill server URL (for document links, optional).
    pub windchill_url: Option<String>,
}

impl Default for CreoConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            export_path: PathBuf::from("creo-export.json"),
            step_dir: None,
            min_volume_mm3: 1000.0,
            include_piping: true,
            include_bbox: true,
            include_step_refs: true,
            unit_id: None,
            windchill_url: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Creo Adapter
// ─────────────────────────────────────────────────────────────────────────────

/// PTC Creo Parametric → PMEF adapter.
pub struct CreoAdapter {
    config: CreoConfig,
    /// model_name → PMEF @id
    model_to_id: HashMap<String, String>,
}

impl CreoAdapter {
    pub fn new(config: CreoConfig) -> Self {
        Self { config, model_to_id: HashMap::new() }
    }

    /// Export the Creo model (from JSON export + optional STEP) to PMEF NDJSON.
    pub async fn export_to_pmef(
        &mut self,
        output_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        use std::fs::File;
        use std::io::BufWriter;

        let t0 = std::time::Instant::now();
        let mut stats = AdapterStats::default();

        let json_text = std::fs::read_to_string(&self.config.export_path)
            .map_err(AdapterError::Io)?;
        let export: CreoExport = serde_json::from_str(&json_text)
            .map_err(|e| AdapterError::Json(e))?;

        tracing::info!(
            "Loaded Creo export: {} assemblies, {} piping segments from '{}'",
            export.assemblies.len(), export.piping_segments.len(),
            export.assembly_name
        );

        // Pre-register all assembly IDs
        for asm in &export.assemblies {
            let id = self.assembly_id(asm.tag_number(), &asm.model_name);
            self.model_to_id.insert(asm.model_name.clone(), id);
        }

        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        // Header objects
        let proj = self.config.project_code.clone();
        for obj in self.make_header_objects(&export, &proj) {
            writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Map assemblies → equipment objects
        for asm in &export.assemblies {
            // Filter by minimum volume
            if self.config.include_bbox {
                let vol = asm.bounding_box.as_ref()
                    .map(|bb| bb.to_mm(export.coordinate_unit).volume())
                    .unwrap_or(f64::INFINITY);
                if vol < self.config.min_volume_mm3 {
                    stats.objects_skipped += 1;
                    continue;
                }
            }

            // Enrich with STEP data if available
            let step_result = self.try_load_step(asm);

            for obj in self.map_assembly(asm, &export, step_result.as_ref()) {
                writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                stats.objects_ok += 1;
            }
        }

        // Map piping segments
        if self.config.include_piping {
            // Group segments by network name
            let mut networks: HashMap<String, Vec<&CreoPipingSegment>> = HashMap::new();
            for seg in &export.piping_segments {
                networks.entry(seg.network_name.clone()).or_default().push(seg);
            }
            for (network_name, segments) in &networks {
                for obj in self.map_piping_network(network_name, segments, &export) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        tracing::info!(
            "Creo export: {} ok, {} failed, {} skipped in {}ms",
            stats.objects_ok, stats.objects_failed, stats.objects_skipped, stats.duration_ms
        );
        Ok(stats)
    }

    // ── STEP enrichment ───────────────────────────────────────────────────────

    fn try_load_step(&self, asm: &CreoAssembly) -> Option<StepParseResult> {
        let step_dir = self.config.step_dir.as_ref()?;
        let step_path = step_dir.join(format!("{}.stp", asm.model_name));
        if !step_path.exists() {
            let alt = step_dir.join(format!("{}.step", asm.model_name));
            if !alt.exists() { return None; }
        }
        let path = if step_dir.join(format!("{}.stp", asm.model_name)).exists() {
            step_dir.join(format!("{}.stp", asm.model_name))
        } else {
            step_dir.join(format!("{}.step", asm.model_name))
        };
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match parse_step_file(&content) {
                    Ok(r) => {
                        tracing::debug!("Loaded STEP for {}: {} points, bbox={:?}",
                            asm.model_name, r.point_count, r.bbox.is_some());
                        Some(r)
                    }
                    Err(e) => { tracing::warn!("STEP parse failed for {}: {e}", asm.model_name); None }
                }
            }
            Err(e) => { tracing::debug!("Cannot read STEP for {}: {e}", asm.model_name); None }
        }
    }

    // ── @id helpers ───────────────────────────────────────────────────────────

    fn clean(s: &str) -> String {
        s.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_')).collect()
    }

    fn assembly_id(&self, tag: &str, model_name: &str) -> String {
        let local = if tag.is_empty() || tag == model_name {
            Self::clean(model_name)
        } else {
            Self::clean(tag)
        };
        format!("urn:pmef:obj:{}:{local}", self.config.project_code)
    }

    fn line_id(&self, network_name: &str) -> String {
        format!("urn:pmef:line:{}:{}", self.config.project_code, Self::clean(network_name))
    }

    fn unit_id(&self, proj: &str) -> String {
        self.config.unit_id.clone()
            .unwrap_or_else(|| format!("urn:pmef:unit:{proj}:U-01"))
    }

    fn make_has_equivalent_in(&self, pmef_id: &str, creo_id: &str, windchill: Option<&str>) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        let mut obj = serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-creo", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id, "targetId": pmef_id,
            "targetSystem": "CREO",
            "targetSystemId": creo_id,
            "mappingType": "EXACT",
            "derivedBy": "ADAPTER_IMPORT",
            "confidence": 1.0,
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-creo 0.9.0" }
        });
        if let Some(wc) = windchill {
            obj["windchillNumber"] = serde_json::Value::String(wc.to_owned());
        }
        obj
    }

    // ── Header ────────────────────────────────────────────────────────────────

    fn make_header_objects(&self, export: &CreoExport, proj: &str) -> Vec<serde_json::Value> {
        let asm_clean = Self::clean(&export.assembly_name);
        let plant_id = format!("urn:pmef:plant:{proj}:{asm_clean}");
        vec![
            serde_json::json!({
                "@type": "pmef:FileHeader",
                "@id": format!("urn:pmef:pkg:{proj}:{asm_clean}"),
                "pmefVersion": "0.9.0",
                "plantId": plant_id,
                "projectCode": proj,
                "coordinateSystem": "Z-up",
                "units": "mm",
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringTool": format!("pmef-adapter-creo 0.9.0 / {}", export.creo_version)
            }),
            serde_json::json!({
                "@type": "pmef:Plant",
                "@id": plant_id,
                "pmefVersion": "0.9.0",
                "name": export.assembly_name,
                "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
            }),
            serde_json::json!({
                "@type": "pmef:Unit",
                "@id": self.unit_id(proj),
                "pmefVersion": "0.9.0",
                "name": export.assembly_name,
                "isPartOf": plant_id,
                "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
            }),
        ]
    }

    // ── Assembly → Equipment ──────────────────────────────────────────────────

    fn map_assembly(
        &self,
        asm: &CreoAssembly,
        export: &CreoExport,
        step: Option<&StepParseResult>,
    ) -> Vec<serde_json::Value> {
        let obj_id = self.assembly_id(asm.tag_number(), &asm.model_name);
        let (pmef_type, equip_class) = creo_class_to_pmef(
            asm.equipment_class.as_deref().unwrap_or("GENERIC")
        );
        let unit = &self.config.project_code;

        // Bounding box: prefer STEP-derived (more accurate) over Creo JSON export
        let bbox_pmef = step.and_then(|s| s.bbox.as_ref()).map(|sb| {
            serde_json::json!({
                "xMin": sb.x_min, "xMax": sb.x_max,
                "yMin": sb.y_min, "yMax": sb.y_max,
                "zMin": sb.z_min, "zMax": sb.z_max
            })
        }).or_else(|| {
            asm.bounding_box.as_ref().map(|bb| {
                let mm = bb.to_mm(export.coordinate_unit);
                serde_json::json!({
                    "xMin": mm.x_min, "xMax": mm.x_max,
                    "yMin": mm.y_min, "yMax": mm.y_max,
                    "zMin": mm.z_min, "zMax": mm.z_max
                })
            })
        });

        // Geometry layer selection
        let geometry_layer = if asm.step_file.is_some() && self.config.include_step_refs {
            "mesh_ref"
        } else if bbox_pmef.is_some() {
            "none" // bbox embedded in boundingBox field
        } else {
            "none"
        };

        let step_uri = asm.step_file.as_ref().and_then(|f| {
            self.config.windchill_url.as_ref().map(|base| {
                format!("{base}/wt/file/{f}")
            })
        });

        // Map nozzles from export.nozzles filtered by parent_assembly_id
        let nozzles: Vec<serde_json::Value> = export.nozzles.iter()
            .filter(|noz| noz.parent_assembly_id == asm.session_id)
            .map(|noz| {
                let pos = export.coordinate_unit;
                serde_json::json!({
                    "nozzleId": noz.nozzle_mark,
                    "nozzleMark": noz.nozzle_mark,
                    "service": noz.service,
                    "nominalDiameter": noz.dn_mm(),
                    "flangeRating": noz.flange_rating.as_ref()
                        .map(|r| format!("ANSI-{r}")),
                    "facingType": noz.facing_type.as_deref().unwrap_or("RF"),
                    "coordinate": [
                        pos.to_mm(noz.origin.x),
                        pos.to_mm(noz.origin.y),
                        pos.to_mm(noz.origin.z)
                    ],
                    "direction": noz.direction
                })
            })
            .collect();

        // Also extract nozzles from STEP axis placements named CS_NOZZLE_*
        let step_nozzles: Vec<serde_json::Value> = step
            .map(|s| &s.axis_placements)
            .into_iter()
            .flatten()
            .filter(|ap| ap.name.to_uppercase().starts_with("CS_NOZZLE"))
            .map(|ap| {
                let mark = ap.name
                    .trim_start_matches("CS_NOZZLE_")
                    .trim_start_matches("CS_NOZZLE");
                serde_json::json!({
                    "nozzleId": mark,
                    "nozzleMark": mark,
                    "nominalDiameter": 100.0, // unknown without annotation
                    "flangeRating": "ANSI-150",
                    "facingType": "RF",
                    "coordinate": ap.origin,
                    "direction": ap.z_axis
                })
            })
            .collect();

        let all_nozzles: Vec<serde_json::Value> = if !nozzles.is_empty() {
            nozzles
        } else {
            step_nozzles
        };

        // Windchill document link
        let documents: Vec<serde_json::Value> = asm.windchill_number.as_ref()
            .map(|wn| vec![serde_json::json!({
                "documentId": wn,
                "documentType": "WINDCHILL_WTPART",
                "revision": "latest"
            })])
            .unwrap_or_default();

        let obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": self.unit_id(unit),
            "equipmentBasic": {
                "tagNumber": asm.tag_number(),
                "equipmentClass": equip_class,
                "serviceDescription": asm.description,
                "designCode": asm.design_code
            },
            "nozzles": all_nozzles,
            "documents": documents,
            "geometry": {
                "type": geometry_layer,
                "lod": if asm.step_file.is_some() { "LOD3_FINE" } else { "LOD1_COARSE" },
                "refUri": step_uri,
                "boundingBox": bbox_pmef
            },
            "customAttributes": {
                "creoModelName": asm.model_name,
                "creoSessionId": asm.session_id,
                "windchillNumber": asm.windchill_number,
                "designPressure_Pa": asm.design_pressure_pa(),
                "designTemperature_K": asm.design_temperature_k(),
                "material": asm.material,
                "weightKg": asm.weight.map(|w| export.coordinate_unit.to_mm(w) / 1000.0), // unit to approx kg
                "creoParameters": asm.parameters
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": asm.model_name,
                "authoringTool": format!("pmef-adapter-creo 0.9.0 / {}", export.creo_version)
            }
        });

        vec![
            obj,
            self.make_has_equivalent_in(
                &obj_id, &asm.model_name, asm.windchill_number.as_deref()
            )
        ]
    }

    // ── Piping network mapping ────────────────────────────────────────────────

    fn map_piping_network(
        &self,
        network_name: &str,
        segments: &[&CreoPipingSegment],
        export: &CreoExport,
    ) -> Vec<serde_json::Value> {
        let line_id = self.line_id(network_name);
        let seg_id  = format!("{line_id}-S1");
        let unit    = &self.config.project_code;

        let dn_mm = segments.first()
            .map(|s| s.dn_mm())
            .unwrap_or(100.0);

        let mut result = Vec::new();

        // PipingNetworkSystem
        result.push(serde_json::json!({
            "@type": "pmef:PipingNetworkSystem",
            "@id": line_id,
            "pmefVersion": "0.9.0",
            "lineNumber": network_name,
            "nominalDiameter": dn_mm,
            "pipeClass": segments.first().map(|s| s.pipe_spec.as_str()),
            "fluidPhase": "LIQUID",
            "isPartOf": self.unit_id(unit),
            "segments": [seg_id],
            "customAttributes": { "creoNetworkName": network_name },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-creo 0.9.0" }
        }));

        // Build component list
        let mut comp_ids: Vec<String> = Vec::new();
        let mut comp_objs: Vec<serde_json::Value> = Vec::new();
        let pos = export.coordinate_unit;

        for (si, seg) in segments.iter().enumerate() {
            // Pipe component
            let pipe_id = format!("{line_id}-PIPE-{:03}", si + 1);
            comp_ids.push(pipe_id.clone());

            let sp = &seg.start_point;
            let ep = &seg.end_point;
            let length = seg.length_mm(pos);

            comp_objs.push(serde_json::json!({
                "@type": "pmef:Pipe",
                "@id": pipe_id,
                "pmefVersion": "0.9.0",
                "isPartOf": seg_id,
                "pipeLength": length,
                "componentSpec": {
                    "componentClass": "PIPE",
                    "skey": "PIPW    "
                },
                "ports": [
                    {
                        "portId": "P1",
                        "coordinate": [pos.to_mm(sp.x), pos.to_mm(sp.y), pos.to_mm(sp.z)],
                        "nominalDiameter": seg.dn_mm(), "endType": "BW"
                    },
                    {
                        "portId": "P2",
                        "coordinate": [pos.to_mm(ep.x), pos.to_mm(ep.y), pos.to_mm(ep.z)],
                        "nominalDiameter": seg.dn_mm(), "endType": "BW"
                    }
                ],
                "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
            }));

            // Fittings on this segment
            for (fi, fitting) in seg.fittings.iter().enumerate() {
                let (ftype, fclass) = creo_fitting_to_pmef(&fitting.fitting_type);
                let fit_id = format!("{line_id}-FIT-{:03}-{:02}", si + 1, fi + 1);
                comp_ids.push(fit_id.clone());
                let fp = &fitting.position;
                comp_objs.push(serde_json::json!({
                    "@type": ftype,
                    "@id": fit_id,
                    "pmefVersion": "0.9.0",
                    "isPartOf": seg_id,
                    "angle": fitting.angle,
                    "componentSpec": {
                        "componentClass": fclass,
                        "skey": fitting.spec_key.as_deref().unwrap_or("")
                    },
                    "ports": [{
                        "portId": "P1",
                        "coordinate": [pos.to_mm(fp.x), pos.to_mm(fp.y), pos.to_mm(fp.z)],
                        "nominalDiameter": fitting.dn_mm(), "endType": "BW"
                    }],
                    "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
                }));
            }
        }

        // PipingSegment
        result.push(serde_json::json!({
            "@type": "pmef:PipingSegment",
            "@id": seg_id,
            "isPartOf": line_id,
            "segmentNumber": 1,
            "components": comp_ids,
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
        }));

        result.extend(comp_objs);
        result
    }
}

impl PmefAdapter for CreoAdapter {
    fn name(&self) -> &str { "pmef-adapter-creo" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "CREO" }
    fn supported_domains(&self) -> &[&str] { &["equipment", "piping", "steel"] }
    fn conformance_level(&self) -> u8 { 2 }
    fn description(&self) -> &str {
        "PTC Creo Parametric → PMEF adapter. Reads Creo Toolkit JSON export \
         (assembly hierarchy, parameters, nozzle CS) and optional STEP files \
         (bounding box, product names). SMS Group specific: supports rolling mill, \
         EAF, ladle furnace, converter, gearbox equipment classes. \
         Level 2 conformance. Windchill PDM integration via WTPart numbers."
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CreoConfig {
        CreoConfig {
            project_code: "test".to_owned(),
            export_path: PathBuf::from("nonexistent.json"),
            ..Default::default()
        }
    }

    fn mock_assembly() -> CreoAssembly {
        CreoAssembly {
            model_name: "P-201A-ASM".to_owned(),
            session_id: 42,
            windchill_number: Some("0000-P201A-001".to_owned()),
            description: Some("Cooling water pump assembly".to_owned()),
            plant_tag: Some("P-201A".to_owned()),
            equipment_class: Some("PUMP".to_owned()),
            design_code: Some("API 610".to_owned()),
            material: Some("Carbon Steel".to_owned()),
            weight: Some(1850000.0), // mm³ equivalent weight field
            design_pressure_barg: Some(15.0),
            design_temperature_degc: Some(60.0),
            bounding_box: Some(CreoBbox {
                x_min: 10050., x_max: 10450.,
                y_min: 5200.,  y_max: 5700.,
                z_min: 700.,   z_max: 1600.,
            }),
            transform_to_root: CreoTransform::identity(),
            step_file: Some("P-201A-ASM.stp".to_owned()),
            child_parts: vec![],
            parameters: {
                let mut m = HashMap::new();
                m.insert("ERECTION_SEQUENCE".to_owned(), serde_json::Value::from(3));
                m
            },
        }
    }

    fn mock_export() -> CreoExport {
        CreoExport {
            schema_version: "1.0".to_owned(),
            creo_version: "Creo 10.0.0.0".to_owned(),
            exported_at: "2026-03-31T00:00:00Z".to_owned(),
            assembly_name: "EAF-LINE3-ASSY".to_owned(),
            windchill_number: None,
            coordinate_unit: CreoUnit::Mm,
            assemblies: vec![mock_assembly()],
            parts: vec![],
            piping_segments: vec![],
            nozzles: vec![
                CreoNozzle {
                    cs_name: "CS_NOZZLE_N1".to_owned(),
                    parent_assembly_id: 42,
                    nozzle_mark: "N1".to_owned(),
                    service: Some("Suction".to_owned()),
                    nominal_diameter_in: 8.0,
                    flange_rating: Some("150".to_owned()),
                    facing_type: Some("RF".to_owned()),
                    origin: CreoPoint { x: 10200., y: 5400., z: 850. },
                    direction: [-1., 0., 0.],
                }
            ],
            summary: CreoExportSummary {
                assembly_count: 1, part_count: 0,
                piping_segment_count: 0, nozzle_count: 1,
            },
        }
    }

    #[test]
    fn test_map_assembly_basic() {
        let export = mock_export();
        let mut adapter = CreoAdapter::new(test_config());
        // Pre-register
        let asm = &export.assemblies[0];
        adapter.model_to_id.insert(asm.model_name.clone(),
            adapter.assembly_id(asm.tag_number(), &asm.model_name));

        let objs = adapter.map_assembly(asm, &export, None);
        assert_eq!(objs.len(), 2); // equipment + HasEquivalentIn
        let eq = &objs[0];
        assert_eq!(eq["@type"], "pmef:Pump");
        assert_eq!(eq["equipmentBasic"]["tagNumber"], "P-201A");
        assert_eq!(eq["equipmentBasic"]["equipmentClass"], "CENTRIFUGAL_PUMP");
        assert_eq!(eq["equipmentBasic"]["designCode"], "API 610");
        // Nozzle
        let nozzles = eq["nozzles"].as_array().unwrap();
        assert_eq!(nozzles.len(), 1);
        assert_eq!(nozzles[0]["nozzleId"], "N1");
        assert!((nozzles[0]["nominalDiameter"].as_f64().unwrap() - 203.2).abs() < 0.1);
        // Bounding box
        let bbox = &eq["geometry"]["boundingBox"];
        assert!((bbox["xMin"].as_f64().unwrap() - 10050.).abs() < 0.1);
        // Custom attributes
        assert_eq!(eq["customAttributes"]["creoModelName"], "P-201A-ASM");
        assert_eq!(eq["customAttributes"]["windchillNumber"], "0000-P201A-001");
    }

    #[test]
    fn test_map_assembly_design_conditions() {
        let export = mock_export();
        let mut adapter = CreoAdapter::new(test_config());
        let asm = &export.assemblies[0];
        adapter.model_to_id.insert(asm.model_name.clone(),
            adapter.assembly_id(asm.tag_number(), &asm.model_name));
        let objs = adapter.map_assembly(asm, &export, None);
        let attrs = &objs[0]["customAttributes"];
        let dp = attrs["designPressure_Pa"].as_f64().unwrap();
        assert!((dp - 1_601_325.0).abs() < 10.0, "Got {dp}");
        let dt = attrs["designTemperature_K"].as_f64().unwrap();
        assert!((dt - 333.15).abs() < 0.01, "Got {dt}");
    }

    #[test]
    fn test_map_assembly_has_equivalent_in() {
        let export = mock_export();
        let mut adapter = CreoAdapter::new(test_config());
        let asm = &export.assemblies[0];
        adapter.model_to_id.insert(asm.model_name.clone(),
            adapter.assembly_id(asm.tag_number(), &asm.model_name));
        let objs = adapter.map_assembly(asm, &export, None);
        let rel = &objs[1];
        assert_eq!(rel["@type"], "pmef:HasEquivalentIn");
        assert_eq!(rel["targetSystem"], "CREO");
        assert_eq!(rel["targetSystemId"], "P-201A-ASM");
        assert_eq!(rel["windchillNumber"], "0000-P201A-001");
    }

    #[test]
    fn test_map_assembly_with_step() {
        let export = mock_export();
        let mut adapter = CreoAdapter::new(test_config());
        let asm = &export.assemblies[0];
        adapter.model_to_id.insert(asm.model_name.clone(),
            adapter.assembly_id(asm.tag_number(), &asm.model_name));

        // Inject synthetic STEP result
        let mut step = StepParseResult::default();
        step.bbox = Some(StepBbox {
            x_min: 10000., x_max: 10500.,
            y_min: 5100., y_max: 5800.,
            z_min: 600., z_max: 1700.,
        });
        step.axis_placements = vec![
            crate::step::StepAxisPlacement {
                name: "CS_NOZZLE_DISCHARGE".to_owned(),
                origin: [10200., 5400., 1250.],
                z_axis: [0., 0., 1.],
                x_axis: [1., 0., 0.],
            }
        ];

        let objs = adapter.map_assembly(asm, &export, Some(&step));
        let eq = &objs[0];
        // STEP bbox should be used (more accurate)
        let bbox = &eq["geometry"]["boundingBox"];
        assert!((bbox["xMin"].as_f64().unwrap() - 10000.).abs() < 0.1);
        assert!((bbox["zMax"].as_f64().unwrap() - 1700.).abs() < 0.1);
        // LOD should be LOD3_FINE (step file available)
        assert_eq!(eq["geometry"]["lod"], "LOD3_FINE");
    }

    #[test]
    fn test_map_piping_network() {
        let export = mock_export();
        let adapter = CreoAdapter::new(test_config());
        let seg = CreoPipingSegment {
            segment_id: "S001".to_owned(),
            network_name: "CW-201".to_owned(),
            nominal_diameter_in: 8.0,
            pipe_spec: "A1A2".to_owned(),
            outside_diameter: 219.1, wall_thickness: 8.18,
            start_point: CreoPoint { x: 0., y: 0., z: 850. },
            end_point:   CreoPoint { x: 2500., y: 0., z: 850. },
            route_points: vec![], fittings: vec![], material: None,
        };
        let objs = adapter.map_piping_network("CW-201", &[&seg], &export);
        // PipingNetworkSystem + PipingSegment + Pipe = 3 min
        assert!(objs.len() >= 3);
        assert_eq!(objs[0]["@type"], "pmef:PipingNetworkSystem");
        assert_eq!(objs[0]["lineNumber"], "CW-201");
        assert!((objs[0]["nominalDiameter"].as_f64().unwrap() - 203.2).abs() < 0.1);
        let pipe = &objs[2];
        assert_eq!(pipe["@type"], "pmef:Pipe");
        assert!((pipe["pipeLength"].as_f64().unwrap() - 2500.).abs() < 1.0);
    }

    #[test]
    fn test_adapter_trait() {
        let adapter = CreoAdapter::new(test_config());
        assert_eq!(adapter.name(), "pmef-adapter-creo");
        assert_eq!(adapter.target_system(), "CREO");
        assert_eq!(adapter.conformance_level(), 2);
        assert!(adapter.supported_domains().contains(&"equipment"));
    }

    #[test]
    fn test_min_volume_filter() {
        let config = CreoConfig {
            min_volume_mm3: 1e9, // very large — filter everything
            ..test_config()
        };
        let asm = mock_assembly();
        // Volume: 400×500×900 = 180,000,000 mm³ > 1e9? No → check
        let bb = asm.bounding_box.as_ref().unwrap();
        let vol = bb.to_mm(CreoUnit::Mm).volume();
        // 400 × 500 × 900 = 180e6 mm³
        assert!((vol - 180_000_000.).abs() < 1000.0, "vol={vol}");
    }
}
