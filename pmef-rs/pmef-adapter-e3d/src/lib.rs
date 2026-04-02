//! # pmef-adapter-e3d
//!
//! PMEF adapter for **AVEVA E3D Plant Design** (previously AVEVA PDMS).
//!
//! ## Architecture
//!
//! AVEVA E3D does not have a REST API. The adapter uses two complementary
//! data sources:
//!
//! 1. **PML export** — semantic data (attributes, connectivity, materials).
//!    PML scripts (`*.pml`) are run from within E3D or AVEVA Engage to dump
//!    the plant hierarchy, line data, equipment tags, and component attributes.
//!
//! 2. **RVM export** — 3D geometry (primitives: cylinders, elbows, tees).
//!    E3D exports RVM files via the Review export dialog. The adapter parses
//!    the binary RVM format to extract bounding boxes and parametric primitives.
//!
//! ## Export pipeline
//!
//! ```text
//! E3D Plant Model
//!   │
//!   ├── PML Script Export ──→ plant.pml (hierarchical text)
//!   │                              │
//!   │                              ▼
//!   │                        PML Parser (src/pml.rs)
//!   │                              │
//!   ├── RVM Export ─────────→ plant.rvm (binary geometry)
//!   │                              │
//!   │                              ▼
//!   │                        RVM Parser (src/rvm.rs)
//!   │                              │
//!   └── Field Mapping ─────────────┤
//!                                   ▼
//!                             PMEF NDJSON
//! ```
//!
//! ## Usage
//!
//! ```rust,no_run
//! use pmef_adapter_e3d::{E3DAdapter, E3DConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = E3DConfig {
//!         project_code: "eaf-2026".to_owned(),
//!         pml_export_path: "exports/plant.pml".into(),
//!         rvm_export_path: Some("exports/plant.rvm".into()),
//!         ..Default::default()
//!     };
//!     let adapter = E3DAdapter::new(config);
//!     let stats = adapter.export_to_pmef("output.ndjson").await?;
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod mapping;
pub mod pml;
pub mod rvm;

pub use mapping::{
    classify_elbow_radius, e3d_dtyp_to_pmef, e3d_element_to_pmef,
    e3d_material_to_pmef, e3d_psup_to_support_type, skey_to_flange_type,
};
pub use pml::{parse_pml_text, PmlElement, PmlError, PmlValue, E3dElementType};
pub use rvm::{parse_rvm_bytes, RvmBbox, RvmFile, RvmGroup, RvmPrimitive};

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use std::collections::HashMap;
use std::path::PathBuf;

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for the AVEVA E3D adapter.
#[derive(Debug, Clone)]
pub struct E3DConfig {
    /// Short PMEF project code for @id generation (e.g. `"eaf-2026"`).
    pub project_code: String,
    /// Path to the PML export file.
    pub pml_export_path: PathBuf,
    /// Path to the RVM geometry export file (optional).
    pub rvm_export_path: Option<PathBuf>,
    /// Unit area / process unit ID for isPartOf references.
    pub unit_area: String,
    /// Coordinate system of the E3D model.
    /// E3D uses X=East, Y=North, Z=Up (same as PMEF) — no conversion needed.
    pub coordinate_system: E3DCoordinateSystem,
}

/// E3D coordinate system convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum E3DCoordinateSystem {
    /// Default: E3D X=East, Y=North, Z=Up. PMEF compatible.
    XEastYNorthZUp,
    /// Legacy PDMS: E3D X=East, Y=Up, Z=South.
    /// Requires Z→Y swap and Y→-Z to convert to PMEF.
    XEastYUpZSouth,
}

impl Default for E3DConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            pml_export_path: PathBuf::from("export.pml"),
            rvm_export_path: None,
            unit_area: "U-100".to_owned(),
            coordinate_system: E3DCoordinateSystem::XEastYNorthZUp,
        }
    }
}

