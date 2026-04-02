//! # pmef-adapter-inventor
//!
//! PMEF adapter for **Autodesk Inventor** — mechanical assembly envelopes,
//! Frame Generator structural members, and Tube & Pipe routing.
//!
//! ## Architecture
//!
//! Like the other adapters, Inventor uses a two-component design:
//! 1. **C# Add-in** (`InventorExporter.cs`) — reads the Inventor model via
//!    the COM API and writes a structured JSON export.
//! 2. **Rust processor** (this crate) — maps the JSON to PMEF NDJSON.
//!
//! ## PMEF parameter convention
//!
//! Inventor stores engineering attributes in model parameters. The following
//! parameter names are read by the add-in:
//!
//! | Parameter | Type | PMEF field |
//! |-----------|------|-----------|
//! | `PMEF_TAG` | String | `equipmentBasic.tagNumber` |
//! | `PMEF_CLASS` | String | `equipmentBasic.equipmentClass` |
//! | `PMEF_DESIGN_PRESSURE` | Real [bar g] | `customAttributes.designPressure_Pa` |
//! | `PMEF_DESIGN_TEMP` | Real [°C] | `customAttributes.designTemperature_K` |
//! | `PMEF_DESIGN_CODE` | String | `equipmentBasic.designCode` |
//!
//! Nozzle work points follow the naming convention `PMEF_NOZZLE_<mark>`.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod export_schema;

pub use export_schema::{
    inventor_class_to_pmef, inv_section_to_standard,
    InventorAssembly, InventorExport, InventorFrameMember,
    InventorNozzlePoint, InventorPart, InventorTubeRun,
    InvBbox, InvPoint, InvTransform, FrameMemberType,
};

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the Inventor adapter.
#[derive(Debug, Clone)]
pub struct InventorConfig {
    /// PMEF project code for @id generation.
    pub project_code: String,
    /// Path to the Inventor JSON export file.
    pub export_path: PathBuf,
    /// Directory containing STEP files (optional — for geometry enrichment).
    pub step_dir: Option<PathBuf>,
    /// Minimum assembly bounding box diagonal [mm] to include.
    /// Filters out tiny hardware (bolts, washers). Default: 50 mm.
    pub min_diagonal_mm: f64,
    /// Include only assemblies with `PMEF_TAG` parameter set. Default: false.
    pub tagged_only: bool,
    /// Include Frame Generator members as PMEF SteelMember objects. Default: true.
    pub include_frame: bool,
    /// Include Tube & Pipe runs as PMEF piping objects. Default: true.
    pub include_tube_pipe: bool,
    /// Parent unit @id for isPartOf references.
    pub unit_id: Option<String>,
    /// Vault server URL (for document links).
    pub vault_url: Option<String>,
}

impl Default for InventorConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            export_path: PathBuf::from("inventor-export.json"),
            step_dir: None,
            min_diagonal_mm: 50.0,
            tagged_only: false,
            include_frame: true,
            include_tube_pipe: true,
            unit_id: None,
            vault_url: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Material mapping
// ─────────────────────────────────────────────────────────────────────────────

