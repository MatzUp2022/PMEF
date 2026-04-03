//! Creo Parametric JSON export data types.
//!
//! Creo Parametric is PTC's mechanical CAD system used at SMS Group for:
//! - Equipment envelope models (simplified 3D bodies for clash checking)
//! - Piping routing in 3D (Creo Piping & Cabling Extension)
//! - Structural steel frame models (Creo Advanced Framework Extension)
//! - Integration with Windchill PDM/PLM
//!
//! The Creo Toolkit (C API) plugin (`CreoExporter.c`) reads the model
//! and writes a structured JSON export consumed by this Rust crate.
//!
//! ## SMS Group specific context
//!
//! SMS Group uses Creo primarily for mechanical equipment design:
//! - Rolling mill frames and housings
//! - Furnace equipment (EAF, ladle furnace, converter)
//! - Hydraulic and cooling system components
//! - Drive and gearbox assemblies
//!
//! PMEF maps Creo assemblies to equipment envelope models with bounding
//! box geometry (LOD1) and, where available, detailed STEP geometry (LOD3).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Export root
// ─────────────────────────────────────────────────────────────────────────────

/// Root of the Creo JSON export produced by `CreoExporter.c`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreoExport {
    pub schema_version: String,
    pub creo_version: String,
    pub exported_at: String,
    /// Top-level Creo assembly name.
    pub assembly_name: String,
    /// Windchill WTPart number (if available).
    pub windchill_number: Option<String>,
    /// Coordinate unit used in the export.
    /// Creo models can be mm or inches — normalised here.
    pub coordinate_unit: CreoUnit,
    #[serde(default)]
    pub assemblies: Vec<CreoAssembly>,
    #[serde(default)]
    pub parts: Vec<CreoPart>,
    #[serde(default)]
    pub piping_segments: Vec<CreoPipingSegment>,
    #[serde(default)]
    pub nozzles: Vec<CreoNozzle>,
    pub summary: CreoExportSummary,
}

/// Unit system used in the Creo model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CreoUnit {
    Mm,
    Inches,
    Meters,
}

impl CreoUnit {
    /// Convert a length value to mm.
    pub fn to_mm(self, v: f64) -> f64 {
        match self {
            Self::Mm      => v,
            Self::Inches  => v * 25.4,
            Self::Meters  => v * 1000.0,
        }
    }
}

/// Export summary counts.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreoExportSummary {
    pub assembly_count: u32,
    pub part_count: u32,
    pub piping_segment_count: u32,
    pub nozzle_count: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Assembly (equipment envelope)
// ─────────────────────────────────────────────────────────────────────────────

/// Coordinate in the Creo world coordinate system [model units].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreoPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A transformation matrix from component CS to assembly CS.
/// Row-major 4×3 (translation in last row).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreoTransform {
    pub m: [[f64; 3]; 4],
}

impl CreoTransform {
    /// Identity transform.
    pub fn identity() -> Self {
        Self { m: [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
        ]}
    }

    /// Translation vector [mm in assembly CS].
    pub fn translation(&self) -> (f64, f64, f64) {
        (self.m[3][0], self.m[3][1], self.m[3][2])
    }
}

/// Axis-aligned bounding box in the assembly coordinate system.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreoBbox {
    pub x_min: f64, pub x_max: f64,
    pub y_min: f64, pub y_max: f64,
    pub z_min: f64, pub z_max: f64,
}

impl CreoBbox {
    pub fn volume(&self) -> f64 {
        (self.x_max - self.x_min).max(0.0) *
        (self.y_max - self.y_min).max(0.0) *
        (self.z_max - self.z_min).max(0.0)
    }

    pub fn centre(&self) -> CreoPoint {
        CreoPoint {
            x: (self.x_min + self.x_max) / 2.0,
            y: (self.y_min + self.y_max) / 2.0,
            z: (self.z_min + self.z_max) / 2.0,
        }
    }

    /// Scale bounding box from model units to mm.
    pub fn to_mm(&self, unit: CreoUnit) -> CreoBbox {
        CreoBbox {
            x_min: unit.to_mm(self.x_min), x_max: unit.to_mm(self.x_max),
            y_min: unit.to_mm(self.y_min), y_max: unit.to_mm(self.y_max),
            z_min: unit.to_mm(self.z_min), z_max: unit.to_mm(self.z_max),
        }
    }
}

