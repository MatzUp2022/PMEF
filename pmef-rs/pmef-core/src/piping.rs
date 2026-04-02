//! Piping domain entity types.
//!
//! Covers: PipingNetworkSystem, PipingSegment, and all PipingComponent subtypes.

use crate::catalog::{CatalogReference, DocumentLink, Port};
use crate::geometry::GeometryReference;
use crate::revision::RevisionMetadata;
use crate::types::{Coordinate3D, Iec81346Designation, PmefId, RdlUri};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Design conditions ─────────────────────────────────────────────────────

/// Process design envelope for a piping line or segment.
/// All pressures in Pa (absolute), all temperatures in K.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PipingDesignConditions {
    /// Design pressure [Pa, absolute].
    pub design_pressure: f64,
    /// Maximum design temperature [K].
    pub design_temperature: f64,
    /// Normal operating pressure [Pa, absolute].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operating_pressure: Option<f64>,
    /// Normal operating temperature [K].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operating_temperature: Option<f64>,
    /// Hydrostatic test pressure [Pa, absolute].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_pressure: Option<f64>,
    /// Test medium (e.g. `"WATER"`, `"NITROGEN"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_medium: Option<String>,
    /// True if line operates under vacuum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vacuum_service: Option<bool>,
    /// PED fluid category (1, 2, 3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fluid_category: Option<String>,
    /// PED pressure equipment category (I–IV).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ped_category: Option<String>,
}

impl PipingDesignConditions {
    /// Design temperature converted to Celsius.
    pub fn design_temperature_celsius(&self) -> f64 {
        self.design_temperature - 273.15
    }

    /// Design pressure converted to bar (gauge).
    pub fn design_pressure_barg(&self) -> f64 {
        (self.design_pressure - 101_325.0) / 100_000.0
    }
}

/// Pipe class and material specification.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PipingSpecification {
    pub nominal_diameter: f64,
    pub outside_diameter: f64,
    pub wall_thickness: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    pub pipe_class: String,
    pub material: String,
    pub pressure_rating: String,
    pub corrosion_allowance: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insulation_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heat_tracing_type: Option<String>,
}

impl PipingSpecification {
    /// Calculated bore (inside diameter) [mm].
    pub fn bore_mm(&self) -> f64 {
        self.outside_diameter - 2.0 * self.wall_thickness
    }

    /// Effective wall thickness after corrosion allowance.
    pub fn effective_wall_mm(&self) -> f64 {
        self.wall_thickness - self.corrosion_allowance
    }
}

/// Component spec carried by every piping component.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipingComponentSpec {
    pub component_class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_type1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_type2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub face_to_face: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

// ── PipingNetworkSystem ───────────────────────────────────────────────────

/// A complete piping line from the P&ID.
/// Corresponds to DEXPI `PipingNetworkSystem` and ISO 15926-14 `ProcessSystem`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipingNetworkSystem {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub pmef_version: String,
    /// Full line number tag (e.g. `"8\"-CW-201-A1A2"`).
    pub line_number: String,
    pub is_part_of: PmefId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nominal_diameter: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipe_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fluid_phase: Option<FluidPhase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<PmefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_conditions: Option<PipingDesignConditions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specification: Option<PipingSpecification>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub segments: Vec<PmefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<RdlUri>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid_sheet_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isometric_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

/// Fluid phase enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FluidPhase {
    Liquid,
    Gas,
    TwoPhase,
    Slurry,
    Steam,
    Powder,
}

// ── PipingSegment ─────────────────────────────────────────────────────────

/// A contiguous section of a piping line with uniform specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipingSegment {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub is_part_of: PmefId,
    pub segment_number: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specification: Option<PipingSpecification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_conditions: Option<PipingDesignConditions>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub components: Vec<PmefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
}

// ── Macro for piping component boilerplate ────────────────────────────────

