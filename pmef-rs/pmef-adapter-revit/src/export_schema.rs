//! Autodesk Revit JSON export data types.
//!
//! Revit is the primary AEC (Architecture, Engineering, Construction) BIM platform.
//! In plant engineering context it covers:
//!
//! - **Civil/Structural** — foundations, concrete structures, load-bearing elements
//! - **MEP** — Mechanical, Electrical, Plumbing (pipe systems, duct systems, cable trays)
//! - **Equipment** — generic families for process equipment placeholders
//! - **Architectural** — buildings, rooms, levels, grids
//!
//! PMEF focuses on the **MEP and Equipment** domains from Revit, which overlap
//! with piping, E&I, and equipment in the plant engineering world.
//!
//! The Revit API (C# add-in) exports data via `FilteredElementCollector`
//! and `Element.Parameters` — all in the internal Revit unit system
//! (feet for length, BTU/hr for heat, etc.), converted to PMEF units (mm, Pa, K, W).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Export root
// ─────────────────────────────────────────────────────────────────────────────

/// Root of the Revit JSON export produced by `RevitExporter.cs`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitExport {
    pub schema_version: String,
    pub revit_version: String,
    pub exported_at: String,
    pub project_name: String,
    pub project_number: Option<String>,
    pub building_name: Option<String>,
    /// Revit internal unit — always "FEET" internally; export converts to mm.
    pub length_unit: String,
    #[serde(default)]
    pub levels: Vec<RevitLevel>,
    #[serde(default)]
    pub grids: Vec<RevitGrid>,
    #[serde(default)]
    pub pipe_segments: Vec<RevitPipeSegment>,
    #[serde(default)]
    pub pipe_fittings: Vec<RevitPipeFitting>,
    #[serde(default)]
    pub pipe_accessories: Vec<RevitPipeAccessory>,
    #[serde(default)]
    pub mechanical_equipment: Vec<RevitMechanicalEquipment>,
    #[serde(default)]
    pub duct_segments: Vec<RevitDuctSegment>,
    #[serde(default)]
    pub cable_trays: Vec<RevitCableTray>,
    #[serde(default)]
    pub structural_columns: Vec<RevitStructuralColumn>,
    #[serde(default)]
    pub structural_framing: Vec<RevitStructuralFraming>,
    #[serde(default)]
    pub rooms: Vec<RevitRoom>,
    pub summary: RevitExportSummary,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitExportSummary {
    pub pipe_segment_count: u32,
    pub fitting_count: u32,
    pub equipment_count: u32,
    pub structural_count: u32,
    pub duct_count: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Building structure
// ─────────────────────────────────────────────────────────────────────────────

/// A Revit Level (floor/storey).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitLevel {
    pub element_id: i64,
    pub name: String,
    /// Elevation above project base point [mm].
    pub elevation_mm: f64,
    pub is_building_story: bool,
}

/// A Revit Grid line.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitGrid {
    pub element_id: i64,
    pub name: String,
    /// Start point [mm, world CS].
    pub start: [f64; 3],
    /// End point [mm, world CS].
    pub end: [f64; 3],
}

/// A Revit Room.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitRoom {
    pub element_id: i64,
    pub name: String,
    pub number: Option<String>,
    pub level_name: Option<String>,
    pub area_m2: Option<f64>,
    pub volume_m3: Option<f64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// MEP — Piping
// ─────────────────────────────────────────────────────────────────────────────

/// A Revit Pipe segment (from the Pipe category).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitPipeSegment {
    pub element_id: i64,
    /// Revit unique identifier (stable across saves).
    pub unique_id: String,
    /// Pipe system classification (e.g. `"HydronicSupply"`, `"ProcessPipe"`).
    pub system_type: String,
    /// System name (user-defined, e.g. `"CW-201"`).
    pub system_name: Option<String>,
    /// Nominal diameter [mm].
    pub diameter_mm: f64,
    /// Outside diameter [mm].
    pub outside_diameter_mm: Option<f64>,
    /// Wall thickness [mm].
    pub wall_thickness_mm: Option<f64>,
    /// Pipe material (Revit material name).
    pub material: Option<String>,
    /// Pipe segment type (Revit family name, e.g. `"Standard"`).
    pub segment_type: String,
    /// Start connector position [mm, world CS].
    pub start_point: [f64; 3],
    /// End connector position [mm, world CS].
    pub end_point: [f64; 3],
    /// Length [mm].
    pub length_mm: f64,
    /// Level on which the pipe is hosted.
    pub level_name: Option<String>,
    /// Design pressure [Pa].
    pub pressure_pa: Option<f64>,
    /// Design temperature [K].
    pub temperature_k: Option<f64>,
    /// Flow [m³/h].
    pub flow_m3h: Option<f64>,
    /// Insulation type.
    pub insulation_type: Option<String>,
    /// Comments parameter.
    pub comments: Option<String>,
    /// Mark (pipe tag number if set).
    pub mark: Option<String>,
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