/// A Creo assembly — maps to a PMEF equipment object.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreoAssembly {
    /// Creo model name (filename without .asm extension).
    pub model_name: String,
    /// Creo session ID (unique within export session).
    pub session_id: u64,
    /// Windchill WTPartNumber if available.
    pub windchill_number: Option<String>,
    /// User-readable description.
    pub description: Option<String>,
    /// Creo parameter `PLANT_TAG` — equipment tag number.
    pub plant_tag: Option<String>,
    /// Creo parameter `EQUIPMENT_CLASS`.
    pub equipment_class: Option<String>,
    /// Creo parameter `DESIGN_CODE`.
    pub design_code: Option<String>,
    /// Creo parameter `MATERIAL`.
    pub material: Option<String>,
    /// Creo parameter `WEIGHT` [model units].
    pub weight: Option<f64>,
    /// Design pressure [bar g] from Creo parameter `DESIGN_PRESSURE`.
    pub design_pressure_barg: Option<f64>,
    /// Design temperature [°C] from Creo parameter `DESIGN_TEMPERATURE`.
    pub design_temperature_degc: Option<f64>,
    /// Bounding box in assembly CS (model units).
    pub bounding_box: Option<CreoBbox>,
    /// Transform from this assembly's CS to the root assembly CS.
    pub transform_to_root: CreoTransform,
    /// Path to the STEP file for this assembly (relative to export dir).
    pub step_file: Option<String>,
    /// Child part session IDs.
    #[serde(default)]
    pub child_parts: Vec<u64>,
    /// Creo parameters (all user-defined parameters).
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

impl CreoAssembly {
    /// Returns the equipment tag number (from PLANT_TAG parameter or model_name).
    pub fn tag_number(&self) -> &str {
        self.plant_tag.as_deref().unwrap_or(&self.model_name)
    }

    /// Design pressure in Pa absolute.
    pub fn design_pressure_pa(&self) -> Option<f64> {
        self.design_pressure_barg.map(|b| b * 100_000.0 + 101_325.0)
    }

    /// Design temperature in K.
    pub fn design_temperature_k(&self) -> Option<f64> {
        self.design_temperature_degc.map(|c| c + 273.15)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Part
// ─────────────────────────────────────────────────────────────────────────────

/// A Creo part — leaf-level geometry object.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreoPart {
    pub model_name: String,
    pub session_id: u64,
    pub windchill_number: Option<String>,
    pub description: Option<String>,
    /// Creo part type: `"SOLID"`, `"SHEETMETAL"`, `"PIPING"`, `"CABLE"`.
    pub part_type: String,
    pub material: Option<String>,
    pub weight: Option<f64>,
    pub bounding_box: Option<CreoBbox>,
    pub transform_to_root: CreoTransform,
    pub step_file: Option<String>,
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Piping (Creo Piping & Cabling Extension)
// ─────────────────────────────────────────────────────────────────────────────

/// Creo Piping segment — a run of pipe with fittings.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreoPipingSegment {
    pub segment_id: String,
    /// Piping network / line name (from Creo piping network parameter).
    pub network_name: String,
    /// Nominal diameter [inches in Creo Piping standard].
    pub nominal_diameter_in: f64,
    /// Pipe spec (Creo pipe spec name).
    pub pipe_spec: String,
    /// Outside diameter [model units].
    pub outside_diameter: f64,
    /// Wall thickness [model units].
    pub wall_thickness: f64,
    /// Start point in root assembly CS [model units].
    pub start_point: CreoPoint,
    /// End point in root assembly CS [model units].
    pub end_point: CreoPoint,
    /// Pipe run points (for bends and route geometry).
    #[serde(default)]
    pub route_points: Vec<CreoPoint>,
    /// Fittings on this segment.
    #[serde(default)]
    pub fittings: Vec<CreoFitting>,
    pub material: Option<String>,
}

impl CreoPipingSegment {
    pub fn dn_mm(&self) -> f64 { self.nominal_diameter_in * 25.4 }

