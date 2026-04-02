//! CADMATIC REST API response data types.
//!
//! These structs mirror the JSON response shapes returned by the CADMATIC
//! Web API (Swagger schema as of CADMATIC version 2024.1).

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Project
// ─────────────────────────────────────────────────────────────────────────────

/// Response from `GET /api/v1/projects`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadmaticProject {
    pub project_id: String,
    pub project_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub plant_code: Option<String>,
    #[serde(default)]
    pub created_date: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Piping line
// ─────────────────────────────────────────────────────────────────────────────

/// Response from `GET /api/v1/projects/{projectId}/pipelines`.
///
/// One entry per piping line (CADMATIC "Pipeline" object).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadmaticLine {
    /// CADMATIC internal line GUID.
    pub line_id: String,
    /// Full line number tag (e.g. `"8\"-CW-201-A1A2"`).
    pub line_number: String,
    /// Nominal diameter [mm].
    #[serde(default)]
    pub nominal_diameter: Option<f64>,
    /// Pipe specification / class code.
    #[serde(default)]
    pub pipe_class: Option<String>,
    /// Fluid / medium code.
    #[serde(default)]
    pub fluid_code: Option<String>,
    /// Fluid description.
    #[serde(default)]
    pub fluid_description: Option<String>,
    /// Design pressure [bar g].
    #[serde(default, rename = "designPressureBarG")]
    pub design_pressure_barg: Option<f64>,
    /// Design temperature [°C].
    #[serde(default, rename = "designTemperatureDegC")]
    pub design_temperature_degc: Option<f64>,
    /// Operating pressure [bar g].
    #[serde(default, rename = "operatingPressureBarG")]
    pub operating_pressure_barg: Option<f64>,
    /// Operating temperature [°C].
    #[serde(default, rename = "operatingTemperatureDegC")]
    pub operating_temperature_degc: Option<f64>,
    /// Test pressure [bar g].
    #[serde(default, rename = "testPressureBarG")]
    pub test_pressure_barg: Option<f64>,
    /// Pipe schedule.
    #[serde(default)]
    pub schedule: Option<String>,
    /// Outside diameter [mm].
    #[serde(default, rename = "outsideDiameterMm")]
    pub outside_diameter_mm: Option<f64>,
    /// Wall thickness [mm].
    #[serde(default, rename = "wallThicknessMm")]
    pub wall_thickness_mm: Option<f64>,
    /// Material designation (CADMATIC native string).
    #[serde(default)]
    pub material: Option<String>,
    /// Insulation type code.
    #[serde(default)]
    pub insulation_type: Option<String>,
    /// P&ID sheet reference.
    #[serde(default)]
    pub pid_reference: Option<String>,
    /// DEXPI functional object reference.
    #[serde(default)]
    pub dexpi_ref: Option<String>,
    /// Number of components in this line.
    #[serde(default)]
    pub component_count: Option<u32>,
    /// Last modified timestamp.
    #[serde(default)]
    pub modified_date: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Piping component
// ─────────────────────────────────────────────────────────────────────────────

/// A 3D coordinate in CADMATIC's own format [mm].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadmaticPoint3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A connection point (port) on a CADMATIC component.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadmaticEndPoint {
    /// Connection point index (0-based).
    pub index: u32,
    /// Position [mm].
    pub position: CadmaticPoint3D,
    /// Direction vector (unit vector in CADMATIC's coordinate system).
    #[serde(default)]
    pub direction: Option<CadmaticPoint3D>,
    /// Nominal bore [mm].
    #[serde(default, rename = "boreMm")]
    pub bore_mm: Option<f64>,
    /// Connection end type (BW, FL, SW, SC).
    #[serde(default)]
    pub end_type: Option<String>,
    /// ObjectGUID of the adjacent connected component.
    #[serde(default)]
    pub connected_to_guid: Option<String>,
}