impl RevitPipeSegment {
    /// Revit system_type → PMEF mediumCode approximation.
    pub fn medium_code(&self) -> &str {
        match self.system_type.to_uppercase().as_str() {
            s if s.contains("HYDRONIC") => "HW",
            s if s.contains("CHILLED")  => "CHW",
            s if s.contains("COOLING")  => "CW",
            s if s.contains("STEAM")    => "ST",
            s if s.contains("CONDENSE") => "CD",
            s if s.contains("DOMESTIC") => "DWS",
            s if s.contains("FIRE")     => "FW",
            s if s.contains("GAS")      => "NG",
            s if s.contains("PROCESS")  => "PROC",
            _                            => "PROC",
        }
    }
}

/// A Revit Pipe Fitting (elbow, tee, reducer, union, etc.).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitPipeFitting {
    pub element_id: i64,
    pub unique_id: String,
    pub family_name: String,
    pub type_name: String,
    pub system_name: Option<String>,
    /// Fitting part type from Revit PartType enum.
    pub part_type: RevitPartType,
    pub diameter_mm: f64,
    /// For reducers: outlet diameter [mm].
    pub outlet_diameter_mm: Option<f64>,
    /// For elbows: angle [degrees].
    pub angle_deg: Option<f64>,
    /// Position [mm, world CS].
    pub position: [f64; 3],
    pub material: Option<String>,
    pub level_name: Option<String>,
    pub mark: Option<String>,
}

/// Revit MEP part type (from `PartType` enum).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum RevitPartType {
    Elbow,
    Tee,
    Cross,
    Transition,   // reducer
    Union,
    FlangePair,
    Cap,
    MultiPort,
    Other,
}

impl RevitPartType {
    pub fn to_pmef_type(&self) -> (&'static str, &'static str) {
        match self {
            Self::Elbow      => ("pmef:Elbow",   "ELBOW"),
            Self::Tee        => ("pmef:Tee",     "TEE"),
            Self::Cross      => ("pmef:Tee",     "TEE"),
            Self::Transition => ("pmef:Reducer", "REDUCER_CONCENTRIC"),
            Self::Union      => ("pmef:Flange",  "FLANGE"),
            Self::FlangePair => ("pmef:Flange",  "FLANGE"),
            Self::Cap        => ("pmef:Flange",  "BLIND_FLANGE"),
            _                => ("pmef:Pipe",    "PIPE"),
        }
    }
}

/// A Revit Pipe Accessory (valve, strainer, etc. — inline equipment).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitPipeAccessory {
    pub element_id: i64,
    pub unique_id: String,
    pub family_name: String,
    pub type_name: String,
    pub system_name: Option<String>,
    pub diameter_mm: f64,
    pub position: [f64; 3],
    pub mark: Option<String>,
    pub comments: Option<String>,
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