impl E3DCoordinateSystem {
    /// Convert an E3D (x, y, z) coordinate to PMEF (x, y, z).
    pub fn to_pmef(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        match self {
            Self::XEastYNorthZUp => (x, y, z),       // no conversion needed
            Self::XEastYUpZSouth => (x, -z, y),      // swap Y and Z, negate new Y
        }
    }
}

// ── E3D Adapter ───────────────────────────────────────────────────────────────

/// AVEVA E3D → PMEF adapter.
pub struct E3DAdapter {
    config: E3DConfig,
    /// GUID (E3D DB address) → PMEF @id mapping.
    addr_to_id: HashMap<String, String>,
    /// Sequential counters per component type.
    counters: HashMap<String, usize>,
}

impl E3DAdapter {
    /// Create a new E3D adapter.
    pub fn new(config: E3DConfig) -> Self {
        Self {
            config,
            addr_to_id: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    /// Export the E3D plant model (PML + optional RVM) to a PMEF NDJSON file.
    pub async fn export_to_pmef(
        &mut self,
        output_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        use std::fs::File;
        use std::io::BufWriter;

        let t0 = std::time::Instant::now();
        let mut stats = AdapterStats::default();

        // Load PML data
        let pml_text = std::fs::read_to_string(&self.config.pml_export_path)
            .map_err(AdapterError::Io)?;
        let elements = parse_pml_text(&pml_text)
            .map_err(|e| AdapterError::Other(e.to_string()))?;
        tracing::info!("Parsed {} top-level PML elements", elements.len());

        // Load RVM geometry (optional)
        let rvm: Option<RvmFile> = if let Some(ref rvm_path) = self.config.rvm_export_path {
            match std::fs::read(rvm_path) {
                Ok(bytes) => match parse_rvm_bytes(&bytes) {
                    Ok(f) => { tracing::info!("Loaded RVM: {} primitives", f.primitive_count); Some(f) }
                    Err(e) => { tracing::warn!("Could not parse RVM: {e}"); None }
                },
                Err(e) => { tracing::warn!("Could not read RVM file: {e}"); None }
            }
        } else { None };

        // Open output
        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        // Write FileHeader + Plant + Unit
        let proj = &self.config.project_code.clone();
        writer.write_value(&self.make_file_header(proj))
            .map_err(|e| AdapterError::Json(e.into()))?;
        writer.write_value(&self.make_plant(proj))
            .map_err(|e| AdapterError::Json(e.into()))?;
        writer.write_value(&self.make_unit(proj))
            .map_err(|e| AdapterError::Json(e.into()))?;
        stats.objects_ok += 3;

        // Walk PML elements
        for elem in &elements {
            match elem.element_type {
                E3dElementType::Equipment => {
                    let objs = self.map_equipment(elem, rvm.as_ref());
                    for obj in objs {
                        writer.write_value(&obj)
                            .map_err(|e| AdapterError::Json(e.into()))?;
                        stats.objects_ok += 1;
                    }
                }
                E3dElementType::Pipe => {
                    let objs = self.map_pipe_line(elem, rvm.as_ref());
                    for obj in objs {
                        writer.write_value(&obj)
                            .map_err(|e| AdapterError::Json(e.into()))?;
                        stats.objects_ok += 1;
                    }
                }
                E3dElementType::Site | E3dElementType::Zone => {
                    // Recurse into children
                    for child in &elem.children {
                        // (simplified: a full implementation recurses)
                        tracing::debug!("Skipping zone/site child: {:?}", child.element_type);
                    }
                }
                _ => {
                    stats.objects_skipped += 1;
                }
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        tracing::info!(
            "E3D export complete: {} ok, {} failed, {} skipped in {}ms",
            stats.objects_ok, stats.objects_failed, stats.objects_skipped, stats.duration_ms
        );
        Ok(stats)
    }

    // ── @id generation ────────────────────────────────────────────────────────

    fn next_id(&mut self, domain: &str, local: &str) -> String {
        let local_clean: String = local.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        format!("urn:pmef:{domain}:{}:{local_clean}", self.config.project_code)
    }

    fn component_id(&mut self, line_clean: &str, comp_type: &str) -> String {
        let key = format!("{line_clean}-{comp_type}");
        let count = self.counters.entry(key.clone()).or_insert(0);
        *count += 1;
        format!(
            "urn:pmef:obj:{}:{line_clean}-{comp_type}-{:03}",
            self.config.project_code, count
        )
    }

    fn make_has_equivalent_in(&self, pmef_id: &str, e3d_addr: &str) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-e3d", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id,
            "targetId": pmef_id,
            "targetSystem": "AVEVA_E3D",
            "targetSystemId": e3d_addr,
            "mappingType": "EXACT",
            "derivedBy": "ADAPTER_IMPORT",
            "confidence": 1.0,
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringTool": "pmef-adapter-e3d 0.9.0"
            }
        })
    }

