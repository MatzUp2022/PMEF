//! # pmef-adapter-revit
//!
//! PMEF adapter for **Autodesk Revit** — BIM model exchange for MEP,
//! structural, and equipment elements.
//!
//! ## Architecture
//!
//! Same two-component pattern as other adapters:
//! 1. **C# Revit Add-in** (`RevitExporter.cs`) — uses `FilteredElementCollector`
//!    to read all pipe segments, fittings, accessories, mechanical equipment,
//!    structural framing, and exports JSON.
//! 2. **Rust processor** (this crate) — maps JSON → PMEF NDJSON.
//!
//! ## Revit → PMEF domain mapping
//!
//! | Revit category | PMEF types |
//! |----------------|-----------|
//! | Pipes | `pmef:PipingNetworkSystem`, `pmef:Pipe` |
//! | Pipe Fittings | `pmef:Elbow`, `pmef:Tee`, `pmef:Reducer`, `pmef:Flange` |
//! | Pipe Accessories | `pmef:Valve`, `pmef:Filter` |
//! | Mechanical Equipment | `pmef:Pump`, `pmef:HeatExchanger`, `pmef:GenericEquipment` |
//! | Structural Framing | `pmef:SteelMember` (BEAM/BRACE) |
//! | Structural Columns | `pmef:SteelMember` (COLUMN) |

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod export_schema;

pub use export_schema::*;

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the Revit adapter.
#[derive(Debug, Clone)]
pub struct RevitConfig {
    pub project_code: String,
    pub export_path: PathBuf,
    /// Minimum pipe diameter [mm] to include (filters out small domestic pipes).
    pub min_pipe_diameter_mm: f64,
    /// Group pipes by system name into PipingNetworkSystems. Default: true.
    pub group_by_system: bool,
    /// Include structural framing. Default: true.
    pub include_structural: bool,
    /// Include mechanical equipment. Default: true.
    pub include_equipment: bool,
    /// Include duct segments. Default: false.
    pub include_ducts: bool,
    pub unit_id: Option<String>,
}

impl Default for RevitConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            export_path: PathBuf::from("revit-export.json"),
            min_pipe_diameter_mm: 25.4, // 1 inch minimum
            group_by_system: true,
            include_structural: true,
            include_equipment: true,
            include_ducts: false,
            unit_id: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Adapter
// ─────────────────────────────────────────────────────────────────────────────

/// Autodesk Revit → PMEF adapter.
pub struct RevitAdapter {
    config: RevitConfig,
    counters: HashMap<String, usize>,
    uid_to_id: HashMap<String, String>,
}

impl RevitAdapter {
    pub fn new(config: RevitConfig) -> Self {
        Self { config, counters: HashMap::new(), uid_to_id: HashMap::new() }
    }

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
        let export: RevitExport = serde_json::from_str(&json)
            .map_err(|e| AdapterError::Json(e))?;