    pub fn length_mm(&self, unit: CreoUnit) -> f64 {
        let dx = unit.to_mm(self.end_point.x - self.start_point.x);
        let dy = unit.to_mm(self.end_point.y - self.start_point.y);
        let dz = unit.to_mm(self.end_point.z - self.start_point.z);
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
}

/// A fitting on a Creo piping segment (elbow, tee, reducer, etc.).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreoFitting {
    pub fitting_id: String,
    /// Creo fitting type string.
    pub fitting_type: String,
    /// Creo pipe spec key (SKEY equivalent).
    pub spec_key: Option<String>,
    pub nominal_diameter_in: f64,
    /// Branch diameter [inches] for tees.
    pub branch_diameter_in: Option<f64>,
    /// Elbow angle [degrees].
    pub angle: Option<f64>,
    /// Position in root assembly CS [model units].
    pub position: CreoPoint,
    pub material: Option<String>,
}

impl CreoFitting {
    pub fn dn_mm(&self) -> f64 { self.nominal_diameter_in * 25.4 }
}

// ─────────────────────────────────────────────────────────────────────────────
// Nozzle (Creo coordinate system placed at nozzle face)
// ─────────────────────────────────────────────────────────────────────────────

/// A nozzle modelled in Creo as a named coordinate system.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreoNozzle {
    /// Name of the Creo coordinate system (e.g. `"CS_NOZZLE_N1"`).
    pub cs_name: String,
    /// Parent assembly session ID.
    pub parent_assembly_id: u64,
    /// Nozzle mark (derived from CS name or parameter).
    pub nozzle_mark: String,
    /// Service (from `NZ_SERVICE` parameter).
    pub service: Option<String>,
    /// Nominal diameter [inches].
    pub nominal_diameter_in: f64,
    /// Flange rating (from `NZ_RATING` parameter, e.g. `"150"`, `"300"`).
    pub flange_rating: Option<String>,
    /// Facing type (from `NZ_FACING`, e.g. `"RF"`, `"RTJ"`).
    pub facing_type: Option<String>,
    /// Origin of the nozzle CS in root assembly CS [model units].
    pub origin: CreoPoint,
    /// Z-axis direction of the nozzle CS (outward normal).
    pub direction: [f64; 3],
}

impl CreoNozzle {
    pub fn dn_mm(&self) -> f64 { self.nominal_diameter_in * 25.4 }
}

// ─────────────────────────────────────────────────────────────────────────────
// Creo fitting type → PMEF mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map a Creo Piping fitting type string to (PMEF @type, componentClass).
pub fn creo_fitting_to_pmef(creo_type: &str) -> (&'static str, &'static str) {
    match creo_type.trim().to_uppercase().as_str() {
        "ELBOW" | "ELBOW_90" | "ELL_90LR"   => ("pmef:Elbow",   "ELBOW"),
        "ELBOW_45" | "ELL_45LR"              => ("pmef:Elbow",   "ELBOW"),
        "ELBOW_SR90" | "ELL_90SR"            => ("pmef:Elbow",   "ELBOW"),
        "TEE" | "EQUAL_TEE"                  => ("pmef:Tee",     "TEE"),
        "REDUCING_TEE"                        => ("pmef:Tee",     "TEE"),
        "REDUCER" | "CONCENTRIC_REDUCER"     => ("pmef:Reducer", "REDUCER_CONCENTRIC"),
        "ECCENTRIC_REDUCER"                   => ("pmef:Reducer", "REDUCER_ECCENTRIC"),
        "FLANGE" | "WELD_NECK_FLANGE"        => ("pmef:Flange",  "FLANGE"),
        "BLIND_FLANGE"                        => ("pmef:Flange",  "BLIND_FLANGE"),
        "GATE_VALVE" | "GATE VALVE"          => ("pmef:Valve",   "VALVE_GATE"),
        "GLOBE_VALVE" | "GLOBE VALVE"        => ("pmef:Valve",   "VALVE_GLOBE"),
        "BALL_VALVE" | "BALL VALVE"          => ("pmef:Valve",   "VALVE_BALL"),
        "BUTTERFLY_VALVE"                     => ("pmef:Valve",   "VALVE_BUTTERFLY"),
        "CHECK_VALVE" | "CHECK VALVE"        => ("pmef:Valve",   "VALVE_CHECK"),
        "CONTROL_VALVE" | "CONTROL VALVE"    => ("pmef:Valve",   "VALVE_CONTROL"),
        "GASKET"                              => ("pmef:Gasket",  "GASKET"),
        "PIPE_SUPPORT" | "SUPPORT"           => ("pmef:PipeSupport", "PIPE_SUPPORT"),
        _                                     => ("pmef:Pipe",    "PIPE"),
    }
}