    // ── Fixed structure objects ───────────────────────────────────────────────

    fn make_file_header(&self, project_id: &str) -> serde_json::Value {
        serde_json::json!({
            "@type": "pmef:FileHeader",
            "@id": format!("urn:pmef:pkg:{}:{project_id}", self.config.project_code),
            "pmefVersion": "0.9.0",
            "plantId": format!("urn:pmef:plant:{}:{project_id}", self.config.project_code),
            "projectCode": self.config.project_code,
            "coordinateSystem": "Z-up",
            "units": "mm",
            "revisionId": "r2026-01-01-001",
            "changeState": "SHARED",
            "authoringTool": "pmef-adapter-e3d 0.9.0"
        })
    }

    fn make_plant(&self, project_id: &str) -> serde_json::Value {
        serde_json::json!({
            "@type": "pmef:Plant",
            "@id": format!("urn:pmef:plant:{}:{project_id}", self.config.project_code),
            "pmefVersion": "0.9.0",
            "name": project_id,
            "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
        })
    }

    fn make_unit(&self, project_id: &str) -> serde_json::Value {
        serde_json::json!({
            "@type": "pmef:Unit",
            "@id": format!("urn:pmef:unit:{}:{}-U01", self.config.project_code, project_id),
            "pmefVersion": "0.9.0",
            "name": format!("{} — Main Unit", project_id),
            "isPartOf": format!("urn:pmef:plant:{}:{project_id}", self.config.project_code),
            "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
        })
    }

    // ── Equipment mapping ─────────────────────────────────────────────────────

