//! # pmef-adapter-tekla
//!
//! PMEF adapter for **Tekla Structures** — bidirectional structural steel model exchange.
//!
//! ## Architecture
//!
//! Tekla Structures uses the **Tekla Open API** (.NET, runs inside the Tekla process).
//! This adapter uses a two-component design:
//!
//! 1. **C# exporter** (`pmef-tekla-dotnet/`) — a Tekla Open API plugin that reads
//!    the Tekla model and writes a structured JSON export file.
//!
//! 2. **Rust processor** (this crate) — reads the JSON export and produces PMEF NDJSON.
//!
//! ```text
//! Tekla Structures
//!   │
//!   ├── Tekla Open API (C# plugin) ──→ tekla-export.json
//!   │                                        │
//!   │                                        ▼
//!   │                               Rust JSON reader
//!   │                               + profile mapper
//!   │                               + field mapper
//!   │                                        │
//!   │                                        ▼
//!   └── ─────────────────────────────→ PMEF NDJSON
//! ```
//!
//! ## Import (PMEF → Tekla)
//!
//! PMEF NDJSON → C# importer (`PmefImporter.cs`) → Tekla model via Open API.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use pmef_adapter_tekla::{TeklaAdapter, TeklaConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = TeklaConfig {
//!         project_code: "eaf-2026".to_owned(),
//!         export_path: "tekla-export.json".into(),
//!         ..Default::default()
//!     };
//!     let mut adapter = TeklaAdapter::new(config);
//!     let stats = adapter.export_to_pmef("output.ndjson").await?;
//!     println!("Exported {} objects", stats.objects_ok);
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod export_schema;
pub mod profile_map;

pub use export_schema::*;
pub use profile_map::{build_override_table, map_material, map_profile, map_profile_with_table, ProfileId};

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the Tekla adapter.
#[derive(Debug, Clone)]
pub struct TeklaConfig {
    /// PMEF project code for @id generation.
    pub project_code: String,
    /// Path to the Tekla JSON export file (produced by `PmefExporter.cs`).
    pub export_path: PathBuf,
    /// Export only steel members (skip concrete, pads, etc.). Default: false.
    pub steel_only: bool,
    /// Minimum member length [mm] to include. Default: 0 (include all).
    pub min_length_mm: f64,
    /// Include connections. Default: true.
    pub include_connections: bool,
    /// Include assemblies as PMEF Spool objects. Default: false.
    pub include_assemblies: bool,
    /// Include analysis results in customAttributes. Default: true.
    pub include_analysis: bool,
    /// Parent unit @id for isPartOf references.
    pub unit_id: Option<String>,
}

impl Default for TeklaConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            export_path: PathBuf::from("tekla-export.json"),
            steel_only: false,
            min_length_mm: 0.0,
            include_connections: true,
            include_assemblies: false,
            include_analysis: true,
            unit_id: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Adapter
// ─────────────────────────────────────────────────────────────────────────────

/// Tekla Structures → PMEF adapter.
pub struct TeklaAdapter {
    config: TeklaConfig,
    profile_overrides: HashMap<String, (String, String)>,
    /// GUID → PMEF @id for connection resolution.
    guid_to_id: HashMap<String, String>,
}

impl TeklaAdapter {
    /// Create a new Tekla adapter.
    pub fn new(config: TeklaConfig) -> Self {
        Self {
            profile_overrides: build_override_table(),
            guid_to_id: HashMap::new(),
            config,
        }
    }