        tracing::info!(
            "Loaded Revit export: {} pipes, {} fittings, {} equipment, {} structural from '{}'",
            export.pipe_segments.len(), export.pipe_fittings.len(),
            export.mechanical_equipment.len(), export.structural_framing.len(),
            export.project_name
        );

        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        let proj = self.config.project_code.clone();
        for obj in self.make_header_objects(&export, &proj) {
            writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Mechanical Equipment
        if self.config.include_equipment {
            for equip in &export.mechanical_equipment {
                for obj in self.map_mechanical_equipment(equip) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // Piping — group by system name → PipingNetworkSystem
        if self.config.group_by_system {
            let mut systems: HashMap<String, Vec<&RevitPipeSegment>> = HashMap::new();
            for seg in &export.pipe_segments {
                if seg.diameter_mm < self.config.min_pipe_diameter_mm {
                    stats.objects_skipped += 1; continue;
                }
                let key = seg.system_name.clone().unwrap_or_else(|| {
                    format!("{}-UNNAMED", seg.system_type)
                });
                systems.entry(key).or_default().push(seg);
            }
            for (sys_name, segments) in &systems {
                for obj in self.map_pipe_system(sys_name, segments, &export) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // Pipe Fittings
        for fitting in &export.pipe_fittings {
            let (pmef_type, comp_class) = fitting.part_type.to_pmef_type();
            let obj = self.map_fitting_or_accessory(
                pmef_type, comp_class,
                fitting.element_id, &fitting.unique_id,
                &fitting.family_name, fitting.mark.as_deref(),
                fitting.position, fitting.diameter_mm,
                fitting.angle_deg, fitting.outlet_diameter_mm,
                fitting.material.as_deref(),
                fitting.system_name.as_deref(),
            );
            for o in obj { writer.write_value(&o).map_err(|e| AdapterError::Json(e.into()))?; stats.objects_ok += 1; }
        }

        // Pipe Accessories (valves)
        for acc in &export.pipe_accessories {
            let comp_class = acc.valve_class();
            let (pmef_type, _) = if comp_class == "Y_STRAINER" {
                ("pmef:Filter", comp_class)
            } else {
                ("pmef:Valve", comp_class)
            };
            let obj = self.map_fitting_or_accessory(
                pmef_type, comp_class,
                acc.element_id, &acc.unique_id,
                &acc.family_name, acc.mark.as_deref(),
                acc.position, acc.diameter_mm,
                None, None, None,
                acc.system_name.as_deref(),
            );
            for o in obj { writer.write_value(&o).map_err(|e| AdapterError::Json(e.into()))?; stats.objects_ok += 1; }
        }

        // Structural framing + columns
        if self.config.include_structural {
            for fr in &export.structural_framing {
                for obj in self.map_structural_framing(fr) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
            for col in &export.structural_columns {
                for obj in self.map_structural_column(col) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        tracing::info!("Revit export: {} ok, {} skipped in {}ms",
            stats.objects_ok, stats.objects_skipped, stats.duration_ms);
        Ok(stats)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn clean(s: &str) -> String {
        s.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_')).collect()
    }

    fn next_id(&mut self, domain: &str, local: &str) -> String {
        let n = self.counters.entry(domain.to_owned()).or_insert(0);
        *n += 1;
        let clean = Self::clean(local);
        if clean.is_empty() {
            format!("urn:pmef:{domain}:{}:{:05}", self.config.project_code, n)
        } else {
            format!("urn:pmef:{domain}:{}:{clean}", self.config.project_code)
        }
    }

    fn unit_id(&self) -> String {
        self.config.unit_id.clone()
            .unwrap_or_else(|| format!("urn:pmef:unit:{}:U-01", self.config.project_code))
    }

    fn has_equiv(&self, pmef_id: &str, revit_uid: &str, elem_id: i64) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-revit", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id, "targetId": pmef_id,
            "targetSystem": "REVIT",
            "targetSystemId": revit_uid,
            "confidence": 1.0,
            "customAttributes": { "revitElementId": elem_id },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-revit 0.9.0" }
        })
    }

    fn make_header_objects(
        &self, export: &RevitExport, proj: &str,
    ) -> Vec<serde_json::Value> {
        let proj_clean = Self::clean(&export.project_name);
        let plant_id   = format!("urn:pmef:plant:{proj}:{proj_clean}");
        vec![
            serde_json::json!({
                "@type": "pmef:FileHeader",
                "@id": format!("urn:pmef:pkg:{proj}:{proj_clean}"),
                "pmefVersion": "0.9.0",
                "plantId": plant_id,
                "projectCode": proj,
                "coordinateSystem": "Z-up",
                "units": "mm",
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringTool": format!("pmef-adapter-revit 0.9.0 / {}", export.revit_version)
            }),
            serde_json::json!({
                "@type": "pmef:Plant",
                "@id": plant_id,
                "pmefVersion": "0.9.0",
                "name": export.project_name,
                "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
            }),
            serde_json::json!({
                "@type": "pmef:Unit",
                "@id": self.unit_id(),
                "pmefVersion": "0.9.0",
                "name": export.project_name,
                "isPartOf": plant_id,
                "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
            }),
        ]
    }

    // ── Pipe system → PipingNetworkSystem ─────────────────────────────────────

    fn map_pipe_system(
        &mut self,
        sys_name: &str,
        segments: &[&RevitPipeSegment],
        _export: &RevitExport,
    ) -> Vec<serde_json::Value> {
        let line_id = format!("urn:pmef:line:{}:{}", self.config.project_code, Self::clean(sys_name));
        let seg_id  = format!("{line_id}-S1");
        let unit    = self.unit_id();

        let dn    = segments.first().map(|s| s.diameter_mm).unwrap_or(100.0);
        let pres  = segments.iter().find_map(|s| s.pressure_pa);
        let temp  = segments.iter().find_map(|s| s.temperature_k);
        let flow  = segments.iter().find_map(|s| s.flow_m3h);
        let insul = segments.iter().find_map(|s| s.insulation_type.as_deref());
        let medium= segments.first().map(|s| s.medium_code()).unwrap_or("PROC");
        let mat   = segments.iter()
            .find_map(|s| s.material.as_deref())
            .map(revit_material_to_pmef)
            .unwrap_or("ASTM A106 Gr. B");

        let mut result: Vec<serde_json::Value> = Vec::new();

        result.push(serde_json::json!({
            "@type": "pmef:PipingNetworkSystem",
            "@id": line_id,
            "pmefVersion": "0.9.0",
            "lineNumber": sys_name,
            "nominalDiameter": dn,
            "mediumCode": medium,
            "fluidPhase": "LIQUID",
            "isPartOf": unit,
            "segments": [seg_id],
            "designConditions": {
                "designPressure": pres,
                "designTemperature": temp,
                "vacuumService": false
            },
            "specification": {
                "nominalDiameter": dn,
                "material": mat,
                "corrosionAllowance": 3.0,
                "insulationType": insul.unwrap_or("NONE")
            },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-revit 0.9.0" }
        }));

        // Component IDs
        let mut comp_ids: Vec<String> = Vec::new();
        let mut comp_objs: Vec<serde_json::Value> = Vec::new();

        for seg in segments {
            let comp_id = format!("urn:pmef:obj:{}:PIPE-{}", self.config.project_code, seg.element_id);
            comp_ids.push(comp_id.clone());
            comp_objs.push(serde_json::json!({
                "@type": "pmef:Pipe",
                "@id": comp_id,
                "pmefVersion": "0.9.0",
                "isPartOf": seg_id,
                "pipeLength": seg.length_mm,
                "componentSpec": { "componentClass": "PIPE", "skey": "PIPW    " },
                "ports": [
                    { "portId": "P1", "coordinate": seg.start_point, "nominalDiameter": seg.diameter_mm, "endType": "BW" },
                    { "portId": "P2", "coordinate": seg.end_point,   "nominalDiameter": seg.diameter_mm, "endType": "BW" }
                ],
                "customAttributes": {
                    "revitElementId": seg.element_id,
                    "revitMark": seg.mark,
                    "flowM3h": seg.flow_m3h,
                    "outsideDiameterMm": seg.outside_diameter_mm,
                    "wallThicknessMm": seg.wall_thickness_mm
                },
                "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                              "authoringTool":"pmef-adapter-revit 0.9.0" }
            }));
            comp_objs.push(self.has_equiv(&comp_id, &seg.unique_id, seg.element_id));
        }

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

    // ── Fitting / Accessory ───────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn map_fitting_or_accessory(
        &self,
        pmef_type: &str, comp_class: &str,
        elem_id: i64, uid: &str,
        family_name: &str, mark: Option<&str>,
        position: [f64; 3], dn: f64,
        angle: Option<f64>, outlet_dn: Option<f64>,
        material: Option<&str>,
        system_name: Option<&str>,
    ) -> Vec<serde_json::Value> {
        let clean_id = format!("FITTING-{elem_id}");
        let obj_id = format!("urn:pmef:obj:{}:{clean_id}", self.config.project_code);

        let mut obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "componentSpec": { "componentClass": comp_class },
            "ports": [{ "portId": "P1", "coordinate": position, "nominalDiameter": dn, "endType": "BW" }],
            "customAttributes": {
                "revitElementId": elem_id,
                "revitFamily": family_name,
                "revitMark": mark,
                "systemName": system_name,
                "material": material.map(revit_material_to_pmef)
            },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-revit 0.9.0" }
        });

        match pmef_type {
            "pmef:Elbow"   => { obj["angle"] = serde_json::Value::from(angle.unwrap_or(90.0)); }
            "pmef:Reducer" => {
                obj["reducerType"]   = serde_json::Value::from("CONCENTRIC");
                obj["largeDiameter"] = serde_json::Value::from(dn);
                obj["smallDiameter"] = serde_json::Value::from(outlet_dn.unwrap_or(dn * 0.8));
            }
            "pmef:Valve"   => { if let Some(m) = mark { obj["tagNumber"] = serde_json::Value::from(m); } }
            _ => {}
        }

        vec![obj, self.has_equiv(&obj_id, uid, elem_id)]
    }

    // ── Mechanical Equipment ──────────────────────────────────────────────────

    fn map_mechanical_equipment(
        &self, equip: &RevitMechanicalEquipment,
    ) -> Vec<serde_json::Value> {
        let (pmef_type, equip_class) = equip.to_pmef_type();
        let tag = equip.mark.as_deref().unwrap_or(&equip.type_name);
        let clean_tag = Self::clean(tag);
        let obj_id = format!("urn:pmef:obj:{}:{clean_tag}", self.config.project_code);

        let bbox = equip.bounding_box_mm.as_ref().map(|bb| serde_json::json!({
            "xMin": bb.min[0], "xMax": bb.max[0],
            "yMin": bb.min[1], "yMax": bb.max[1],
            "zMin": bb.min[2], "zMax": bb.max[2]
        }));

        let obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": self.unit_id(),
            "equipmentBasic": {
                "tagNumber": tag,
                "equipmentClass": equip_class,
                "serviceDescription": equip.comments
            },
            "nozzles": [],
            "geometry": { "type": "none", "boundingBox": bbox },
            "customAttributes": {
                "revitElementId": equip.element_id,
                "revitFamily": equip.family_name,
                "revitType": equip.type_name,
                "omniclass": equip.omniclass,
                "levelName": equip.level_name,
                "designFlow_m3h": equip.design_flow_m3h,
                "power_W": equip.power_w,
                "position": equip.position,
                "rotationDeg": equip.rotation_deg
            },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-revit 0.9.0" }
        });

        vec![obj, self.has_equiv(&obj_id, &equip.unique_id, equip.element_id)]
    }

    // ── Structural framing ────────────────────────────────────────────────────

    fn map_structural_framing(
        &self, fr: &RevitStructuralFraming,
    ) -> Vec<serde_json::Value> {
        let mark_str = fr.mark.as_deref().unwrap_or(&fr.type_name);
        let obj_id = format!("urn:pmef:obj:{}:STR-{}", self.config.project_code, fr.element_id);
        let profile_id = revit_family_to_profile_id(&fr.family_name, &fr.type_name);
        let mat_pmef = fr.material.as_deref().map(revit_material_to_pmef).unwrap_or("S355JR");

        let (fy, fu, std) = steel_props(mat_pmef);

        let obj = serde_json::json!({
            "@type": "pmef:SteelMember",
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": self.unit_id(),
            "memberMark": mark_str,
            "memberType": fr.structural_usage.pmef_member_type(),
            "profileId": profile_id,
            "startPoint": fr.start_point,
            "endPoint":   fr.end_point,
            "rollAngle":  fr.rotation_deg,
            "material": { "grade": mat_pmef, "standard": std, "fy": fy, "fu": fu },
            "customAttributes": {
                "revitElementId": fr.element_id,
                "revitFamily": fr.family_name,
                "revitType": fr.type_name,
                "levelName": fr.level_name
            },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-revit 0.9.0" }
        });
        vec![obj, self.has_equiv(&obj_id, &fr.unique_id, fr.element_id)]
    }

    fn map_structural_column(
        &self, col: &RevitStructuralColumn,
    ) -> Vec<serde_json::Value> {
        let mark_str = col.mark.as_deref().unwrap_or(&col.type_name);
        let obj_id = format!("urn:pmef:obj:{}:COL-{}", self.config.project_code, col.element_id);
        let profile_id = revit_family_to_profile_id(&col.family_name, &col.type_name);
        let mat_pmef = col.material.as_deref().map(revit_material_to_pmef).unwrap_or("S355JR");
        let (fy, fu, std) = steel_props(mat_pmef);

        let obj = serde_json::json!({
            "@type": "pmef:SteelMember",
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": self.unit_id(),
            "memberMark": mark_str,
            "memberType": "COLUMN",
            "profileId": profile_id,
            "startPoint": col.base_point,
            "endPoint":   col.top_point,
            "rollAngle":  0.0,
            "material": { "grade": mat_pmef, "standard": std, "fy": fy, "fu": fu },
            "customAttributes": {
                "revitElementId": col.element_id,
                "revitFamily": col.family_name,
                "levelName": col.level_name
            },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-revit 0.9.0" }
        });
        vec![obj, self.has_equiv(&obj_id, &col.unique_id, col.element_id)]
    }
}

fn steel_props(grade: &str) -> (f64, f64, &'static str) {
    match grade.to_uppercase().replace(['-',' '], "").as_str() {
        "S235JR"       => (235., 360., "EN 10025-2"),
        "S275JR"       => (275., 430., "EN 10025-2"),
        "S355JR"       => (355., 490., "EN 10025-2"),
        "A992"         => (345., 448., "ASTM A992"),
        "A36"          => (235., 400., "ASTM A36"),
        _              => (275., 430., "EN 10025-2"),
    }
}

impl PmefAdapter for RevitAdapter {
    fn name(&self) -> &str { "pmef-adapter-revit" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "REVIT" }
    fn supported_domains(&self) -> &[&str] { &["piping", "equipment", "steel"] }
    fn conformance_level(&self) -> u8 { 2 }
    fn description(&self) -> &str {
        "Autodesk Revit → PMEF adapter. Maps MEP pipes, fittings, accessories, \
         mechanical equipment, structural framing and columns to PMEF. \
         Pipe segments are grouped by system name into PipingNetworkSystem objects. \
         Level 2 conformance."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RevitConfig {
        RevitConfig { project_code: "test".to_owned(),
            export_path: PathBuf::from("x.json"), ..Default::default() }
    }

    #[test]
    fn test_map_pipe_system() {
        let mut adapter = RevitAdapter::new(test_config());
        let seg = RevitPipeSegment {
            element_id: 101, unique_id: "UID-101".to_owned(),
            system_type: "ProcessPipe".to_owned(),
            system_name: Some("CW-201".to_owned()),
            diameter_mm: 200., outside_diameter_mm: Some(219.1),
            wall_thickness_mm: Some(8.18),
            material: Some("A106B".to_owned()),
            segment_type: "Standard".to_owned(),
            start_point: [0.,0.,850.], end_point: [2500.,0.,850.],
            length_mm: 2500., level_name: None,
            pressure_pa: Some(1_601_325.), temperature_k: Some(333.15),
            flow_m3h: Some(250.), insulation_type: None,
            comments: None, mark: None, parameters: Default::default(),
        };
        let export = RevitExport {
            schema_version: "1.0".to_owned(), revit_version: "2024".to_owned(),
            exported_at: "2026-03-31T00:00:00Z".to_owned(),
            project_name: "EAF-2026".to_owned(), project_number: None,
            building_name: None, length_unit: "FEET".to_owned(),
            levels: vec![], grids: vec![], pipe_segments: vec![seg.clone()],
            pipe_fittings: vec![], pipe_accessories: vec![],
            mechanical_equipment: vec![], duct_segments: vec![],
            cable_trays: vec![], structural_columns: vec![],
            structural_framing: vec![], rooms: vec![],
            summary: RevitExportSummary { pipe_segment_count:1, fitting_count:0,
                equipment_count:0, structural_count:0, duct_count:0 },
        };
        let objs = adapter.map_pipe_system("CW-201", &[&seg], &export);
        // PipingNetworkSystem + PipingSegment + Pipe + HasEquivalentIn = 4
        assert!(objs.len() >= 4);
        assert_eq!(objs[0]["@type"], "pmef:PipingNetworkSystem");
        assert_eq!(objs[0]["lineNumber"], "CW-201");
        assert_eq!(objs[0]["nominalDiameter"], 200.0);
        let dp = objs[0]["designConditions"]["designPressure"].as_f64().unwrap();
        assert!((dp - 1_601_325.).abs() < 1.0);
        let pipe = &objs[2];
        assert_eq!(pipe["@type"], "pmef:Pipe");
        assert!((pipe["pipeLength"].as_f64().unwrap() - 2500.).abs() < 1.0);
    }

    #[test]
    fn test_map_mechanical_equipment() {
        let adapter = RevitAdapter::new(test_config());
        let equip = RevitMechanicalEquipment {
            element_id: 201, unique_id: "UID-201".to_owned(),
            family_name: "Pump - Centrifugal".to_owned(),
            type_name: "Standard".to_owned(),
            mark: Some("P-201A".to_owned()), omniclass: None,
            position: [10200.,5400.,850.], rotation_deg: 0.,
            level_name: Some("Level 1".to_owned()),
            bounding_box_mm: Some(RevitBbox { min:[10050.,5200.,700.], max:[10450.,5700.,1600.] }),
            design_flow_m3h: Some(250.), power_w: Some(55000.),
            comments: None, parameters: Default::default(),
        };
        let objs = adapter.map_mechanical_equipment(&equip);
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0]["@type"], "pmef:Pump");
        assert_eq!(objs[0]["equipmentBasic"]["tagNumber"], "P-201A");
        assert_eq!(objs[0]["equipmentBasic"]["equipmentClass"], "CENTRIFUGAL_PUMP");
        assert_eq!(objs[1]["targetSystem"], "REVIT");
    }