impl RevitPipeAccessory {
    /// Classify accessory as PMEF valve type from family name.
    pub fn valve_class(&self) -> &'static str {
        let n = self.family_name.to_uppercase();
        if n.contains("GATE")        { return "VALVE_GATE"; }
        if n.contains("GLOBE")       { return "VALVE_GLOBE"; }
        if n.contains("BALL")        { return "VALVE_BALL"; }
        if n.contains("BUTTERFLY")   { return "VALVE_BUTTERFLY"; }
        if n.contains("CHECK")       { return "VALVE_CHECK"; }
        if n.contains("CONTROL")     { return "VALVE_CONTROL"; }
        if n.contains("SAFETY") || n.contains("RELIEF") { return "VALVE_RELIEF"; }
        if n.contains("STRAINER")    { return "Y_STRAINER"; }
        "VALVE_GATE"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MEP — Mechanical Equipment
// ─────────────────────────────────────────────────────────────────────────────

/// A Revit Mechanical Equipment element (pump, AHU, fan, etc.).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitMechanicalEquipment {
    pub element_id: i64,
    pub unique_id: String,
    pub family_name: String,
    pub type_name: String,
    /// Equipment tag (from `Mark` parameter).
    pub mark: Option<String>,
    /// OmniClass number (equipment classification).
    pub omniclass: Option<String>,
    /// Position [mm, world CS, insertion point].
    pub position: [f64; 3],
    pub rotation_deg: f64,
    pub level_name: Option<String>,
    pub bounding_box_mm: Option<RevitBbox>,
    /// Design flow [m³/h].
    pub design_flow_m3h: Option<f64>,
    /// Power [W].
    pub power_w: Option<f64>,
    pub comments: Option<String>,
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Revit-style bounding box [mm].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevitBbox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl RevitBbox {
    pub fn volume(&self) -> f64 {
        let dx = (self.max[0] - self.min[0]).max(0.0);
        let dy = (self.max[1] - self.min[1]).max(0.0);
        let dz = (self.max[2] - self.min[2]).max(0.0);
        dx * dy * dz
    }
    pub fn diagonal(&self) -> f64 {
        let dx = self.max[0] - self.min[0];
        let dy = self.max[1] - self.min[1];
        let dz = self.max[2] - self.min[2];
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
}

impl RevitMechanicalEquipment {
    /// Classify Revit Mechanical Equipment to PMEF type from family name + OmniClass.
    pub fn to_pmef_type(&self) -> (&'static str, &'static str) {
        // OmniClass 23 is mechanical equipment
        if let Some(oc) = &self.omniclass {
            if oc.starts_with("23-33") { return ("pmef:Pump", "CENTRIFUGAL_PUMP"); }
            if oc.starts_with("23-35") { return ("pmef:Compressor", "CENTRIFUGAL_COMPRESSOR"); }
            if oc.starts_with("23-37") { return ("pmef:HeatExchanger", "SHELL_AND_TUBE_HEAT_EXCHANGER"); }
            if oc.starts_with("23-41") { return ("pmef:Vessel", "PRESSURE_VESSEL"); }
        }
        let n = self.family_name.to_uppercase();
        if n.contains("PUMP")                       { ("pmef:Pump",         "CENTRIFUGAL_PUMP") }
        else if n.contains("FAN") || n.contains("AHU") || n.contains("AIR") {
            ("pmef:GenericEquipment",  "FAN")
        }
        else if n.contains("BOILER") || n.contains("HEATER") { ("pmef:Reactor", "FIRED_HEATER") }
        else if n.contains("CHILLER")               { ("pmef:HeatExchanger", "CHILLER") }
        else if n.contains("COOLING")               { ("pmef:HeatExchanger", "COOLING_TOWER") }
        else if n.contains("TANK") || n.contains("VESSEL") { ("pmef:Vessel", "PRESSURE_VESSEL") }
        else if n.contains("COMPRESSOR")            { ("pmef:Compressor",    "CENTRIFUGAL_COMPRESSOR") }
        else                                         { ("pmef:GenericEquipment", "GENERIC") }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MEP — Ducts and Cable Trays
// ─────────────────────────────────────────────────────────────────────────────

/// A Revit Duct segment.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitDuctSegment {
    pub element_id: i64,
    pub unique_id: String,
    pub system_name: Option<String>,
    pub width_mm: f64,
    pub height_mm: f64,
    pub start_point: [f64; 3],
    pub end_point: [f64; 3],
    pub length_mm: f64,
    pub level_name: Option<String>,
}

/// A Revit Cable Tray segment.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitCableTray {
    pub element_id: i64,
    pub unique_id: String,
    pub system_name: Option<String>,
    pub width_mm: f64,
    pub height_mm: f64,
    pub start_point: [f64; 3],
    pub end_point: [f64; 3],
    pub length_mm: f64,
    pub level_name: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Structural
// ─────────────────────────────────────────────────────────────────────────────

/// A Revit Structural Column.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitStructuralColumn {
    pub element_id: i64,
    pub unique_id: String,
    pub family_name: String,
    pub type_name: String,
    pub mark: Option<String>,
    pub material: Option<String>,
    pub base_point: [f64; 3],
    pub top_point: [f64; 3],
    pub length_mm: f64,
    pub level_name: Option<String>,
    pub bounding_box_mm: Option<RevitBbox>,
}

/// A Revit Structural Framing member (beam, brace, etc.).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevitStructuralFraming {
    pub element_id: i64,
    pub unique_id: String,
    pub family_name: String,
    pub type_name: String,
    pub mark: Option<String>,
    pub material: Option<String>,
    pub structural_usage: RevitStructuralUsage,
    pub start_point: [f64; 3],
    pub end_point: [f64; 3],
    pub length_mm: f64,
    pub rotation_deg: f64,
    pub level_name: Option<String>,
    pub bounding_box_mm: Option<RevitBbox>,
}

/// Revit structural usage type (from `StructuralType` enum).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum RevitStructuralUsage {
    Beam,
    Brace,
    Column,
    Girder,
    HorizontalBracing,
    KickerBracing,
    Other,
}