fn material_to_pmef(mat: &str) -> &str {
    match mat.trim().to_uppercase().replace(['-',' '], "").as_str() {
        "S235" | "S235JR"                  => "S235JR",
        "S275" | "S275JR"                  => "S275JR",
        "S355" | "S355JR" | "S355J2"       => "S355JR",
        "S420" | "S420ML"                  => "S420ML",
        "S460" | "S460ML"                  => "S460ML",
        "A36" | "ASTMA36"                  => "A36",
        "A992" | "ASTMA992"                => "A992",
        "A106B" | "A106GRADEB"             => "ASTM A106 Gr. B",
        "SS316L" | "1.4404" | "316L"       => "ASTM A312 TP316L",
        "SS304L" | "1.4307" | "304L"       => "ASTM A312 TP304L",
        "ALUMINIUM" | "ALUMINUM" | "AL"    => "Aluminium",
        "GREY CAST IRON" | "GREYCASTIRON"  => "EN-GJL-250",
        "DUCTILE IRON" | "DUCTILEIRON"     => "EN-GJS-400-15",
        _ => mat,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inventor Adapter
// ─────────────────────────────────────────────────────────────────────────────

/// Autodesk Inventor → PMEF adapter.
pub struct InventorAdapter {
    config: InventorConfig,
    /// occurrence_path → PMEF @id
    path_to_id: HashMap<String, String>,
    /// sequential counters per type
    counters: HashMap<String, usize>,
}

impl InventorAdapter {
    pub fn new(config: InventorConfig) -> Self {
        Self {
            config,
            path_to_id: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    // ── @id helpers ───────────────────────────────────────────────────────────

    fn clean(s: &str) -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            .collect()
    }

    fn assembly_id(&self, tag: &str, path: &str) -> String {
        let local = if tag.is_empty() || tag == path {
            let short = path.split(':').last().unwrap_or(path);
            Self::clean(short)
        } else {
            Self::clean(tag)
        };
        format!("urn:pmef:obj:{}:{local}", self.config.project_code)
    }

    fn frame_id(&mut self, mark: &str) -> String {
        let n = self.counters.entry("FRAME".to_owned()).or_insert(0);
        *n += 1;
        let clean = Self::clean(mark);
        format!("urn:pmef:obj:{}:STR-{clean}-{:03}", self.config.project_code, n)
    }

    fn line_id(&self, name: &str) -> String {
        format!("urn:pmef:line:{}:{}", self.config.project_code, Self::clean(name))
    }

    fn unit_id(&self) -> String {
        self.config.unit_id.clone()
            .unwrap_or_else(|| format!("urn:pmef:unit:{}:U-01", self.config.project_code))
    }

    fn make_has_equivalent_in(
        &self, pmef_id: &str, inventor_path: &str, vault: Option<&str>,
    ) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        let mut rel = serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-inventor", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id, "targetId": pmef_id,
            "targetSystem": "INVENTOR",
            "targetSystemId": inventor_path,
            "mappingType": "EXACT",
            "derivedBy": "ADAPTER_IMPORT",
            "confidence": 1.0,
            "revision": {
                "revisionId": "r2026-01-01-001", "changeState": "SHARED",
                "authoringTool": "pmef-adapter-inventor 0.9.0"
            }
        });
        if let Some(v) = vault {
            rel["vaultNumber"] = serde_json::Value::String(v.to_owned());
        }
        rel
    }

    // ── Export pipeline ───────────────────────────────────────────────────────

    /// Export the Inventor model (from JSON export) to PMEF NDJSON.
    pub async fn export_to_pmef(
        &mut self, output_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        use std::fs::File;
        use std::io::BufWriter;

        let t0 = std::time::Instant::now();
        let mut stats = AdapterStats::default();

        let json = std::fs::read_to_string(&self.config.export_path)
            .map_err(AdapterError::Io)?;
        let export: InventorExport = serde_json::from_str(&json)
            .map_err(|e| AdapterError::Json(e))?;

        tracing::info!(
            "Loaded Inventor export: {} assemblies, {} frame members, {} tube runs from '{}'",
            export.assemblies.len(), export.frame_members.len(),
            export.tube_runs.len(), export.assembly_name
        );

        // Pre-register all assembly IDs
        for asm in &export.assemblies {
            let id = self.assembly_id(asm.tag_number(), &asm.occurrence_path);
            self.path_to_id.insert(asm.occurrence_path.clone(), id);
        }

        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        // Header
        let proj = self.config.project_code.clone();
        for obj in self.make_header_objects(&export, &proj) {
            writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Assemblies → equipment objects
        for asm in &export.assemblies {
            // Filter: skip if tagged_only and no PMEF_TAG
            if self.config.tagged_only && asm.pmef_tag.is_none() {
                stats.objects_skipped += 1;
                continue;
            }
            // Filter: skip tiny components by diagonal
            if let Some(bb) = &asm.bounding_box {
                if bb.diagonal() < self.config.min_diagonal_mm {
                    stats.objects_skipped += 1;
                    continue;
                }
            }

            for obj in self.map_assembly(asm, &export) {
                writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                stats.objects_ok += 1;
            }
        }

        // Frame Generator members
        if self.config.include_frame {
            for member in &export.frame_members {
                for obj in self.map_frame_member(member) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // Tube & Pipe runs
        if self.config.include_tube_pipe {
            for run in &export.tube_runs {
                for obj in self.map_tube_run(run) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        tracing::info!(
            "Inventor export: {} ok, {} skipped in {}ms",
            stats.objects_ok, stats.objects_skipped, stats.duration_ms
        );
        Ok(stats)
    }

    // ── Header ────────────────────────────────────────────────────────────────

    fn make_header_objects(
        &self, export: &InventorExport, proj: &str,
    ) -> Vec<serde_json::Value> {
        let asm_clean = Self::clean(&export.assembly_name);
        let plant_id  = format!("urn:pmef:plant:{proj}:{asm_clean}");
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
                "authoringTool": format!("pmef-adapter-inventor 0.9.0 / {}", export.inventor_version)
            }),
            serde_json::json!({
                "@type": "pmef:Plant",
                "@id": plant_id,
                "pmefVersion": "0.9.0",
                "name": export.assembly_name,
                "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
            }),
            serde_json::json!({
                "@type": "pmef:Unit",
                "@id": self.unit_id(),
                "pmefVersion": "0.9.0",
                "name": export.assembly_name,
                "isPartOf": plant_id,
                "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
            }),
        ]
    }

    // ── Assembly → Equipment ──────────────────────────────────────────────────

    fn map_assembly(
        &self, asm: &InventorAssembly, export: &InventorExport,
    ) -> Vec<serde_json::Value> {
        let obj_id = self.path_to_id.get(&asm.occurrence_path)
            .cloned()
            .unwrap_or_else(|| self.assembly_id(asm.tag_number(), &asm.occurrence_path));

        let (pmef_type, equip_class) = inventor_class_to_pmef(
            asm.equipment_class().unwrap_or("GENERIC"),
        );

        // Bounding box
        let bbox_json = asm.bounding_box.as_ref().map(|bb| {
            serde_json::json!({
                "xMin": bb.x_min, "xMax": bb.x_max,
                "yMin": bb.y_min, "yMax": bb.y_max,
                "zMin": bb.z_min, "zMax": bb.z_max
            })
        });

        let lod = if asm.step_file.is_some() { "LOD3_FINE" } else { "LOD1_COARSE" };

        let step_uri = asm.step_file.as_ref()
            .and_then(|f| self.config.vault_url.as_ref()
                .map(|base| format!("{base}/vault/{f}")));

        // Nozzles — from export.nozzle_work_points filtered by parent
        let nozzles: Vec<serde_json::Value> = export.nozzle_points.iter()
            .filter(|noz| noz.parent_occurrence_path == asm.occurrence_path)
            .chain(
                asm.nozzle_work_points.iter().map(|wp| {
                    // Convert InvWorkPoint → InventorNozzlePoint on the fly
                    let mark = wp.name.trim_start_matches("PMEF_NOZZLE_").to_owned();
                    let dn = wp.parameters.get("NZ_DN")
                        .and_then(|v| v.as_f64());
                    let rating = wp.parameters.get("NZ_RATING")
                        .and_then(|v| v.as_str()).map(|s| s.to_owned());
                    let facing = wp.parameters.get("NZ_FACING")
                        .and_then(|v| v.as_str()).map(|s| s.to_owned());
                    let svc = wp.parameters.get("NZ_SERVICE")
                        .and_then(|v| v.as_str()).map(|s| s.to_owned());
                    // We need a reference lifetime trick — build inline
                    &*Box::leak(Box::new(InventorNozzlePoint {
                        work_point_name: wp.name.clone(),
                        nozzle_mark: mark,
                        parent_occurrence_path: asm.occurrence_path.clone(),
                        position: wp.position.clone(),
                        direction: wp.z_axis,
                        nominal_diameter_mm: dn,
                        flange_rating: rating,
                        facing_type: facing,
                        service: svc,
                    }))
                })
            )
            .map(|noz| {
                serde_json::json!({
                    "nozzleId": noz.nozzle_mark,
                    "nozzleMark": noz.nozzle_mark,
                    "service": noz.service,
                    "nominalDiameter": noz.nominal_diameter_mm.unwrap_or(100.0),
                    "flangeRating": noz.flange_rating.as_deref()
                        .map(|r| format!("ANSI-{r}")),
                    "facingType": noz.facing_type.as_deref().unwrap_or("RF"),
                    "coordinate": [noz.position.x, noz.position.y, noz.position.z],
                    "direction": noz.direction
                })
            })
            .collect();

        // Vault document link
        let documents: Vec<serde_json::Value> = asm.vault_number.as_ref()
            .map(|vn| vec![serde_json::json!({
                "documentId": vn,
                "documentType": "VAULT_DOC",
                "revision": asm.iproperties.revision.as_deref().unwrap_or("latest")
            })])
            .unwrap_or_default();

        let mat_pmef = asm.iproperties.material.as_deref()
            .map(material_to_pmef);

        let obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": self.unit_id(),
            "equipmentBasic": {
                "tagNumber":          asm.tag_number(),
                "equipmentClass":     equip_class,
                "serviceDescription": asm.iproperties.description,
                "designCode":         asm.pmef_design_code
            },
            "nozzles": nozzles,
            "documents": documents,
            "geometry": {
                "type":         "none",
                "lod":          lod,
                "refUri":       step_uri,
                "boundingBox":  bbox_json
            },
            "customAttributes": {
                "inventorOccurrencePath": asm.occurrence_path,
                "inventorIamFile":        asm.iam_file,
                "vaultNumber":            asm.vault_number,
                "partNumber":             asm.iproperties.part_number,
                "designer":               asm.iproperties.designer,
                "material":               mat_pmef,
                "massKg":                 asm.iproperties.mass_kg,
                "designPressure_Pa":      asm.design_pressure_pa(),
                "designTemperature_K":    asm.design_temp_k(),
                "isIAssembly":            asm.is_iassembly,
                "ipartRow":               asm.ipart_info.as_ref().map(|i| i.row_number),
                "inventorParameters":     asm.parameters
            },
            "revision": {
                "revisionId":          "r2026-01-01-001",
                "changeState":         "SHARED",
                "authoringToolObjectId": asm.occurrence_path,
                "authoringTool":       "pmef-adapter-inventor 0.9.0"
            }
        });

        vec![
            obj,
            self.make_has_equivalent_in(
                &obj_id, &asm.occurrence_path, asm.vault_number.as_deref(),
            ),
        ]
    }

    // ── Frame Generator member ────────────────────────────────────────────────

    fn map_frame_member(&mut self, member: &InventorFrameMember) -> Vec<serde_json::Value> {
        let obj_id = self.frame_id(&member.name);
        let (grade, standard) = match member.material.to_uppercase().as_str() {
            m if m.contains("S355") => ("S355JR", "EN 10025-2"),
            m if m.contains("S275") => ("S275JR", "EN 10025-2"),
            m if m.contains("S235") => ("S235JR", "EN 10025-2"),
            m if m.contains("S420") => ("S420ML", "EN 10025-4"),
            m if m.contains("S460") => ("S460ML", "EN 10025-4"),
            m if m.contains("A992") => ("A992",   "ASTM A992"),
            m if m.contains("A572") => ("A572 Gr.50", "ASTM A572"),
            m if m.contains("A36")  => ("A36",    "ASTM A36"),
            _                       => (member.material.as_str(), "UNKNOWN"),
        };

        let obj = serde_json::json!({
            "@type": "pmef:SteelMember",
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": self.unit_id(),
            "memberMark": member.name,
            "memberType": member.member_type.pmef_member_type(),
            "profileId": member.pmef_profile_id(),
            "startPoint": [member.start_point.x, member.start_point.y, member.start_point.z],
            "endPoint":   [member.end_point.x,   member.end_point.y,   member.end_point.z],
            "rollAngle":  member.roll_angle_deg,
            "material": {
                "grade":    grade,
                "standard": standard,
                "fy":       member.fy_mpa(),
                "fu":       fu_for_grade(grade)
            },
            "weight": member.mass_kg,
            "customAttributes": {
                "inventorOccurrencePath": member.occurrence_path,
                "sectionName":           member.section_name,
                "sectionStandard":       member.section_standard,
                "vaultNumber":           member.vault_number,
                "inventorParameters":    member.parameters
            },
            "revision": {
                "revisionId":            "r2026-01-01-001",
                "changeState":           "SHARED",
                "authoringToolObjectId": member.occurrence_path,
                "authoringTool":         "pmef-adapter-inventor 0.9.0"
            }
        });

        vec![
            obj,
            self.make_has_equivalent_in(
                &obj_id, &member.occurrence_path,
                member.vault_number.as_deref(),
            ),
        ]
    }

    // ── Tube & Pipe run ───────────────────────────────────────────────────────

    fn map_tube_run(&self, run: &InventorTubeRun) -> Vec<serde_json::Value> {
        let line_id = self.line_id(&run.run_name);
        let seg_id  = format!("{line_id}-S1");
        let pipe_id = format!("{line_id}-PIPE-001");

        let mat = run.material.as_deref()
            .map(material_to_pmef)
            .unwrap_or("ASTM A106 Gr. B");

        let sp = &run.start_point;
        let ep = &run.end_point;
        let length = run.length_mm();

        let mut result = vec![
            serde_json::json!({
                "@type": "pmef:PipingNetworkSystem",
                "@id": line_id,
                "pmefVersion": "0.9.0",
                "lineNumber": run.run_name,
                "nominalDiameter": run.dn_mm(),
                "pipeClass": run.pipe_spec,
                "fluidPhase": "LIQUID",
                "isPartOf": self.unit_id(),
                "segments": [seg_id],
                "customAttributes": { "inventorRunId": run.run_id },
                "revision": {
                    "revisionId": "r2026-01-01-001", "changeState": "SHARED",
                    "authoringTool": "pmef-adapter-inventor 0.9.0"
                }
            }),
            serde_json::json!({
                "@type": "pmef:PipingSegment",
                "@id": seg_id,
                "isPartOf": line_id,
                "segmentNumber": 1,
                "components": [pipe_id],
                "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
            }),
            serde_json::json!({
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
                    { "portId": "P1", "coordinate": [sp.x, sp.y, sp.z],
                      "nominalDiameter": run.dn_mm(), "endType": "BW" },
                    { "portId": "P2", "coordinate": [ep.x, ep.y, ep.z],
                      "nominalDiameter": run.dn_mm(), "endType": "BW" }
                ],
                "customAttributes": {
                    "outsideDiameter_mm": run.outside_diameter_mm,
                    "wallThickness_mm":   run.wall_thickness_mm,
                    "material":           mat
                },
                "revision": {
                    "revisionId": "r2026-01-01-001", "changeState": "SHARED",
                    "authoringTool": "pmef-adapter-inventor 0.9.0"
                }
            }),
        ];

        // HasEquivalentIn for the line
        result.push(self.make_has_equivalent_in(&line_id, &run.run_id, None));
        result
    }
}