/// Response from `GET /api/v1/pipelines/{lineId}/components`.
///
/// One entry per piping component (pipe, elbow, valve, etc.).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadmaticComponent {
    /// CADMATIC object GUID (used for `HasEquivalentIn.targetSystemId`).
    pub object_guid: String,
    /// CADMATIC component type string (see mapping table in `mapping.rs`).
    pub component_type: String,
    /// Catalogue / specification key (shape key, similar to PCF SKEY).
    #[serde(default)]
    pub spec_key: Option<String>,
    /// Item number within the line.
    #[serde(default)]
    pub item_number: Option<String>,
    /// Tag number (for valves and instruments).
    #[serde(default)]
    pub tag_number: Option<String>,
    /// Material designation.
    #[serde(default)]
    pub material: Option<String>,
    /// Nominal diameter (bore) [mm].
    #[serde(default, rename = "nominalDiameterMm")]
    pub nominal_diameter_mm: Option<f64>,
    /// Connection end points (typically 2, sometimes 3 for tees).
    #[serde(default)]
    pub end_points: Vec<CadmaticEndPoint>,
    /// Component weight [kg].
    #[serde(default, rename = "weightKg")]
    pub weight_kg: Option<f64>,
    /// Catalogue entry reference.
    #[serde(default)]
    pub catalogue_ref: Option<String>,
    /// Vendor / manufacturer.
    #[serde(default)]
    pub vendor: Option<String>,
    /// Custom attributes (project-specific).
    #[serde(default)]
    pub custom_attributes: std::collections::HashMap<String, serde_json::Value>,
    /// For elbows: angle [degrees].
    #[serde(default, rename = "angleDeg")]
    pub angle_deg: Option<f64>,
    /// For elbows: bend radius [mm].
    #[serde(default, rename = "bendRadiusMm")]
    pub bend_radius_mm: Option<f64>,
    /// For reducers: large end bore [mm].
    #[serde(default, rename = "largeBoreMm")]
    pub large_bore_mm: Option<f64>,
    /// For reducers: small end bore [mm].
    #[serde(default, rename = "smallBoreMm")]
    pub small_bore_mm: Option<f64>,
    /// For valves: actuator type.
    #[serde(default)]
    pub actuator_type: Option<String>,
    /// For valves: fail position.
    #[serde(default)]
    pub fail_position: Option<String>,
    /// Weld number (for welds).
    #[serde(default)]
    pub weld_number: Option<String>,
    /// NDE method (for welds).
    #[serde(default)]
    pub nde_method: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Equipment
// ─────────────────────────────────────────────────────────────────────────────

/// Response from `GET /api/v1/equipment`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadmaticEquipment {
    /// CADMATIC object GUID.
    pub object_guid: String,
    /// Equipment tag number.
    pub tag_number: String,
    /// CADMATIC equipment type string (see mapping table).
    pub equipment_type: String,
    /// Service description.
    #[serde(default)]
    pub description: Option<String>,
    /// Design code.
    #[serde(default)]
    pub design_code: Option<String>,
    /// Train identifier (A, B, etc.).
    #[serde(default)]
    pub train_id: Option<String>,
    /// Weight [kg].
    #[serde(default, rename = "weightKg")]
    pub weight_kg: Option<f64>,
    /// Empty weight [kg].
    #[serde(default, rename = "emptyWeightKg")]
    pub empty_weight_kg: Option<f64>,
    /// Operating weight [kg].
    #[serde(default, rename = "operatingWeightKg")]
    pub operating_weight_kg: Option<f64>,
    /// Bounding box minimum point [mm].
    #[serde(default)]
    pub bbox_min: Option<CadmaticPoint3D>,
    /// Bounding box maximum point [mm].
    #[serde(default)]
    pub bbox_max: Option<CadmaticPoint3D>,
    /// Equipment location / area code.
    #[serde(default)]
    pub area_code: Option<String>,
    /// Manufacturer / vendor.
    #[serde(default)]
    pub manufacturer: Option<String>,
    /// Model designation.
    #[serde(default)]
    pub model: Option<String>,
    /// Nozzles (fetched separately via `/equipment/{id}/connections`).
    #[serde(default)]
    pub nozzles: Vec<CadmaticNozzle>,
    /// Custom attributes.
    #[serde(default)]
    pub custom_attributes: std::collections::HashMap<String, serde_json::Value>,
}

/// Equipment nozzle / connection point.
/// Returned from `GET /api/v1/equipment/{id}/connections`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadmaticNozzle {
    /// Nozzle identifier on the equipment (e.g. `"N1"`, `"SUCTION"`).
    pub nozzle_id: String,
    /// Nozzle mark (from equipment drawing).
    #[serde(default)]
    pub nozzle_mark: Option<String>,
    /// Service description.
    #[serde(default)]
    pub service: Option<String>,
    /// Nominal diameter [mm].
    #[serde(default, rename = "nominalDiameterMm")]
    pub nominal_diameter_mm: Option<f64>,
    /// Flange rating (e.g. `"ANSI-150"`, `"PN16"`).
    #[serde(default)]
    pub flange_rating: Option<String>,
    /// Flange facing type.
    #[serde(default)]
    pub facing_type: Option<String>,
    /// Nozzle face centre position [mm].
    pub position: CadmaticPoint3D,
    /// Outward direction vector.
    #[serde(default)]
    pub direction: Option<CadmaticPoint3D>,
    /// ObjectGUID of the connected piping line.
    #[serde(default)]
    pub connected_line_id: Option<String>,
}

/// Response from `GET /api/v1/equipment/{id}/connections`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadmaticConnection {
    pub equipment_guid: String,
    pub nozzles: Vec<CadmaticNozzle>,
}