impl RevitStructuralUsage {
    pub fn pmef_member_type(&self) -> &'static str {
        match self {
            Self::Beam | Self::Girder            => "BEAM",
            Self::Column                          => "COLUMN",
            Self::Brace | Self::HorizontalBracing
            | Self::KickerBracing                 => "BRACE",
            _                                     => "GENERIC",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Revit profile / material helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Map a Revit structural family type name to a PMEF profile ID.
/// Revit uses a format like `"W12x53"`, `"HEA200"`, `"UC 203x203x60"`.
pub fn revit_family_to_profile_id(family_name: &str, type_name: &str) -> String {
    let s = type_name.trim().replace(" x ", "x").replace("x ", "x");
    let u = s.to_uppercase();

    if u.starts_with('W') && u.contains('X') { return format!("AISC:{s}"); }
    if u.starts_with("HSS")                  { return format!("AISC:{s}"); }
    for prefix in ["HEA","HEB","HEM","IPE","IPN","UPE","UPN","CHS","RHS","SHS","HE"] {
        if u.starts_with(prefix) { return format!("EN:{}", s.replace(' ', "")); }
    }
    if u.contains("UB") || u.contains("UC") || u.contains("EA") {
        if s.starts_with(|c: char| c.is_ascii_digit()) { return format!("BS:{s}"); }
    }
    format!("CUSTOM:{}", family_name.chars().filter(|c| c.is_alphanumeric() || *c == '-').collect::<String>())
}

/// Map a Revit material name to a PMEF material string.
pub fn revit_material_to_pmef(mat: &str) -> &str {
    match mat.trim().to_uppercase().replace(['-',' '], "").as_str() {
        "STEEL" | "STRUCTURALSTEEL" | "S355" | "S355JR"  => "S355JR",
        "S275" | "S275JR"                                  => "S275JR",
        "S235" | "S235JR"                                  => "S235JR",
        "A992" | "ASTMA992"                                => "A992",
        "COPPER" | "COPPERPIPE"                            => "Copper",
        "CPVC" | "PVC"                                     => "CPVC",
        "CARBONSTEEL" | "CS" | "A106B"                    => "ASTM A106 Gr. B",
        "STAINLESSSTEEL" | "SS316" | "316L"               => "ASTM A312 TP316L",
        "CASTIRONDUCTILE" | "DUCTILEIRON"                  => "EN-GJS-400-15",
        "CONCRETE" | "NORMALWEIGHTCONCRETE"               => "Concrete C30/37",
        _ => mat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revit_part_type_to_pmef() {
        assert_eq!(RevitPartType::Elbow.to_pmef_type(),      ("pmef:Elbow",   "ELBOW"));
        assert_eq!(RevitPartType::Tee.to_pmef_type(),        ("pmef:Tee",     "TEE"));
        assert_eq!(RevitPartType::Transition.to_pmef_type(), ("pmef:Reducer", "REDUCER_CONCENTRIC"));
        assert_eq!(RevitPartType::FlangePair.to_pmef_type(), ("pmef:Flange",  "FLANGE"));
        assert_eq!(RevitPartType::Cap.to_pmef_type(),        ("pmef:Flange",  "BLIND_FLANGE"));
    }

    #[test]
    fn test_revit_structural_usage_pmef_type() {
        assert_eq!(RevitStructuralUsage::Beam.pmef_member_type(),   "BEAM");
        assert_eq!(RevitStructuralUsage::Column.pmef_member_type(), "COLUMN");
        assert_eq!(RevitStructuralUsage::Brace.pmef_member_type(),  "BRACE");
    }

    #[test]
    fn test_revit_family_to_profile_id() {
        assert_eq!(revit_family_to_profile_id("W-Wide Flange", "W12x53"),   "AISC:W12x53");
        assert_eq!(revit_family_to_profile_id("HE", "HEA200"),              "EN:HEA200");
        assert_eq!(revit_family_to_profile_id("IPE", "IPE 300"),            "EN:IPE300");
        assert!(revit_family_to_profile_id("Custom", "MY-SECTION").contains("CUSTOM"));
    }

    #[test]
    fn test_revit_material_to_pmef() {
        assert_eq!(revit_material_to_pmef("Steel"),         "S355JR");
        assert_eq!(revit_material_to_pmef("Copper"),        "Copper");
        assert_eq!(revit_material_to_pmef("A106B"),         "ASTM A106 Gr. B");
        assert_eq!(revit_material_to_pmef("316L"),          "ASTM A312 TP316L");
        assert_eq!(revit_material_to_pmef("UNKNOWN"),       "UNKNOWN");
    }

    #[test]
    fn test_pipe_segment_medium_code() {
        let seg = RevitPipeSegment {
            element_id: 1, unique_id: "A".to_owned(),
            system_type: "HydronicSupply".to_owned(),
            system_name: None, diameter_mm: 100., outside_diameter_mm: None,
            wall_thickness_mm: None, material: None,
            segment_type: "Standard".to_owned(),
            start_point: [0.,0.,0.], end_point: [1000.,0.,0.], length_mm: 1000.,
            level_name: None, pressure_pa: None, temperature_k: None,
            flow_m3h: None, insulation_type: None, comments: None, mark: None,
            parameters: Default::default(),
        };
        assert_eq!(seg.medium_code(), "HW");
    }

    #[test]
    fn test_mechanical_equipment_pmef_type() {
        let equip = RevitMechanicalEquipment {
            element_id: 1, unique_id: "B".to_owned(),
            family_name: "Pump - Centrifugal".to_owned(),
            type_name: "P-201A".to_owned(),
            mark: Some("P-201A".to_owned()), omniclass: None,
            position: [0.,0.,0.], rotation_deg: 0., level_name: None,
            bounding_box_mm: None, design_flow_m3h: None, power_w: None,
            comments: None, parameters: Default::default(),
        };
        let (t, c) = equip.to_pmef_type();
        assert_eq!(t, "pmef:Pump"); assert_eq!(c, "CENTRIFUGAL_PUMP");
    }

    #[test]
    fn test_bbox_volume_and_diagonal() {
        let bb = RevitBbox { min: [0.,0.,0.], max: [400.,500.,900.] };
        assert!((bb.volume() - 180_000_000.).abs() < 1.0);
        let diag = bb.diagonal();
        assert!((diag - (400f64.powi(2)+500f64.powi(2)+900f64.powi(2)).sqrt()).abs() < 0.1);
    }

    #[test]
    fn test_pipe_accessory_valve_class() {
        let mk = |name: &str| RevitPipeAccessory {
            element_id: 1, unique_id: "X".to_owned(),
            family_name: name.to_owned(), type_name: "T".to_owned(),
            system_name: None, diameter_mm: 100., position: [0.,0.,0.],
            mark: None, comments: None, parameters: Default::default(),
        };
        assert_eq!(mk("Ball Valve").valve_class(),       "VALVE_BALL");
        assert_eq!(mk("Gate Valve").valve_class(),       "VALVE_GATE");
        assert_eq!(mk("Check Valve").valve_class(),      "VALVE_CHECK");
        assert_eq!(mk("Y-Strainer").valve_class(),       "Y_STRAINER");
        assert_eq!(mk("Safety Relief").valve_class(),    "VALVE_RELIEF");
    }
}