    /// Export the Tekla model (from JSON export) to PMEF NDJSON.
    pub async fn export_to_pmef(
        &mut self,
        output_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        use std::fs::File;
        use std::io::BufWriter;

        let t0 = std::time::Instant::now();
        let mut stats = AdapterStats::default();

        // Load the Tekla JSON export
        let json_text = std::fs::read_to_string(&self.config.export_path)
            .map_err(AdapterError::Io)?;
        let export: TeklaExport = serde_json::from_str(&json_text)
            .map_err(|e| AdapterError::Json(e))?;

        tracing::info!(
            "Loaded Tekla export: {} members, {} connections from '{}'",
            export.members.len(), export.connections.len(), export.model_name
        );

        // Pre-register all member GUIDs → PMEF IDs
        for member in &export.members {
            let pmef_id = self.member_id(&member.member_mark, &member.guid);
            self.guid_to_id.insert(member.guid.clone(), pmef_id);
        }

        // Open output
        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        // Write FileHeader + Plant + Unit
        let proj = self.config.project_code.clone();
        for obj in self.make_header_objects(&export, &proj) {
            writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Write structural members
        for member in &export.members {
            // Filter
            if self.config.steel_only && !member.member_class.is_steel() {
                stats.objects_skipped += 1;
                continue;
            }
            if member.length_mm < self.config.min_length_mm {
                stats.objects_skipped += 1;
                continue;
            }

            match self.map_member(member) {
                Ok(objs) => {
                    for obj in objs {
                        writer.write_value(&obj)
                            .map_err(|e| AdapterError::Json(e.into()))?;
                        stats.objects_ok += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to map member '{}': {e}", member.member_mark);
                    stats.objects_failed += 1;
                }
            }
        }

        // Write connections
        if self.config.include_connections {
            for conn in &export.connections {
                match self.map_connection(conn) {
                    Ok(objs) => {
                        for obj in objs {
                            writer.write_value(&obj)
                                .map_err(|e| AdapterError::Json(e.into()))?;
                            stats.objects_ok += 1;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to map connection {}: {e}", conn.identifier);
                        stats.objects_failed += 1;
                    }
                }
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        tracing::info!(
            "Tekla export complete: {} ok, {} failed, {} skipped in {}ms",
            stats.objects_ok, stats.objects_failed, stats.objects_skipped, stats.duration_ms
        );
        Ok(stats)
    }

    // ── @id generation ────────────────────────────────────────────────────────

    fn member_id(&self, mark: &str, guid: &str) -> String {
        let clean: String = mark.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        format!("urn:pmef:obj:{}:STR-{clean}", self.config.project_code)
    }

    fn connection_id(&self, mark: &str, idx: usize) -> String {
        let clean: String = mark.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        if clean.is_empty() {
            format!("urn:pmef:obj:{}:CON-{idx:04}", self.config.project_code)
        } else {
            format!("urn:pmef:obj:{}:CON-{clean}", self.config.project_code)
        }
    }

    fn make_has_equivalent_in(
        &self, pmef_id: &str, tekla_guid: &str, tekla_id: u64,
    ) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-tekla", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id,
            "targetId": pmef_id,
            "targetSystem": "TEKLA_STRUCTURES",
            "targetSystemId": tekla_guid,
            "mappingType": "EXACT",
            "derivedBy": "ADAPTER_IMPORT",
            "confidence": 1.0,
            "customAttributes": { "teklaId": tekla_id },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringTool": "pmef-adapter-tekla 0.9.0"
            }
        })
    }

    // ── Header objects ────────────────────────────────────────────────────────

    fn make_header_objects(&self, export: &TeklaExport, proj: &str) -> Vec<serde_json::Value> {
        let plant_id = format!("urn:pmef:plant:{proj}:{}", export.model_name
            .chars().filter(|c| c.is_alphanumeric() || *c == '-').collect::<String>());
        let unit_id = self.config.unit_id.clone().unwrap_or_else(||
            format!("urn:pmef:unit:{proj}:{}-U01", export.model_name
                .chars().filter(|c| c.is_alphanumeric() || *c == '-').collect::<String>())
        );

        vec![
            serde_json::json!({
                "@type": "pmef:FileHeader",
                "@id": format!("urn:pmef:pkg:{proj}:{}", export.model_name),
                "pmefVersion": "0.9.0",
                "plantId": plant_id,
                "projectCode": proj,
                "coordinateSystem": "Z-up",
                "units": "mm",
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringTool": format!("pmef-adapter-tekla 0.9.0 / Tekla {}", export.tekla_version)
            }),
            serde_json::json!({
                "@type": "pmef:Plant",
                "@id": plant_id,
                "pmefVersion": "0.9.0",
                "name": export.model_name,
                "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
            }),
            serde_json::json!({
                "@type": "pmef:Unit",
                "@id": unit_id,
                "pmefVersion": "0.9.0",
                "name": export.project.as_ref().map(|p| p.project_name.clone())
                    .unwrap_or_else(|| export.model_name.clone()),
                "isPartOf": plant_id,
                "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
            }),
        ]
    }

    // ── Member mapping ────────────────────────────────────────────────────────

    fn map_member(
        &self,
        member: &TeklaMember,
    ) -> Result<Vec<serde_json::Value>, AdapterError> {
        let obj_id = self.guid_to_id.get(&member.guid)
            .cloned()
            .unwrap_or_else(|| self.member_id(&member.member_mark, &member.guid));

        let unit_id = self.config.unit_id.clone().unwrap_or_else(||
            format!("urn:pmef:unit:{}:U-01", self.config.project_code)
        );

        // Map profile
        let profile_id = map_profile_with_table(&member.profile, &self.profile_overrides);
        let (grade, std) = map_material(&member.material);

        // Analysis results → customAttributes
        let analysis_attrs = if self.config.include_analysis {
            member.analysis.as_ref().map(|a| serde_json::json!({
                "utilisationRatio": a.utilisation_ratio,
                "criticalCheck": a.critical_check,
                "axialForce_kN": a.axial_force_kn,
                "majorBending_kNm": a.major_bending_knm,
                "minorBending_kNm": a.minor_bending_knm,
                "shearY_kN": a.shear_y_kn,
                "shearZ_kN": a.shear_z_kn,
            }))
        } else { None };

        // Connection type from end releases
        let (start_conn, end_conn) = (
            self.end_release_to_pmef(&member.start_release),
            self.end_release_to_pmef(&member.end_release),
        );

        let fire_prot = member.fire_protection.as_ref().map(|fp| serde_json::json!({
            "type": fp.protection_type,
            "requiredPeriod": fp.required_period_min,
            "sectionFactor": fp.section_factor_m,
            "thicknessMm": fp.thickness_mm
        }));

        let obj = serde_json::json!({
            "@type": "pmef:SteelMember",
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": unit_id,
            "memberMark": member.member_mark,
            "memberType": member.member_class.pmef_member_type(),
            "profileId": profile_id.as_pmef_str(),
            "startPoint": [member.start_point.x, member.start_point.y, member.start_point.z],
            "endPoint":   [member.end_point.x,   member.end_point.y,   member.end_point.z],
            "rollAngle": member.roll_angle_deg,
            "material": {
                "grade": grade,
                "standard": std,
                "fy": fy_for_grade(grade),
                "fu": fu_for_grade(grade)
            },
            "weight": member.mass_kg,
            "finish": member.finish.as_ref().map(|f| f.as_pmef_str()),
            "fireProtection": fire_prot,
            "teklaGUID": member.guid,
            "cis2Ref": member.cis2_ref,
            "startConnectionType": start_conn,
            "endConnectionType": end_conn,
            "geometry": member.bbox.as_ref().map(|bb| serde_json::json!({
                "type": "none",
                "boundingBox": {
                    "xMin": bb.min.x, "xMax": bb.max.x,
                    "yMin": bb.min.y, "yMax": bb.max.y,
                    "zMin": bb.min.z, "zMax": bb.max.z
                }
            })).unwrap_or(serde_json::json!({ "type": "none" })),
            "customAttributes": {
                "teklaId": member.tekla_id,
                "partMark": member.part_mark,
                "assemblyId": member.assembly_id,
                "surfaceArea_m2": member.surface_area_m2,
                "profileOriginal": member.profile,
                "analysisResults": analysis_attrs,
                "udas": member.udas
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": member.guid,
                "authoringTool": "pmef-adapter-tekla 0.9.0"
            }
        });

        let equiv = self.make_has_equivalent_in(&obj_id, &member.guid, member.tekla_id);
        Ok(vec![obj, equiv])
    }

    fn end_release_to_pmef(&self, release: &TeklaEndRelease) -> &'static str {
        if release.is_pinned() { "PINNED" }
        else if release.is_fixed() { "FIXED" }
        else { "PARTIAL" }
    }