    fn map_equipment(
        &mut self, equi: &PmlElement, rvm: Option<&RvmFile>,
    ) -> Vec<serde_json::Value> {
        let tag = equi.attr_str("TAG").unwrap_or(equi.local_name());
        let dtyp = equi.attr_str("DTYP").unwrap_or("GENERIC");
        let (pmef_type, equip_class) = e3d_dtyp_to_pmef(dtyp);
        let obj_id = self.next_id("obj", tag);

        // Map nozzles (NOZZ children)
        let nozzles: Vec<serde_json::Value> = equi.children.iter()
            .filter(|c| c.element_type == E3dElementType::Nozzle)
            .map(|noz| {
                let pos = noz.attr("POS").and_then(|v| v.as_vec3()).unwrap_or((0.,0.,0.));
                let dir = noz.attr("HDIR").and_then(|v| v.as_vec3()).unwrap_or((0.,0.,1.));
                let (px, py, pz) = self.config.coordinate_system.to_pmef(pos.0, pos.1, pos.2);
                let (dx, dy, dz) = self.config.coordinate_system.to_pmef(dir.0, dir.1, dir.2);
                serde_json::json!({
                    "nozzleId": noz.attr_str("TAG").unwrap_or(noz.local_name()),
                    "nozzleMark": noz.attr_str("TAG"),
                    "nominalDiameter": noz.attr_f64("HBOR").unwrap_or(100.),
                    "flangeRating": noz.attr_str("NFRAT").unwrap_or("ANSI-150"),
                    "facingType": noz.attr_str("NFTYP").unwrap_or("RF"),
                    "coordinate": [px, py, pz],
                    "direction": [dx, dy, dz],
                    "connectedLineId": noz.attr_str("CREF")
                })
            }).collect();

        // Optional bounding box from RVM
        let rvm_bbox = rvm.and_then(|f| f.find_group(equi.local_name()))
            .and_then(|g| g.local_bbox());

        let geometry = match rvm_bbox {
            Some(bb) => serde_json::json!({
                "type": "mesh_ref",
                "lod": "LOD2_MEDIUM",
                "boundingBox": {
                    "xMin": bb.xmin, "xMax": bb.xmax,
                    "yMin": bb.ymin, "yMax": bb.ymax,
                    "zMin": bb.zmin, "zMax": bb.zmax
                }
            }),
            None => serde_json::json!({ "type": "none" }),
        };

        let mut result = Vec::new();
        let obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": format!("urn:pmef:unit:{}:{}-U01",
                self.config.project_code, self.config.project_code),
            "equipmentBasic": {
                "tagNumber": tag,
                "equipmentClass": equip_class,
                "serviceDescription": equi.attr_str("DESC"),
                "designCode": equi.attr_str("DESCD")
            },
            "nozzles": nozzles,
            "geometry": geometry,
            "customAttributes": {
                "e3dDbAddress": equi.db_address,
                "designPressure_Pa": equi.attr_f64("DPRES"),
                "designTemperature_K": equi.attr_f64("DTEMP"),
                "weightKg": equi.attr_f64("WGHT"),
                "designVolume_m3": equi.attr_f64("DVOL")
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": equi.db_address,
                "authoringTool": "pmef-adapter-e3d 0.9.0"
            }
        });
        result.push(obj);
        result.push(self.make_has_equivalent_in(&obj_id, &equi.db_address));
        result
    }

    // ── Piping line mapping ───────────────────────────────────────────────────

    fn map_pipe_line(
        &mut self, pipe: &PmlElement, rvm: Option<&RvmFile>,
    ) -> Vec<serde_json::Value> {
        let line_ref = pipe.attr_str("LINREF").unwrap_or(pipe.local_name());
        let line_clean: String = line_ref.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        let line_id = format!("urn:pmef:line:{}:{line_clean}", self.config.project_code);
        let seg_id  = format!("urn:pmef:seg:{}:{line_clean}-S1", self.config.project_code);

        let dn    = pipe.attr_f64("BORE").unwrap_or(100.0);
        let pres  = pipe.attr_f64("PRES");   // already in Pa in E3D export
        let temp  = pipe.attr_f64("TEMP");   // already in K
        let oppres= pipe.attr_f64("OPPRES");
        let optemp= pipe.attr_f64("OPTEMP");
        let tpres = pipe.attr_f64("TPRES");
        let spec  = pipe.attr_str("SPEC").unwrap_or("SPEC-UNKNOWN");
        let dtxr  = pipe.attr_str("DTXR");
        let mat   = pipe.attr_str("MAT").map(e3d_material_to_pmef).unwrap_or("ASTM A106 Gr. B");
        let diam  = pipe.attr_f64("DIAM");
        let wthk  = pipe.attr_f64("WTHK");
        let ca    = pipe.attr_f64("CORRA").unwrap_or(3.0);
        let insul = pipe.attr_str("INSUL").unwrap_or("NONE");

        // Walk branch children to find components
        let branches: Vec<&PmlElement> = pipe.children.iter()
            .filter(|c| c.element_type == E3dElementType::Branch)
            .collect();

        let mut comp_ids: Vec<String> = Vec::new();
        let mut all_objs: Vec<serde_json::Value> = Vec::new();

        for bran in &branches {
            for comp in &bran.children {
                let (pmef_type, comp_class) = e3d_element_to_pmef(
                    &format!("{:?}", comp.element_type),
                    comp.attr_str("SKEY"),
                );
                let short = &comp_class.chars().take(4).collect::<String>();
                let comp_id = self.component_id(&line_clean, short);
                comp_ids.push(comp_id.clone());

                let obj = self.map_component(comp, pmef_type, comp_class, &comp_id, &seg_id, rvm);
                all_objs.push(obj);
                all_objs.push(self.make_has_equivalent_in(&comp_id, &comp.db_address));
            }
        }

        // Build output
        let mut result: Vec<serde_json::Value> = Vec::new();

        result.push(serde_json::json!({
            "@type": "pmef:PipingNetworkSystem",
            "@id": line_id,
            "pmefVersion": "0.9.0",
            "lineNumber": line_ref,
            "nominalDiameter": dn,
            "pipeClass": spec.rsplit('/').next().unwrap_or(spec),
            "mediumCode": dtxr,
            "fluidPhase": "LIQUID",
            "isPartOf": format!("urn:pmef:unit:{}:{}-U01",
                self.config.project_code, self.config.project_code),
            "designConditions": {
                "designPressure": pres,
                "designTemperature": temp,
                "operatingPressure": oppres,
                "operatingTemperature": optemp,
                "testPressure": tpres,
                "testMedium": "WATER",
                "vacuumService": false
            },
            "specification": {
                "nominalDiameter": dn,
                "outsideDiameter": diam,
                "wallThickness": wthk,
                "pipeClass": spec.rsplit('/').next().unwrap_or(spec),
                "material": mat,
                "pressureRating": "ANSI-150",
                "corrosionAllowance": ca,
                "insulationType": insul
            },
            "segments": [seg_id],
            "customAttributes": { "e3dDbAddress": pipe.db_address },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": pipe.db_address,
                "authoringTool": "pmef-adapter-e3d 0.9.0"
            }
        }));

        result.push(self.make_has_equivalent_in(&line_id, &pipe.db_address));

        result.push(serde_json::json!({
            "@type": "pmef:PipingSegment",
            "@id": seg_id,
            "isPartOf": line_id,
            "segmentNumber": 1,
            "components": comp_ids,
            "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
        }));

        result.extend(all_objs);
        result
    }

    // ── Component mapping ─────────────────────────────────────────────────────

    fn map_component(
        &mut self,
        comp: &PmlElement,
        pmef_type: &str,
        comp_class: &str,
        comp_id: &str,
        seg_id: &str,
        _rvm: Option<&RvmFile>,
    ) -> serde_json::Value {
        let skey = comp.attr_str("SKEY");
        let dn   = comp.attr_f64("BORE")
            .or_else(|| comp.attr_f64("RBOR"))
            .unwrap_or(100.0);
        let pos  = comp.attr("POS").and_then(|v| v.as_vec3()).unwrap_or((0.,0.,0.));
        let (px, py, pz) = self.config.coordinate_system.to_pmef(pos.0, pos.1, pos.2);

        let mut obj = serde_json::json!({
            "@type": pmef_type,
            "@id": comp_id,
            "pmefVersion": "0.9.0",
            "isPartOf": seg_id,
            "componentSpec": {
                "componentClass": comp_class,
                "skey": skey.unwrap_or(""),
                "weight": comp.attr_f64("WGHT")
            },
            "ports": [{
                "portId": "P1",
                "coordinate": [px, py, pz],
                "nominalDiameter": dn,
                "endType": "BW"
            }],
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": comp.db_address,
                "authoringTool": "pmef-adapter-e3d 0.9.0"
            }
        });

        // Type-specific enrichment
        match pmef_type {
            "pmef:Elbow" => {
                let angle  = comp.attr_f64("ANGL").unwrap_or(90.0);
                let radius = comp.attr_f64("RADI").unwrap_or(1.5 * dn);
                let radius_enum = classify_elbow_radius(skey, radius, dn);
                obj["angle"]  = serde_json::Value::from(angle);
                obj["radius"] = serde_json::Value::from(radius_enum);
                if radius_enum == "CUSTOM" {
                    obj["radiusMm"] = serde_json::Value::from(radius);
                }
            }
            "pmef:Reducer" => {
                let is_ecc = comp_class == "REDUCER_ECCENTRIC";
                obj["reducerType"]   = serde_json::Value::from(if is_ecc { "ECCENTRIC" } else { "CONCENTRIC" });
                obj["largeDiameter"] = serde_json::Value::from(dn);
                obj["smallDiameter"] = serde_json::Value::from(
                    comp.attr_f64("BORE2").or_else(|| comp.attr_f64("RBOR2")).unwrap_or(dn * 0.8)
                );
            }
            "pmef:Flange" => {
                obj["flangeType"] = serde_json::Value::from(skey_to_flange_type(skey.unwrap_or("")));
                obj["rating"]     = serde_json::Value::from("ANSI-150");
                obj["facing"]     = serde_json::Value::from("RF");
            }
            "pmef:Gasket" => {
                obj["gasketType"]     = serde_json::Value::from("SPIRAL_WOUND");
                obj["gasketMaterial"] = serde_json::Value::from("SS316-FLEXITE");
            }
            "pmef:Valve" => {
                if let Some(tag) = comp.attr_str("TAG") {
                    obj["tagNumber"] = serde_json::Value::from(tag);
                }
            }
            "pmef:PipeSupport" => {
                let sup_type = e3d_psup_to_support_type(skey.unwrap_or("SUPRW"));
                obj["supportsMark"] = serde_json::Value::from(comp.local_name());
                obj["supportSpec"]  = serde_json::json!({ "supportType": sup_type, "attachmentType": "WELDED" });
            }
            _ => {}
        }
        obj
    }
}

