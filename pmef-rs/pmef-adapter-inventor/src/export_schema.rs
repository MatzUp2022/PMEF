//! Autodesk Inventor JSON export data types.
//!
//! Inventor stores mechanical assemblies in `.iam` (assembly) and `.ipt` (part)
//! files. Engineering data is stored in:
//!
//! - **iProperties** — document-level metadata (PartNumber, Description, Material, etc.)
//! - **Parameters** — model parameters (user-defined numeric/string values)
//! - **iLogic Rules** — VBA/iLogic code that can read/write parameters
//! - **Content Center** — standard parts library (bolts, flanges, structural profiles)
//! - **Frame Generator** — structural steel frames (beams, columns)
//! - **Tube & Pipe** — piping and tubing runs
//! - **Stress Analysis** — FEA results (Von Mises, displacement)
//!
//! The .NET/COM add-in (`InventorExporter.cs`) reads these and writes
//! the structured JSON export consumed by this Rust crate.
//!
//! ## SMS Group context
//!
//! Inventor is used at SMS Group for:
//! - Mechanical equipment design (detailed parts and assemblies)
//! - Frame and structural steel design (Frame Generator)
//! - Hydraulic manifold blocks
//! - Auxiliary equipment (cable trays, platforms, ladders)
//! - Integration with Vault PDM and Autodesk Construction Cloud

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Export root
// ─────────────────────────────────────────────────────────────────────────────

/// Root of the Inventor JSON export.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorExport {
    pub schema_version: String,
    pub inventor_version: String,
    pub exported_at: String,
    pub assembly_name: String,
    pub assembly_file: String,
    /// Vault document number (if using Vault PDM).
    pub vault_number: Option<String>,
    /// Coordinate unit (Inventor always uses cm internally; export in mm).
    pub coordinate_unit: String,
    #[serde(default)]
    pub assemblies: Vec<InventorAssembly>,
    #[serde(default)]
    pub parts: Vec<InventorPart>,
    #[serde(default)]
    pub frame_members: Vec<InventorFrameMember>,
    #[serde(default)]
    pub tube_runs: Vec<InventorTubeRun>,
    #[serde(default)]
    pub nozzle_points: Vec<InventorNozzlePoint>,
    pub summary: InventorExportSummary,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorExportSummary {
    pub assembly_count: u32,
    pub part_count: u32,
    pub frame_member_count: u32,
    pub tube_run_count: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Core geometry
// ─────────────────────────────────────────────────────────────────────────────

/// 3D point [mm, world CS].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct InvPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl InvPoint {
    pub fn distance_to(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
}

/// Axis-aligned bounding box [mm].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InvBbox {
    pub x_min: f64, pub x_max: f64,
    pub y_min: f64, pub y_max: f64,
    pub z_min: f64, pub z_max: f64,
}

impl InvBbox {
    pub fn volume(&self) -> f64 {
        (self.x_max - self.x_min).max(0.0) *
        (self.y_max - self.y_min).max(0.0) *
        (self.z_max - self.z_min).max(0.0)
    }
    pub fn centre(&self) -> InvPoint {
        InvPoint {
            x: (self.x_min + self.x_max) / 2.0,
            y: (self.y_min + self.y_max) / 2.0,
            z: (self.z_min + self.z_max) / 2.0,
        }
    }
    pub fn diagonal(&self) -> f64 {
        let dx = self.x_max - self.x_min;
        let dy = self.y_max - self.y_min;
        let dz = self.z_max - self.z_min;
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
}

/// 4×4 world transform matrix (row-major). Last column is [0,0,0,1].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InvTransform {
    /// 3×3 rotation matrix (rows).
    pub rotation: [[f64; 3]; 3],
    /// Translation vector [mm].
    pub translation: [f64; 3],
}