// ── Steel material property lookup ────────────────────────────────────────────

fn fu_for_grade(grade: &str) -> f64 {
    match grade.to_uppercase().replace(['-', ' '], "").as_str() {
        "S235JR" | "A36"        => 360.0,
        "S275JR"                 => 430.0,
        "S355JR"                 => 490.0,
        "S420ML"                 => 520.0,
        "S460ML"                 => 550.0,
        "A992"                   => 448.0,
        "A572GR.50"              => 448.0,
        _                        => 430.0,
    }
}

impl PmefAdapter for InventorAdapter {
    fn name(&self) -> &str { "pmef-adapter-inventor" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "INVENTOR" }
    fn supported_domains(&self) -> &[&str] { &["equipment", "piping", "steel"] }
    fn conformance_level(&self) -> u8 { 2 }
    fn description(&self) -> &str {
        "Autodesk Inventor → PMEF adapter. Reads the JSON export produced by \
         InventorExporter.cs (Inventor Add-in via COM API). Maps assemblies to \
         equipment envelopes, Frame Generator members to SteelMember objects, \
         and Tube & Pipe runs to piping networks. Nozzle work points named \
         PMEF_NOZZLE_* become PMEF nozzles. PMEF_TAG/CLASS/DESIGN_PRESSURE \
         parameters drive the equipment mapping. Level 2 conformance."
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> InventorConfig {
        InventorConfig {
            project_code: "test".to_owned(),
            export_path: PathBuf::from("nonexistent.json"),
            ..Default::default()
        }
    }

    fn mock_export() -> InventorExport {
        InventorExport {
            schema_version: "1.0".to_owned(),
            inventor_version: "Inventor 2024".to_owned(),
            exported_at: "2026-03-31T00:00:00Z".to_owned(),
            assembly_name: "EAF-LINE3-ASSY".to_owned(),
            assembly_file: "EAF-LINE3-ASSY.iam".to_owned(),
            vault_number: None,
            coordinate_unit: "MM".to_owned(),
            assemblies: vec![mock_assembly()],
            parts: vec![],
            frame_members: vec![mock_frame_member()],
            tube_runs: vec![mock_tube_run()],
            nozzle_points: vec![mock_nozzle_point()],
            summary: InventorExportSummary {
                assembly_count: 1, part_count: 0,
                frame_member_count: 1, tube_run_count: 1,
            },
        }
    }

    fn mock_assembly() -> InventorAssembly {
        InventorAssembly {
            occurrence_path: "EAF-LINE3-ASSY:P-201A:1".to_owned(),
            name: "P-201A".to_owned(),
            iam_file: "P-201A.iam".to_owned(),
            vault_number: Some("INV-P201A-001".to_owned()),
            iproperties: InvProperties {
                part_number: Some("P-201A".to_owned()),
                description: Some("Cooling water pump".to_owned()),
                material: Some("Carbon Steel".to_owned()),
                mass_kg: Some(1850.0),
                ..Default::default()
            },
            parameters: Default::default(),
            pmef_tag: Some("P-201A".to_owned()),
            pmef_class: Some("PUMP".to_owned()),
            pmef_design_pressure_barg: Some(15.0),
            pmef_design_temp_degc: Some(60.0),
            pmef_design_code: Some("API 610".to_owned()),
            bounding_box: Some(InvBbox {
                x_min: 10050., x_max: 10450.,
                y_min: 5200.,  y_max: 5700.,
                z_min: 700.,   z_max: 1600.,
            }),
            transform: InvTransform::identity(),
            step_file: None,
            nozzle_work_points: vec![],
            is_iassembly: false,
            ipart_info: None,
            parent_path: None,
            child_paths: vec![],
        }
    }

    fn mock_frame_member() -> InventorFrameMember {
        InventorFrameMember {
            occurrence_path: "EAF-LINE3-ASSY:Frame:BM-101:1".to_owned(),
            name: "BM-101".to_owned(),
            member_type: FrameMemberType::Beam,
            section_name: "HEA 200".to_owned(),
            section_standard: "ISO".to_owned(),
            length_mm: 6000.0,
            start_point: InvPoint { x: 0., y: 0., z: 6000. },
            end_point:   InvPoint { x: 6000., y: 0., z: 6000. },
            roll_angle_deg: 0.0,
            material: "S355JR".to_owned(),
            mass_kg: Some(126.0),
            vault_number: None,
            parameters: Default::default(),
        }
    }

    fn mock_tube_run() -> InventorTubeRun {
        InventorTubeRun {
            run_id: "TR-CW-201".to_owned(),
            run_name: "CW-201".to_owned(),
            nominal_diameter_in: 8.0,
            pipe_spec: Some("A1A2".to_owned()),
            outside_diameter_mm: 219.1,
            wall_thickness_mm: 8.18,
            start_point: InvPoint { x: 0., y: 0., z: 850. },
            end_point:   InvPoint { x: 2500., y: 0., z: 850. },
            route_points: vec![],
            material: Some("A106B".to_owned()),
        }
    }

    fn mock_nozzle_point() -> InventorNozzlePoint {
        InventorNozzlePoint {
            work_point_name: "PMEF_NOZZLE_N1".to_owned(),
            nozzle_mark: "N1".to_owned(),
            parent_occurrence_path: "EAF-LINE3-ASSY:P-201A:1".to_owned(),
            position: InvPoint { x: 10200., y: 5400., z: 850. },
            direction: [-1., 0., 0.],
            nominal_diameter_mm: Some(203.2),
            flange_rating: Some("150".to_owned()),
            facing_type: Some("RF".to_owned()),
            service: Some("Suction".to_owned()),
        }
    }

    #[test]
    fn test_map_assembly_basic() {
        let export = mock_export();
        let mut adapter = InventorAdapter::new(test_config());
        let asm = &export.assemblies[0];
        adapter.path_to_id.insert(
            asm.occurrence_path.clone(),
            adapter.assembly_id(asm.tag_number(), &asm.occurrence_path),
        );
        let objs = adapter.map_assembly(asm, &export);
        assert_eq!(objs.len(), 2);
        let eq = &objs[0];
        assert_eq!(eq["@type"], "pmef:Pump");
        assert_eq!(eq["equipmentBasic"]["tagNumber"], "P-201A");
        assert_eq!(eq["equipmentBasic"]["equipmentClass"], "CENTRIFUGAL_PUMP");
        assert_eq!(eq["equipmentBasic"]["designCode"], "API 610");
        assert_eq!(eq["customAttributes"]["vaultNumber"], "INV-P201A-001");
        let dp = eq["customAttributes"]["designPressure_Pa"].as_f64().unwrap();
        assert!((dp - 1_601_325.).abs() < 10., "Got {dp}");
        let dt = eq["customAttributes"]["designTemperature_K"].as_f64().unwrap();
        assert!((dt - 333.15).abs() < 0.01, "Got {dt}");
    }

    #[test]
    fn test_map_assembly_nozzle() {
        let export = mock_export();
        let mut adapter = InventorAdapter::new(test_config());
        let asm = &export.assemblies[0];
        adapter.path_to_id.insert(
            asm.occurrence_path.clone(),
            adapter.assembly_id(asm.tag_number(), &asm.occurrence_path),
        );
        let objs = adapter.map_assembly(asm, &export);
        let nozzles = objs[0]["nozzles"].as_array().unwrap();
        assert_eq!(nozzles.len(), 1);
        assert_eq!(nozzles[0]["nozzleId"], "N1");
        assert_eq!(nozzles[0]["service"], "Suction");
        assert!((nozzles[0]["nominalDiameter"].as_f64().unwrap() - 203.2).abs() < 0.1);
        let coord = nozzles[0]["coordinate"].as_array().unwrap();
        assert!((coord[0].as_f64().unwrap() - 10200.).abs() < 0.1);
    }

    #[test]
    fn test_map_assembly_has_equivalent_in() {
        let export = mock_export();
        let mut adapter = InventorAdapter::new(test_config());
        let asm = &export.assemblies[0];
        adapter.path_to_id.insert(
            asm.occurrence_path.clone(),
            adapter.assembly_id(asm.tag_number(), &asm.occurrence_path),
        );
        let objs = adapter.map_assembly(asm, &export);
        let rel = &objs[1];
        assert_eq!(rel["@type"], "pmef:HasEquivalentIn");
        assert_eq!(rel["targetSystem"], "INVENTOR");
        assert_eq!(rel["targetSystemId"], "EAF-LINE3-ASSY:P-201A:1");
        assert_eq!(rel["vaultNumber"], "INV-P201A-001");
        assert_eq!(rel["confidence"], 1.0);
    }

    #[test]
    fn test_map_frame_member() {
        let mut adapter = InventorAdapter::new(test_config());
        let member = mock_frame_member();
        let objs = adapter.map_frame_member(&member);
        assert_eq!(objs.len(), 2);
        let sm = &objs[0];
        assert_eq!(sm["@type"], "pmef:SteelMember");
        assert_eq!(sm["memberType"], "BEAM");
        assert_eq!(sm["profileId"], "EN:HEA200");
        assert!((sm["material"]["fy"].as_f64().unwrap() - 355.).abs() < 0.1);
        assert!((sm["material"]["fu"].as_f64().unwrap() - 490.).abs() < 0.1);
        assert_eq!(sm["material"]["grade"], "S355JR");
        assert_eq!(sm["material"]["standard"], "EN 10025-2");
    }

    #[test]
    fn test_map_tube_run() {
        let adapter = InventorAdapter::new(test_config());
        let run = mock_tube_run();
        let objs = adapter.map_tube_run(&run);
        assert!(objs.len() >= 4);
        let pns = &objs[0];
        assert_eq!(pns["@type"], "pmef:PipingNetworkSystem");
        assert_eq!(pns["lineNumber"], "CW-201");
        assert!((pns["nominalDiameter"].as_f64().unwrap() - 203.2).abs() < 0.1);
        let pipe = &objs[2];
        assert_eq!(pipe["@type"], "pmef:Pipe");
        assert!((pipe["pipeLength"].as_f64().unwrap() - 2500.).abs() < 1.);
        assert_eq!(pipe["customAttributes"]["material"], "ASTM A106 Gr. B");
    }

    #[test]
    fn test_tagged_only_filter() {
        let config = InventorConfig { tagged_only: true, ..test_config() };
        let adapter = InventorAdapter::new(config);
        let mut asm = mock_assembly();
        asm.pmef_tag = None;
        // With tagged_only=true and no PMEF_TAG, assembly should be skipped
        assert!(adapter.config.tagged_only);
        assert!(asm.pmef_tag.is_none());
    }

    #[test]
    fn test_diagonal_filter() {
        let config = InventorConfig { min_diagonal_mm: 10000., ..test_config() };
        let adapter = InventorAdapter::new(config);
        let asm = mock_assembly();
        let diag = asm.bounding_box.as_ref().unwrap().diagonal();
        // 400² + 500² + 900² → sqrt = ~1089 mm, < 10000 → should be skipped
        assert!(diag < adapter.config.min_diagonal_mm);
    }

    #[test]
    fn test_make_header_objects() {
        let adapter = InventorAdapter::new(test_config());
        let export = mock_export();
        let hdrs = adapter.make_header_objects(&export, "test");
        assert_eq!(hdrs.len(), 3);
        assert_eq!(hdrs[0]["@type"], "pmef:FileHeader");
        assert_eq!(hdrs[1]["@type"], "pmef:Plant");
        assert_eq!(hdrs[2]["@type"], "pmef:Unit");
        assert_eq!(hdrs[0]["coordinateSystem"], "Z-up");
    }

    #[test]
    fn test_adapter_trait() {
        let adapter = InventorAdapter::new(test_config());
        assert_eq!(adapter.name(), "pmef-adapter-inventor");
        assert_eq!(adapter.target_system(), "INVENTOR");
        assert_eq!(adapter.conformance_level(), 2);
        assert!(adapter.supported_domains().contains(&"equipment"));
        assert!(adapter.supported_domains().contains(&"steel"));
        assert!(adapter.supported_domains().contains(&"piping"));
    }
}