    // ── Connection mapping ────────────────────────────────────────────────────

    fn map_connection(
        &self,
        conn: &TeklaConnection,
    ) -> Result<Vec<serde_json::Value>, AdapterError> {
        let conn_id = self.connection_id(
            conn.connection_mark.as_deref().unwrap_or(&conn.identifier),
            conn.tekla_id as usize,
        );

        let unit_id = self.config.unit_id.clone().unwrap_or_else(||
            format!("urn:pmef:unit:{}:U-01", self.config.project_code)
        );

        // Resolve member GUIDs → PMEF IDs
        let member_ids: Vec<String> = conn.member_guids.iter()
            .filter_map(|g| self.guid_to_id.get(g))
            .cloned()
            .collect();

        let bolt_spec = conn.bolt_spec.as_ref().map(|bs| serde_json::json!({
            "boltGrade": bs.grade,
            "boltDiameter": bs.diameter_mm,
            "numberOfBolts": bs.count,
            "holeType": format!("{:?}", bs.hole_type).to_uppercase(),
            "preloaded": bs.preloaded,
            "assembly": bs.assembly
        }));

        let capacity = conn.design_capacity.as_ref().map(|cap| serde_json::json!({
            "shear": cap.shear_kn,
            "moment": cap.moment_knm,
            "axial": cap.axial_kn
        }));

        let obj = serde_json::json!({
            "@type": "pmef:SteelConnection",
            "@id": conn_id,
            "isPartOf": unit_id,
            "connectionMark": conn.connection_mark,
            "connectionType": conn.connection_type.pmef_connection_type(),
            "memberIds": member_ids,
            "coordinate": [conn.position.x, conn.position.y, conn.position.z],
            "utilisationRatio": conn.utilisation_ratio,
            "teklaConnectionNumber": conn.component_number,
            "boltSpec": bolt_spec,
            "designCapacity": capacity,
            "customAttributes": {
                "teklaId": conn.tekla_id,
                "weldSizeMm": conn.weld_size_mm,
                "teklaGuid": conn.identifier
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": conn.identifier,
                "authoringTool": "pmef-adapter-tekla 0.9.0"
            }
        });

        let equiv = self.make_has_equivalent_in(&conn_id, &conn.identifier, conn.tekla_id);
        Ok(vec![obj, equiv])
    }
}