impl InvTransform {
    pub fn identity() -> Self {
        Self {
            rotation: [[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]],
            translation: [0., 0., 0.],
        }
    }
    /// Returns true if this is effectively the identity transform.
    pub fn is_identity(&self) -> bool {
        let r = &self.rotation;
        (r[0][0]-1.).abs() < 1e-6 && r[0][1].abs() < 1e-6 && r[0][2].abs() < 1e-6 &&
        r[1][0].abs() < 1e-6 && (r[1][1]-1.).abs() < 1e-6 && r[1][2].abs() < 1e-6 &&
        r[2][0].abs() < 1e-6 && r[2][1].abs() < 1e-6 && (r[2][2]-1.).abs() < 1e-6 &&
        self.translation.iter().all(|&v| v.abs() < 1e-6)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Assembly
// ─────────────────────────────────────────────────────────────────────────────

/// iProperties metadata from an Inventor document.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InvProperties {
    pub part_number: Option<String>,
    pub description: Option<String>,
    pub revision: Option<String>,
    pub designer: Option<String>,
    pub material: Option<String>,
    pub mass_kg: Option<f64>,
    pub surface_area_m2: Option<f64>,
    pub vendor: Option<String>,
    pub project: Option<String>,
    pub cost: Option<f64>,
    pub stock_number: Option<String>,
}

/// A named work point (coordinate system marker) in an Inventor assembly.
/// Used for nozzle connection points.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvWorkPoint {
    pub name: String,
    pub position: InvPoint,
    pub x_axis: [f64; 3],
    pub z_axis: [f64; 3],
    /// Optional parameters attached to this work point.
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

/// iPart/iAssembly member info (if this assembly is an iPart factory member).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvIPartInfo {
    pub factory_file: String,
    pub row_number: u32,
    pub member_name: String,
    #[serde(default)]
    pub table_values: HashMap<String, String>,
}

/// An Inventor assembly occurrence in the model.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorAssembly {
    /// Inventor OccurrencePath (unique within assembly).
    pub occurrence_path: String,
    /// Inventor internal name.
    pub name: String,
    /// Source .iam filename (without path).
    pub iam_file: String,
    /// Vault document number.
    pub vault_number: Option<String>,
    /// iProperties of the assembly document.
    pub iproperties: InvProperties,
    /// User parameters (d0, d1, … or user-defined names).
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
    /// Plant engineering tag (from parameter `PMEF_TAG`).
    pub pmef_tag: Option<String>,
    /// Equipment class (from parameter `PMEF_CLASS`).
    pub pmef_class: Option<String>,
    /// Design pressure [bar g] from parameter `PMEF_DESIGN_PRESSURE`.
    pub pmef_design_pressure_barg: Option<f64>,
    /// Design temperature [°C] from parameter `PMEF_DESIGN_TEMP`.
    pub pmef_design_temp_degc: Option<f64>,
    /// Design code from parameter `PMEF_DESIGN_CODE`.
    pub pmef_design_code: Option<String>,
    /// Bounding box in world CS [mm].
    pub bounding_box: Option<InvBbox>,
    /// Transform from occurrence CS to world CS.
    pub transform: InvTransform,
    /// STEP file path (relative to export dir).
    pub step_file: Option<String>,
    /// Work points named `PMEF_NOZZLE_*`.
    #[serde(default)]
    pub nozzle_work_points: Vec<InvWorkPoint>,
    /// True if adaptive (iAssembly).
    pub is_iassembly: bool,
    pub ipart_info: Option<InvIPartInfo>,
    /// Parent occurrence path.
    pub parent_path: Option<String>,
    /// Child occurrence paths.
    #[serde(default)]
    pub child_paths: Vec<String>,
}

impl InventorAssembly {
    /// Tag number — PMEF_TAG parameter, then PartNumber iProperty, then name.
    pub fn tag_number(&self) -> &str {
        self.pmef_tag.as_deref()
            .or(self.iproperties.part_number.as_deref())
            .unwrap_or(&self.name)
    }

    /// Equipment class — PMEF_CLASS parameter or None.
    pub fn equipment_class(&self) -> Option<&str> {
        self.pmef_class.as_deref()
    }

    /// Design pressure in Pa absolute.
    pub fn design_pressure_pa(&self) -> Option<f64> {
        self.pmef_design_pressure_barg.map(|b| b * 100_000.0 + 101_325.0)
    }

    /// Design temperature in K.
    pub fn design_temp_k(&self) -> Option<f64> {
        self.pmef_design_temp_degc.map(|c| c + 273.15)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Part
// ─────────────────────────────────────────────────────────────────────────────

/// An Inventor part occurrence.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorPart {
    pub occurrence_path: String,
    pub name: String,
    pub ipt_file: String,
    pub vault_number: Option<String>,
    pub iproperties: InvProperties,
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
    pub bounding_box: Option<InvBbox>,
    pub transform: InvTransform,
    pub step_file: Option<String>,
    pub parent_path: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Frame Generator members
// ─────────────────────────────────────────────────────────────────────────────

/// Frame Generator member type.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FrameMemberType {
    Beam,
    Column,
    Brace,
    Truss,
    Other,
}

impl FrameMemberType {
    pub fn pmef_member_type(&self) -> &'static str {
        match self {
            Self::Beam   => "BEAM",
            Self::Column => "COLUMN",
            Self::Brace  => "BRACE",
            Self::Truss  => "BRACE",
            Self::Other  => "GENERIC",
        }
    }
}