/// Map a Creo equipment assembly `EQUIPMENT_CLASS` parameter to PMEF.
pub fn creo_class_to_pmef(creo_class: &str) -> (&'static str, &'static str) {
    match creo_class.trim().to_uppercase().replace(['-',' ','_'], "").as_str() {
        "PUMP" | "CENTRIFUGALPUMP"          => ("pmef:Pump",         "CENTRIFUGAL_PUMP"),
        "RECIPROCATINGPUMP"                  => ("pmef:Pump",         "RECIPROCATING_PUMP"),
        "COMPRESSOR" | "CENTRIFUGALCOMP"    => ("pmef:Compressor",   "CENTRIFUGAL_COMPRESSOR"),
        "HEATEXCHANGER" | "HX" | "HE"      => ("pmef:HeatExchanger","SHELL_AND_TUBE_HEAT_EXCHANGER"),
        "PLATEHX" | "PLATEHEATEXCHANGER"    => ("pmef:HeatExchanger","PLATE_HEAT_EXCHANGER"),
        "VESSEL" | "PRESSUREVESSEL"         => ("pmef:Vessel",       "PRESSURE_VESSEL"),
        "DRUM" | "KNOCKOUTDRUM"             => ("pmef:Vessel",       "KNOCK_OUT_DRUM"),
        "TANK" | "STORAGETANK"              => ("pmef:Tank",         "STORAGE_TANK"),
        "REACTOR" | "FURNACE"               => ("pmef:Reactor",      "FIXED_BED_REACTOR"),
        "EAF" | "ELECTRICARCFURNACE"        => ("pmef:Reactor",      "ELECTRIC_ARC_FURNACE"),
        "CONVERTER"                          => ("pmef:Reactor",      "CONVERTER"),
        "LADLE" | "LADDLEFURNACE"           => ("pmef:Reactor",      "LADLE"),
        "ROLLINGMILL" | "MILLFRAME"         => ("pmef:GenericEquipment","ROLLING_MILL"),
        "GEARBOX" | "DRIVE"                 => ("pmef:GenericEquipment","GEARBOX"),
        "HYDRAULICUNIT" | "HPU"             => ("pmef:GenericEquipment","HYDRAULIC_UNIT"),
        "COLUMN" | "TOWER"                  => ("pmef:Column",       "DISTILLATION_COLUMN"),
        "FILTER" | "STRAINER"               => ("pmef:Filter",       "STRAINER"),
        "TURBINE"                            => ("pmef:Turbine",      "STEAM_TURBINE"),
        _                                    => ("pmef:GenericEquipment","GENERIC"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creo_unit_to_mm() {
        assert!((CreoUnit::Mm.to_mm(100.0) - 100.0).abs() < 1e-9);
        assert!((CreoUnit::Inches.to_mm(1.0) - 25.4).abs() < 1e-9);
        assert!((CreoUnit::Meters.to_mm(1.0) - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_creo_bbox_volume() {
        let bb = CreoBbox { x_min:0., x_max:1000., y_min:0., y_max:500., z_min:0., z_max:200. };
        assert!((bb.volume() - 1e8).abs() < 1.0);
    }

    #[test]
    fn test_creo_bbox_centre() {
        let bb = CreoBbox { x_min:0., x_max:1000., y_min:0., y_max:500., z_min:0., z_max:200. };
        let c = bb.centre();
        assert!((c.x - 500.).abs() < 0.001);
        assert!((c.y - 250.).abs() < 0.001);
    }

    #[test]
    fn test_creo_bbox_to_mm() {
        let bb = CreoBbox { x_min:0., x_max:1., y_min:0., y_max:1., z_min:0., z_max:1. };
        let mm = bb.to_mm(CreoUnit::Inches);
        assert!((mm.x_max - 25.4).abs() < 0.001);
    }

    #[test]
    fn test_assembly_pressure_pa() {
        let asm = CreoAssembly {
            model_name: "P-201A".to_owned(), session_id: 1,
            windchill_number: None, description: None,
            plant_tag: Some("P-201A".to_owned()),
            equipment_class: Some("PUMP".to_owned()),
            design_code: None, material: None, weight: None,
            design_pressure_barg: Some(15.0),
            design_temperature_degc: Some(60.0),
            bounding_box: None,
            transform_to_root: CreoTransform::identity(),
            step_file: None, child_parts: vec![],
            parameters: Default::default(),
        };
        let pa = asm.design_pressure_pa().unwrap();
        assert!((pa - 1_601_325.0).abs() < 10.0, "Got {pa}");
        let k = asm.design_temperature_k().unwrap();
        assert!((k - 333.15).abs() < 0.01, "Got {k}");
    }

    #[test]
    fn test_creo_fitting_mapping() {
        let (t, c) = creo_fitting_to_pmef("ELBOW_90");
        assert_eq!(t, "pmef:Elbow"); assert_eq!(c, "ELBOW");
        let (t, c) = creo_fitting_to_pmef("GATE_VALVE");
        assert_eq!(t, "pmef:Valve"); assert_eq!(c, "VALVE_GATE");
        let (t, c) = creo_fitting_to_pmef("CONCENTRIC_REDUCER");
        assert_eq!(t, "pmef:Reducer"); assert_eq!(c, "REDUCER_CONCENTRIC");
        let (t, c) = creo_fitting_to_pmef("UNKNOWN");
        assert_eq!(t, "pmef:Pipe"); // fallback
    }

    #[test]
    fn test_creo_class_mapping() {
        let (t, c) = creo_class_to_pmef("PUMP");
        assert_eq!(t, "pmef:Pump"); assert_eq!(c, "CENTRIFUGAL_PUMP");
        let (t, c) = creo_class_to_pmef("EAF");
        assert_eq!(t, "pmef:Reactor"); assert_eq!(c, "ELECTRIC_ARC_FURNACE");
        let (t, c) = creo_class_to_pmef("ROLLING_MILL");
        assert_eq!(t, "pmef:GenericEquipment"); assert_eq!(c, "ROLLING_MILL");
        let (t, c) = creo_class_to_pmef("UNKNOWN");
        assert_eq!(t, "pmef:GenericEquipment"); assert_eq!(c, "GENERIC");
    }

    #[test]
    fn test_piping_segment_dn() {
        let seg = CreoPipingSegment {
            segment_id: "S1".to_owned(), network_name: "CW-201".to_owned(),
            nominal_diameter_in: 8.0, pipe_spec: "A1A2".to_owned(),
            outside_diameter: 8.625, wall_thickness: 0.322,
            start_point: CreoPoint { x:0.,y:0.,z:0. },
            end_point:   CreoPoint { x:100.,y:0.,z:0. },
            route_points: vec![], fittings: vec![], material: None,
        };
        assert!((seg.dn_mm() - 203.2).abs() < 0.1);
        assert!((seg.length_mm(CreoUnit::Inches) - 2540.0).abs() < 1.0);
    }

    #[test]
    fn test_nozzle_dn() {
        let noz = CreoNozzle {
            cs_name: "CS_NOZZLE_N1".to_owned(), parent_assembly_id: 1,
            nozzle_mark: "N1".to_owned(), service: Some("Suction".to_owned()),
            nominal_diameter_in: 8.0,
            flange_rating: Some("150".to_owned()), facing_type: Some("RF".to_owned()),
            origin: CreoPoint { x:0.,y:0.,z:0. }, direction: [-1.,0.,0.],
        };
        assert!((noz.dn_mm() - 203.2).abs() < 0.1);
    }

    #[test]
    fn test_transform_identity() {
        let t = CreoTransform::identity();
        let (tx, ty, tz) = t.translation();
        assert!((tx + ty + tz).abs() < 1e-9);
    }

    #[test]
    fn test_deserialise_export() {
        let json = r#"{
            "schemaVersion": "1.0", "creoVersion": "Creo 10.0",
            "exportedAt": "2026-03-31T00:00:00Z",
            "assemblyName": "EAF-LINE3-ASSY",
            "coordinateUnit": "MM",
            "assemblies": [], "parts": [],
            "pipingSegments": [], "nozzles": [],
            "summary": { "assemblyCount":0,"partCount":0,"pipingSegmentCount":0,"nozzleCount":0 }
        }"#;
        let export: CreoExport = serde_json::from_str(json).unwrap();
        assert_eq!(export.creo_version, "Creo 10.0");
        assert_eq!(export.coordinate_unit, CreoUnit::Mm);
    }
}