macro_rules! piping_component_base {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            #[serde(rename = "@type")]
            pub entity_type: String,
            #[serde(rename = "@id")]
            pub id: PmefId,
            pub pmef_version: String,
            pub is_part_of: PmefId,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub tag_number: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub item_number: Option<String>,
            pub component_spec: PipingComponentSpec,
            #[serde(skip_serializing_if = "Vec::is_empty", default)]
            pub ports: Vec<Port>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub geometry: Option<GeometryReference>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub catalog_ref: Option<CatalogReference>,
            #[serde(skip_serializing_if = "Vec::is_empty", default)]
            pub documents: Vec<DocumentLink>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub revision: Option<RevisionMetadata>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
            $( pub $field: $ty ),*
        }
    };
}

// ── Concrete component types ──────────────────────────────────────────────

piping_component_base!(Pipe {
    /// Straight pipe length [mm].
    pipe_length: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    spool_mark: Option<String>,
});

/// Elbow radius enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ElbowRadius {
    LongRadius,
    ShortRadius,
    #[serde(rename = "3D")] ThreeD,
    #[serde(rename = "5D")] FiveD,
    Custom,
}

piping_component_base!(Elbow {
    angle: f64,
    radius: ElbowRadius,
    #[serde(skip_serializing_if = "Option::is_none")]
    radius_mm: Option<f64>,
});

piping_component_base!(Tee {
    tee_type: String,
    branch_diameter: f64,
    branch_angle: f64,
});

piping_component_base!(Reducer {
    reducer_type: ReducerType,
    large_diameter: f64,
    small_diameter: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    eccentric_flat: Option<String>,
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReducerType { Concentric, Eccentric }

piping_component_base!(Flange {
    flange_type: String,
    rating: String,
    facing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bore_diameter: Option<f64>,
});

/// Valve actuator specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValveSpec {
    pub actuator_type: String,
    pub fail_position: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leakage_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shutoff_pressure: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_feedback: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handwheel_override: Option<bool>,
}

piping_component_base!(Valve {
    #[serde(skip_serializing_if = "Option::is_none")]
    valve_spec: Option<ValveSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normal_position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instrument_tag: Option<String>,
});

piping_component_base!(Olet {
    olet_type: String,
    branch_diameter: f64,
});

piping_component_base!(Gasket {
    gasket_type: String,
    gasket_material: String,
});

/// Weld inspection specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeldSpec {
    pub weld_number: String,
    pub weld_type: String,
    pub welding_process: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wps_number: Option<String>,
    pub pwht: bool,
    pub nde_method: String,
    pub nde_percentage: u8,
    pub inspection_level: String,
    pub inspection_status: WeldInspectionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WeldInspectionStatus {
    Pending, Accepted, Rejected, Repaired, Waived,
}

piping_component_base!(Weld {
    weld_spec: WeldSpec,
    /// Exactly two component IDs joined by this weld.
    connects: [PmefId; 2],
});

/// Pipe support specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportSpec {
    pub support_type: SupportType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_load_fx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_load_fy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_load_fz: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_moment_mx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_moment_my: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_moment_mz: Option<f64>,
    /// Spring rate [N/mm].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spring_rate: Option<f64>,
    /// Hot (operating) load [N].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hot_load: Option<f64>,
    /// Cold (installed) load [N].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cold_load: Option<f64>,
    /// Travel range [mm].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub travel_range: Option<f64>,
    pub attachment_type: String,
}

impl SupportSpec {
    /// Returns true if this is a spring support (variable or constant hanger).
    pub fn is_spring(&self) -> bool {
        matches!(self.support_type, SupportType::SpringVariable | SupportType::SpringConstant)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupportType {
    Anchor, Guide, Stop, Resting, SpringVariable, SpringConstant,
    Strut, Sway, Rigid, Dummy, Lugs, ClampOn, Trunnion,
}

piping_component_base!(PipeSupport {
    supports_mark: String,
    support_spec: SupportSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    structural_attachment_id: Option<PmefId>,
});

/// Fabrication spool — a group of components shop-fabricated as one unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spool {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub spool_mark: String,
    pub is_part_of: PmefId,
    pub components: Vec<PmefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spool_length: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fabrication_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isometric_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
}