/// A structural member from Inventor Frame Generator.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorFrameMember {
    pub occurrence_path: String,
    pub name: String,
    pub member_type: FrameMemberType,
    /// Frame Generator section name (e.g. `"HEA 200"`, `"W12x53"`, `"SHS 100x6"`).
    pub section_name: String,
    /// Section standard / library (e.g. `"ISO"`, `"ANSI"`, `"BS"`).
    pub section_standard: String,
    /// Member length [mm].
    pub length_mm: f64,
    /// Start point in world CS [mm].
    pub start_point: InvPoint,
    /// End point in world CS [mm].
    pub end_point: InvPoint,
    /// Roll angle around member axis [degrees].
    pub roll_angle_deg: f64,
    /// Material grade (from iProperties or Content Center).
    pub material: String,
    /// Mass [kg].
    pub mass_kg: Option<f64>,
    /// Vault number.
    pub vault_number: Option<String>,
    /// Custom parameters.
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

impl InventorFrameMember {
    /// Parse the Inventor section name to a PMEF profile ID.
    ///
    /// Inventor uses spaces (e.g. `"HEA 200"`) which must be normalised.
    pub fn pmef_profile_id(&self) -> String {
        let standard = match self.section_standard.to_uppercase().as_str() {
            "ISO" | "EN" | "DIN" | "EUROPEAN" => "EN",
            "ANSI" | "AISC" | "US"            => "AISC",
            "BS" | "BRITISH"                   => "BS",
            "AS" | "AUSTRALIAN"                => "AS",
            _                                  => "EN",
        };
        // Remove spaces from section name for PMEF convention
        let section = self.section_name.replace(' ', "");
        format!("{standard}:{section}")
    }