impl PmefAdapter for E3DAdapter {
    fn name(&self) -> &str { "pmef-adapter-e3d" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "AVEVA_E3D" }
    fn supported_domains(&self) -> &[&str] { &["piping", "equipment", "steel"] }
    fn conformance_level(&self) -> u8 { 2 }
    fn description(&self) -> &str {
        "AVEVA E3D → PMEF adapter. Reads PML export (semantic data) and RVM binary (geometry). \
         Level 2 conformance. No live E3D connection required — works from offline exports."
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> E3DConfig {
        E3DConfig {
            project_code: "test".to_owned(),
            pml_export_path: PathBuf::from("nonexistent.pml"),
            rvm_export_path: None,
            unit_area: "U-100".to_owned(),
            coordinate_system: E3DCoordinateSystem::XEastYNorthZUp,
        }
    }

    #[test]
    fn test_coordinate_system_no_conversion() {
        let cs = E3DCoordinateSystem::XEastYNorthZUp;
        assert_eq!(cs.to_pmef(100., 200., 300.), (100., 200., 300.));
    }

    #[test]
    fn test_coordinate_system_pdms_swap() {
        // PDMS: X=E, Y=Up, Z=S → PMEF: X=E, Y=N, Z=Up
        // (x, y_up, z_south) → (x, -z_south, y_up)
        let cs = E3DCoordinateSystem::XEastYUpZSouth;
        let (px, py, pz) = cs.to_pmef(100., 850., 0.);
        assert!((px - 100.).abs() < 0.01);
        assert!((py - 0.).abs() < 0.01);
        assert!((pz - 850.).abs() < 0.01);
    }

    #[test]
    fn test_make_file_header() {
        let adapter = E3DAdapter::new(test_config());
        let hdr = adapter.make_file_header("EAF_2026");
        assert_eq!(hdr["@type"], "pmef:FileHeader");
        assert_eq!(hdr["coordinateSystem"], "Z-up");
        assert_eq!(hdr["units"], "mm");
        assert!(hdr["@id"].as_str().unwrap().contains("test"));
    }

    #[test]
    fn test_make_has_equivalent_in() {
        let adapter = E3DAdapter::new(test_config());
        let rel = adapter.make_has_equivalent_in(
            "urn:pmef:obj:test:P-201A",
            "/SITE01/ZONE-U100/EQUI-P-201A",
        );
        assert_eq!(rel["targetSystem"], "AVEVA_E3D");
        assert_eq!(rel["targetSystemId"], "/SITE01/ZONE-U100/EQUI-P-201A");
        assert_eq!(rel["confidence"], 1.0);
    }

    #[test]
    fn test_map_equipment_from_pml() {
        let pml = "\
EQUI /SITE01/ZONE-U100/EQUI-P-201A
  TAG 'P-201A'
  DTYP 'CENTRIFUGALPUMP'
  DESC 'Cooling water pump'
  WGHT 1850.0
  DPRES 1600000.0
  DTEMP 333.15
  NOZZ /SITE01/ZONE-U100/EQUI-P-201A/NOZZ1
    HBOR 200
    TAG 'SUCTION'
    POS 10200.0 5400.0 850.0
    HDIR -1.0 0.0 0.0
";
        let elements = parse_pml_text(pml).unwrap();
        assert_eq!(elements.len(), 1);
        let mut adapter = E3DAdapter::new(test_config());
        let objs = adapter.map_equipment(&elements[0], None);
        assert_eq!(objs.len(), 2); // object + HasEquivalentIn
        let eq = &objs[0];
        assert_eq!(eq["@type"], "pmef:Pump");
        assert_eq!(eq["equipmentBasic"]["tagNumber"], "P-201A");
        assert_eq!(eq["equipmentBasic"]["equipmentClass"], "CENTRIFUGAL_PUMP");
        let nozzles = eq["nozzles"].as_array().unwrap();
        assert_eq!(nozzles.len(), 1);
        assert_eq!(nozzles[0]["nozzleId"], "SUCTION");
        assert_eq!(nozzles[0]["nominalDiameter"], 200.0);
    }

    #[test]
    fn test_map_pipe_line_from_pml() {
        let pml = "\
PIPE /SITE01/ZONE-CW/PIPE-CW-201
  LINREF '8\"-CW-201-A1A2'
  BORE 200
  PRES 1600000
  TEMP 333.15
  OPPRES 600000
  OPTEMP 303.15
  TPRES 2400000
  SPEC '/SPEC-A1A2'
  DTXR CW
  DIAM 219.1
  WTHK 8.18
  MAT A106B
  BRAN /SITE01/ZONE-CW/PIPE-CW-201/BRAN1
    BORE 200
    ELBOW
      ANGL 90.0
      RBOR 200
      RADI 304.8
      POS 11500.0 5400.0 850.0
";
        let elements = parse_pml_text(pml).unwrap();
        let mut adapter = E3DAdapter::new(test_config());
        let objs = adapter.map_pipe_line(&elements[0], None);

        // Should produce: PipingNetworkSystem + HasEquivalentIn + PipingSegment + component + HasEquivalentIn
        assert!(objs.len() >= 3);
        let line = &objs[0];
        assert_eq!(line["@type"], "pmef:PipingNetworkSystem");
        assert_eq!(line["lineNumber"], "8\"-CW-201-A1A2");
        assert_eq!(line["nominalDiameter"], 200.0);
        let dp = line["designConditions"]["designPressure"].as_f64().unwrap();
        assert!((dp - 1_600_000.0).abs() < 1.0);
        let dt = line["designConditions"]["designTemperature"].as_f64().unwrap();
        assert!((dt - 333.15).abs() < 0.01);
        assert_eq!(line["specification"]["material"], "ASTM A106 Gr. B");
    }

    #[test]
    fn test_adapter_trait() {
        let adapter = E3DAdapter::new(test_config());
        assert_eq!(adapter.name(), "pmef-adapter-e3d");
        assert_eq!(adapter.target_system(), "AVEVA_E3D");
        assert_eq!(adapter.conformance_level(), 2);
        assert!(adapter.supported_domains().contains(&"piping"));
        assert!(adapter.supported_domains().contains(&"equipment"));
    }
}