impl PmefAdapter for TeklaAdapter {
    fn name(&self) -> &str { "pmef-adapter-tekla" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "TEKLA_STRUCTURES" }
    fn supported_domains(&self) -> &[&str] { &["steel"] }
    fn conformance_level(&self) -> u8 { 3 }
    fn description(&self) -> &str {
        "Tekla Structures → PMEF adapter. Reads the JSON export produced by \
         PmefExporter.cs (Tekla Open API plugin) and writes PMEF NDJSON. \
         Level 3 conformance for structural steel domain. \
         Supports EN and AISC profile mapping, 10 connection types, \
         fire protection, and analysis results."
    }
}

// ── Material property lookup ──────────────────────────────────────────────────

/// Nominal yield strength [MPa] for common steel grades.
fn fy_for_grade(grade: &str) -> f64 {
    match grade.to_uppercase().as_str() {
        "S235JR" | "A36"        => 235.0,
        "S275JR"                 => 275.0,
        "S355JR" | "A572 GR.50" => 355.0,
        "S420ML"                 => 420.0,
        "S460ML"                 => 460.0,
        "A992"                   => 345.0,
        "A325"                   => 635.0,
        "A490"                   => 895.0,
        _                        => 275.0,  // conservative default
    }
}

/// Nominal ultimate strength [MPa] for common steel grades.
fn fu_for_grade(grade: &str) -> f64 {
    match grade.to_uppercase().as_str() {
        "S235JR" | "A36"        => 360.0,
        "S275JR"                 => 430.0,
        "S355JR" | "A572 GR.50" => 490.0,
        "S420ML"                 => 520.0,
        "S460ML"                 => 550.0,
        "A992"                   => 448.0,
        "A325"                   => 825.0,
        "A490"                   => 1035.0,
        _                        => 430.0,  // conservative default
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_member() -> TeklaMember {
        TeklaMember {
            identifier: "GUID-BM-001".to_owned(),
            tekla_id: 12345,
            member_class: TeklaMemberClass::Beam,
            member_mark: "B101".to_owned(),
            part_mark: Some("B101-1".to_owned()),
            profile: "HEA200".to_owned(),
            material: "S355JR".to_owned(),
            start_point: TeklaPoint { x: 0.0, y: 0.0, z: 6000.0 },
            end_point: TeklaPoint { x: 6000.0, y: 0.0, z: 6000.0 },
            roll_angle_deg: 0.0,
            length_mm: 6000.0,
            mass_kg: Some(126.0),
            surface_area_m2: Some(2.4),
            guid: "GUID-BM-001".to_owned(),
            cis2_ref: Some("CIS2-BM-001".to_owned()),
            udas: HashMap::new(),
            assembly_id: None,
            finish: Some(TeklaFinish::HotDipGalvanized),
            fire_protection: None,
            analysis: Some(TeklaAnalysisResult {
                utilisation_ratio: Some(0.72),
                critical_check: Some("LTB".to_owned()),
                axial_force_kn: Some(-50.0),
                major_bending_knm: Some(45.0),
                minor_bending_knm: Some(0.0),
                shear_y_kn: None,
                shear_z_kn: None,
            }),
            bbox: Some(TeklaBbox {
                min: TeklaPoint { x: 0.0, y: -100.0, z: 5895.0 },
                max: TeklaPoint { x: 6000.0, y: 100.0, z: 6200.0 },
            }),
            start_release: TeklaEndRelease::default(),
            end_release: TeklaEndRelease::default(),
        }
    }

    fn test_config() -> TeklaConfig {
        TeklaConfig {
            project_code: "test".to_owned(),
            export_path: PathBuf::from("nonexistent.json"),
            ..Default::default()
        }
    }

    #[test]
    fn test_map_member_basic() {
        let mut adapter = TeklaAdapter::new(test_config());
        let member = test_member();
        // Pre-register GUID
        adapter.guid_to_id.insert(member.guid.clone(), adapter.member_id(&member.member_mark, &member.guid));
        let objs = adapter.map_member(&member).unwrap();
        assert_eq!(objs.len(), 2); // SteelMember + HasEquivalentIn
        let sm = &objs[0];
        assert_eq!(sm["@type"], "pmef:SteelMember");
        assert_eq!(sm["memberMark"], "B101");
        assert_eq!(sm["memberType"], "BEAM");
        assert_eq!(sm["profileId"], "EN:HEA200");
        assert_eq!(sm["material"]["grade"], "S355JR");
        assert_eq!(sm["material"]["fy"], 355.0);
        assert_eq!(sm["material"]["fu"], 490.0);
        assert_eq!(sm["finish"], "HOT_DIP_GALVANIZED");
        assert_eq!(sm["teklaGUID"], "GUID-BM-001");
        assert_eq!(sm["cis2Ref"], "CIS2-BM-001");
    }

    #[test]
    fn test_map_member_start_end_coords() {
        let mut adapter = TeklaAdapter::new(test_config());
        let member = test_member();
        adapter.guid_to_id.insert(member.guid.clone(), adapter.member_id(&member.member_mark, &member.guid));
        let objs = adapter.map_member(&member).unwrap();
        let sm = &objs[0];
        let sp = sm["startPoint"].as_array().unwrap();
        assert!((sp[0].as_f64().unwrap() - 0.0).abs() < 0.01);
        assert!((sp[2].as_f64().unwrap() - 6000.0).abs() < 0.01);
        let ep = sm["endPoint"].as_array().unwrap();
        assert!((ep[0].as_f64().unwrap() - 6000.0).abs() < 0.01);
    }

    #[test]
    fn test_map_member_analysis_results() {
        let mut adapter = TeklaAdapter::new(test_config());
        let member = test_member();
        adapter.guid_to_id.insert(member.guid.clone(), adapter.member_id(&member.member_mark, &member.guid));
        let objs = adapter.map_member(&member).unwrap();
        let attrs = &objs[0]["customAttributes"]["analysisResults"];
        assert!(!attrs.is_null());
        assert!((attrs["utilisationRatio"].as_f64().unwrap() - 0.72).abs() < 0.001);
        assert_eq!(attrs["criticalCheck"], "LTB");
    }

    #[test]
    fn test_map_member_has_equivalent_in() {
        let mut adapter = TeklaAdapter::new(test_config());
        let member = test_member();
        adapter.guid_to_id.insert(member.guid.clone(), adapter.member_id(&member.member_mark, &member.guid));
        let objs = adapter.map_member(&member).unwrap();
        let rel = &objs[1];
        assert_eq!(rel["@type"], "pmef:HasEquivalentIn");
        assert_eq!(rel["targetSystem"], "TEKLA_STRUCTURES");
        assert_eq!(rel["targetSystemId"], "GUID-BM-001");
        assert_eq!(rel["confidence"], 1.0);
    }

    #[test]
    fn test_map_connection_bolted() {
        let adapter = TeklaAdapter::new(test_config());
        let conn = TeklaConnection {
            identifier: "GUID-CON-001".to_owned(),
            tekla_id: 99,
            component_number: 142,
            connection_type: TeklaConnectionType::BoltedEndPlate,
            connection_mark: Some("CON-B001".to_owned()),
            member_guids: vec!["GUID-BM-001".to_owned(), "GUID-COL-001".to_owned()],
            position: TeklaPoint { x: 0.0, y: 0.0, z: 6000.0 },
            bolt_spec: Some(TeklaBoltSpec {
                grade: "8.8".to_owned(),
                diameter_mm: 20.0,
                count: 8,
                hole_type: TeklaHoleType::Clearance,
                preloaded: false,
                assembly: None,
            }),
            weld_size_mm: None,
            design_capacity: Some(TeklaConnectionCapacity {
                shear_kn: Some(280.0),
                moment_knm: Some(120.0),
                axial_kn: None,
            }),
            utilisation_ratio: Some(0.68),
        };
        let objs = adapter.map_connection(&conn).unwrap();
        assert_eq!(objs.len(), 2);
        let sc = &objs[0];
        assert_eq!(sc["@type"], "pmef:SteelConnection");
        assert_eq!(sc["connectionType"], "BOLTED_ENDPLATE");
        assert_eq!(sc["utilisationRatio"], 0.68);
        assert_eq!(sc["teklaConnectionNumber"], 142);
        let bolt = &sc["boltSpec"];
        assert_eq!(bolt["boltGrade"], "8.8");
        assert_eq!(bolt["boltDiameter"], 20.0);
        assert_eq!(bolt["numberOfBolts"], 8);
    }

    #[test]
    fn test_make_header_objects() {
        let adapter = TeklaAdapter::new(test_config());
        let export = TeklaExport {
            schema_version: "1.0".to_owned(),
            tekla_version: "2024".to_owned(),
            exported_at: "2026-03-31T00:00:00Z".to_owned(),
            model_name: "EAF-2026".to_owned(),
            project: None,
            members: vec![],
            connections: vec![],
            assemblies: vec![],
            grids: vec![],
            summary: TeklaExportSummary { member_count: 0, connection_count: 0, assembly_count: 0 },
        };
        let headers = adapter.make_header_objects(&export, "test");
        assert_eq!(headers.len(), 3);
        assert_eq!(headers[0]["@type"], "pmef:FileHeader");
        assert_eq!(headers[1]["@type"], "pmef:Plant");
        assert_eq!(headers[2]["@type"], "pmef:Unit");
        assert_eq!(headers[0]["coordinateSystem"], "Z-up");
    }

    #[test]
    fn test_fy_fu_lookup() {
        assert!((fy_for_grade("S355JR") - 355.0).abs() < 0.1);
        assert!((fu_for_grade("S355JR") - 490.0).abs() < 0.1);
        assert!((fy_for_grade("A36") - 235.0).abs() < 0.1);
        assert!((fy_for_grade("A992") - 345.0).abs() < 0.1);
    }

    #[test]
    fn test_adapter_trait() {
        let adapter = TeklaAdapter::new(test_config());
        assert_eq!(adapter.name(), "pmef-adapter-tekla");
        assert_eq!(adapter.target_system(), "TEKLA_STRUCTURES");
        assert_eq!(adapter.conformance_level(), 3); // Level 3 for steel!
        assert!(adapter.supported_domains().contains(&"steel"));
    }

    #[test]
    fn test_steel_only_filter() {
        let mut config = test_config();
        config.steel_only = true;
        let adapter = TeklaAdapter::new(config);
        // Concrete member should be skipped
        assert!(!TeklaMemberClass::Pad.is_steel());
        assert!(!TeklaMemberClass::Slab.is_steel());
        // Steel members pass
        assert!(TeklaMemberClass::Beam.is_steel());
        assert!(TeklaMemberClass::Column.is_steel());
    }
}