    /// Material properties lookup.
    pub fn fy_mpa(&self) -> f64 {
        match self.material.to_uppercase().replace(['-',' '], "").as_str() {
            "S235" | "S235JR"  => 235.0,
            "S275" | "S275JR"  => 275.0,
            "S355" | "S355JR"  => 355.0,
            "S420" | "S420ML"  => 420.0,
            "S460" | "S460ML"  => 460.0,
            "A36"              => 235.0,
            "A572GR50" | "50"  => 345.0,
            "A992"             => 345.0,
            _                  => 275.0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tube & Pipe
// ─────────────────────────────────────────────────────────────────────────────

/// A Tube & Pipe run segment from Inventor.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorTubeRun {
    pub run_id: String,
    pub run_name: String,
    /// Nominal diameter [inches] — Inventor Tube & Pipe uses inches.
    pub nominal_diameter_in: f64,
    /// Pipe spec name.
    pub pipe_spec: Option<String>,
    /// Outside diameter [mm].
    pub outside_diameter_mm: f64,
    /// Wall thickness [mm].
    pub wall_thickness_mm: f64,
    /// Start point [mm, world CS].
    pub start_point: InvPoint,
    /// End point [mm, world CS].
    pub end_point: InvPoint,
    /// Intermediate route points (bends).
    #[serde(default)]
    pub route_points: Vec<InvPoint>,
    pub material: Option<String>,
}

impl InventorTubeRun {
    pub fn dn_mm(&self) -> f64 { self.nominal_diameter_in * 25.4 }

    pub fn length_mm(&self) -> f64 {
        self.start_point.distance_to(&self.end_point)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Nozzle work points
// ─────────────────────────────────────────────────────────────────────────────

/// A nozzle connection point defined as an Inventor work point.
/// Convention: work points named `PMEF_NOZZLE_<mark>` define connection points.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorNozzlePoint {
    pub work_point_name: String,
    pub nozzle_mark: String,
    pub parent_occurrence_path: String,
    pub position: InvPoint,
    pub direction: [f64; 3],
    /// Nominal diameter [mm] — from parameter `NZ_DN`.
    pub nominal_diameter_mm: Option<f64>,
    /// Flange rating — from parameter `NZ_RATING`.
    pub flange_rating: Option<String>,
    /// Facing type — from parameter `NZ_FACING`.
    pub facing_type: Option<String>,
    pub service: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Class mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map an Inventor `PMEF_CLASS` parameter value to (PMEF @type, equipmentClass).
pub fn inventor_class_to_pmef(inv_class: &str) -> (&'static str, &'static str) {
    let cls = inv_class.trim().to_uppercase().replace(['-',' ','_'], "");
    match cls.as_str() {
        "PUMP" | "CENTRIFUGALPUMP"          => ("pmef:Pump",         "CENTRIFUGAL_PUMP"),
        "RECIPROCATINGPUMP" | "RECIPROCPUMP"=> ("pmef:Pump",         "RECIPROCATING_PUMP"),
        "GEARPUMP"                           => ("pmef:Pump",         "GEAR_PUMP"),
        "COMPRESSOR" | "CENTRIFCOMPRESSOR"  => ("pmef:Compressor",   "CENTRIFUGAL_COMPRESSOR"),
        "RECIPRCOMPRESSOR"                   => ("pmef:Compressor",   "RECIPROCATING_COMPRESSOR"),
        "HEATEXCHANGER" | "HX" | "HE"      => ("pmef:HeatExchanger","SHELL_AND_TUBE_HEAT_EXCHANGER"),
        "PLATEHX" | "PHX"                   => ("pmef:HeatExchanger","PLATE_HEAT_EXCHANGER"),
        "VESSEL" | "PRESSUREVESSEL"         => ("pmef:Vessel",       "PRESSURE_VESSEL"),
        "DRUM" | "KODRUM"                   => ("pmef:Vessel",       "KNOCK_OUT_DRUM"),
        "SEPARATOR"                          => ("pmef:Vessel",       "SEPARATOR"),
        "TANK" | "STORAGETANK"              => ("pmef:Tank",         "STORAGE_TANK"),
        "REACTOR"                            => ("pmef:Reactor",      "FIXED_BED_REACTOR"),
        "EAF" | "ELECTRICARCFURNACE"        => ("pmef:Reactor",      "ELECTRIC_ARC_FURNACE"),
        "CONVERTER"                          => ("pmef:Reactor",      "CONVERTER"),
        "LADLE" | "LADLEFURNACE"            => ("pmef:Reactor",      "LADLE"),
        "ROLLINGMILL" | "MILLFRAME" | "MILL"=> ("pmef:GenericEquipment","ROLLING_MILL"),
        "GEARBOX" | "DRIVE" | "GEARUNIT"   => ("pmef:GenericEquipment","GEARBOX"),
        "HPU" | "HYDRAULICUNIT"             => ("pmef:GenericEquipment","HYDRAULIC_UNIT"),
        "FILTER" | "STRAINER"               => ("pmef:Filter",       "STRAINER"),
        "YSTRAINER"                          => ("pmef:Filter",       "Y_STRAINER"),
        "TURBINE" | "STEAMTURBINE"          => ("pmef:Turbine",      "STEAM_TURBINE"),
        "COLUMN" | "TOWER"                  => ("pmef:Column",       "DISTILLATION_COLUMN"),
        _                                    => ("pmef:GenericEquipment","GENERIC"),
    }
}

/// Map an Inventor Frame Generator section name to PMEF profile standard.
pub fn inv_section_to_standard(section_standard: &str) -> &'static str {
    match section_standard.to_uppercase().as_str() {
        "ISO" | "EN" | "DIN" | "EUROPEAN" | "GERMAN" => "EN",
        "ANSI" | "AISC" | "US" | "AMERICAN"          => "AISC",
        "BS" | "BRITISH"                               => "BS",
        "AS" | "AUSTRALIAN"                            => "AS",
        _                                              => "EN",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inv_point_distance() {
        let a = InvPoint { x:0., y:0., z:0. };
        let b = InvPoint { x:3000., y:4000., z:0. };
        assert!((a.distance_to(&b) - 5000.).abs() < 0.001);
    }

    #[test]
    fn test_inv_bbox_volume() {
        let bb = InvBbox { x_min:0., x_max:1000., y_min:0., y_max:500., z_min:0., z_max:200. };
        assert!((bb.volume() - 1e8).abs() < 1.0);
    }

    #[test]
    fn test_inv_bbox_centre() {
        let bb = InvBbox { x_min:0., x_max:1000., y_min:0., y_max:500., z_min:0., z_max:200. };
        let c = bb.centre();
        assert!((c.x - 500.).abs() < 0.001);
        assert!((c.y - 250.).abs() < 0.001);
        assert!((c.z - 100.).abs() < 0.001);
    }

    #[test]
    fn test_inv_transform_identity() {
        let t = InvTransform::identity();
        assert!(t.is_identity());
        let t2 = InvTransform {
            rotation: [[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]],
            translation: [100., 0., 0.],
        };
        assert!(!t2.is_identity()); // has translation
    }

    #[test]
    fn test_assembly_pressure_temp() {
        let asm = InventorAssembly {
            occurrence_path: "Root:P-201A:1".to_owned(),
            name: "P-201A".to_owned(),
            iam_file: "P-201A.iam".to_owned(),
            vault_number: Some("INV-001".to_owned()),
            iproperties: InvProperties {
                part_number: Some("P-201A".to_owned()),
                description: Some("Cooling water pump".to_owned()),
                ..Default::default()
            },
            parameters: Default::default(),
            pmef_tag: Some("P-201A".to_owned()),
            pmef_class: Some("PUMP".to_owned()),
            pmef_design_pressure_barg: Some(15.0),
            pmef_design_temp_degc: Some(60.0),
            pmef_design_code: Some("API 610".to_owned()),
            bounding_box: None,
            transform: InvTransform::identity(),
            step_file: None,
            nozzle_work_points: vec![],
            is_iassembly: false,
            ipart_info: None,
            parent_path: None,
            child_paths: vec![],
        };
        assert_eq!(asm.tag_number(), "P-201A");
        assert_eq!(asm.equipment_class(), Some("PUMP"));
        let pa = asm.design_pressure_pa().unwrap();
        assert!((pa - 1_601_325.).abs() < 10., "Got {pa}");
        let k = asm.design_temp_k().unwrap();
        assert!((k - 333.15).abs() < 0.01, "Got {k}");
    }

    #[test]
    fn test_inventor_class_mapping() {
        let (t, c) = inventor_class_to_pmef("PUMP");
        assert_eq!(t, "pmef:Pump"); assert_eq!(c, "CENTRIFUGAL_PUMP");
        let (t, c) = inventor_class_to_pmef("EAF");
        assert_eq!(t, "pmef:Reactor"); assert_eq!(c, "ELECTRIC_ARC_FURNACE");
        let (t, c) = inventor_class_to_pmef("ROLLING_MILL");
        assert_eq!(t, "pmef:GenericEquipment"); assert_eq!(c, "ROLLING_MILL");
        let (t, c) = inventor_class_to_pmef("GEARBOX");
        assert_eq!(t, "pmef:GenericEquipment"); assert_eq!(c, "GEARBOX");
        let (t, c) = inventor_class_to_pmef("UNKNOWN");
        assert_eq!(t, "pmef:GenericEquipment"); assert_eq!(c, "GENERIC");
    }

    #[test]
    fn test_frame_member_profile_id() {
        let m = InventorFrameMember {
            occurrence_path: "Root:Frame:BM1:1".to_owned(),
            name: "BM1".to_owned(),
            member_type: FrameMemberType::Beam,
            section_name: "HEA 200".to_owned(),
            section_standard: "ISO".to_owned(),
            length_mm: 6000.0,
            start_point: InvPoint { x:0., y:0., z:6000. },
            end_point:   InvPoint { x:6000., y:0., z:6000. },
            roll_angle_deg: 0.0,
            material: "S355JR".to_owned(),
            mass_kg: Some(126.0),
            vault_number: None,
            parameters: Default::default(),
        };
        assert_eq!(m.pmef_profile_id(), "EN:HEA200");
        assert!((m.fy_mpa() - 355.0).abs() < 0.1);
    }

    #[test]
    fn test_frame_member_ansi_profile() {
        let m = InventorFrameMember {
            occurrence_path: "Root:Frame:COL1:1".to_owned(),
            name: "COL1".to_owned(),
            member_type: FrameMemberType::Column,
            section_name: "W12x53".to_owned(),
            section_standard: "ANSI".to_owned(),
            length_mm: 4000.0,
            start_point: InvPoint { x:0., y:0., z:0. },
            end_point:   InvPoint { x:0., y:0., z:4000. },
            roll_angle_deg: 0.0,
            material: "A992".to_owned(),
            mass_kg: None, vault_number: None, parameters: Default::default(),
        };
        assert_eq!(m.pmef_profile_id(), "AISC:W12x53");
        assert!((m.fy_mpa() - 345.0).abs() < 0.1);
    }

    #[test]
    fn test_tube_run_dn() {
        let run = InventorTubeRun {
            run_id: "TR-001".to_owned(), run_name: "CW-201".to_owned(),
            nominal_diameter_in: 8.0, pipe_spec: Some("A1A2".to_owned()),
            outside_diameter_mm: 219.1, wall_thickness_mm: 8.18,
            start_point: InvPoint { x:0., y:0., z:850. },
            end_point:   InvPoint { x:2500., y:0., z:850. },
            route_points: vec![], material: Some("A106B".to_owned()),
        };
        assert!((run.dn_mm() - 203.2).abs() < 0.1);
        assert!((run.length_mm() - 2500.) < 1.0);
    }
}