    #[test]
    fn test_map_structural_framing() {
        let adapter = RevitAdapter::new(test_config());
        let fr = RevitStructuralFraming {
            element_id: 301, unique_id: "UID-301".to_owned(),
            family_name: "HEA".to_owned(), type_name: "HEA200".to_owned(),
            mark: Some("B-101".to_owned()),
            material: Some("Steel".to_owned()),
            structural_usage: RevitStructuralUsage::Beam,
            start_point: [0.,0.,6000.], end_point: [6000.,0.,6000.],
            length_mm: 6000., rotation_deg: 0.,
            level_name: None, bounding_box_mm: None,
        };
        let objs = adapter.map_structural_framing(&fr);
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0]["@type"], "pmef:SteelMember");
        assert_eq!(objs[0]["memberType"], "BEAM");
        assert_eq!(objs[0]["profileId"], "EN:HEA200");
        assert_eq!(objs[0]["material"]["grade"], "S355JR");
        assert!((objs[0]["material"]["fy"].as_f64().unwrap() - 355.).abs() < 0.1);
    }

    #[test]
    fn test_adapter_trait() {
        let adapter = RevitAdapter::new(test_config());
        assert_eq!(adapter.name(), "pmef-adapter-revit");
        assert_eq!(adapter.target_system(), "REVIT");
        assert!(adapter.supported_domains().contains(&"piping"));
    }
}
