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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_pressure: Option<f64>,
    /// Maximum design temperature [K].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_temperature: Option<f64>,
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
    /// Minimum operating temperature (for MDMT) [K]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_operating_temp: Option<f64>,
}

impl PipingDesignConditions {
    /// Design temperature converted to Celsius (`None` if not given).
    pub fn design_temperature_celsius(&self) -> Option<f64> {
        self.design_temperature.map(|t| t - 273.15)
    }

    /// Design pressure converted to bar (gauge) (`None` if not given).
    pub fn design_pressure_barg(&self) -> Option<f64> {
        self.design_pressure.map(|p| (p - 101_325.0) / 100_000.0)
    }
}

/// Pipe class and material specification.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PipingSpecification {
    pub nominal_diameter: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outside_diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    pub pipe_class: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_rating: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrosion_allowance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insulation_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heat_tracing_type: Option<String>,
    /// NPS (nominal pipe size) in inches — informational
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nominal_diameter_inch: Option<f64>,
    /// Material group per EN 13480 / ASME B31.3
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_group: Option<String>,
    /// Full line number tag, e.g. '8"-CW-101-A1A2'
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_number: Option<String>,
    /// Service medium code, e.g. 'CW' (cooling water), 'SS' (steam)'
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium_code: Option<String>,
    /// Schema property `paintingSystem`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub painting_system: Option<String>,
    /// [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insulation_thickness: Option<f64>,
}

impl PipingSpecification {
    /// Calculated bore (inside diameter) [mm]; needs outside diameter and wall thickness.
    pub fn bore_mm(&self) -> Option<f64> {
        Some(self.outside_diameter? - 2.0 * self.wall_thickness?)
    }

    /// Effective wall thickness after corrosion allowance (none given = 0 mm).
    pub fn effective_wall_mm(&self) -> Option<f64> {
        Some(self.wall_thickness? - self.corrosion_allowance.unwrap_or(0.0))
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
    /// Flange facing type
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing_type: Option<String>,
    /// Centre-to-face dimension [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centre_to_face: Option<f64>,
    /// Spool/isometric item number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_number: Option<String>,
    /// Fabrication spool reference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spool_id: Option<String>,
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
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specification: Option<PipingSpecification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_conditions: Option<PipingDesignConditions>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub components: Vec<PmefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

// ── Macro for piping component boilerplate ────────────────────────────────

macro_rules! piping_component_base {
    ($name:ident { $($(#[$field_meta:meta])* $field:ident : $ty:ty),* $(,)? }) => {
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
            /// RFC-0001 provenance / review / asBuilt blocks.
            #[serde(flatten, default)]
            pub recognition: crate::provenance::RecognitionMetadata,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
            /// Material traceability heat/cast number.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub heat_number: Option<String>,
            /// IEC 81346 reference designation.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub iec81346: Option<crate::types::Iec81346Designation>,
            /// Reference data library class URI.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub rdl_type: Option<crate::types::RdlUri>,
            $( $(#[$field_meta])* pub $field: $ty ),*
        }
    };
}

// ── Concrete component types ──────────────────────────────────────────────

piping_component_base!(Pipe {
    /// Straight pipe length [mm].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pipe_length: Option<f64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    radius: Option<ElbowRadius>,
    #[serde(skip_serializing_if = "Option::is_none")]
    radius_mm: Option<f64>,
});

piping_component_base!(Tee {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tee_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch_diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch_angle: Option<f64>,
});

piping_component_base!(Reducer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reducer_type: Option<ReducerType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    large_diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    small_diameter: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eccentric_flat: Option<String>,
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReducerType { Concentric, Eccentric }

piping_component_base!(Flange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    flange_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rating: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    facing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bore_diameter: Option<f64>,
});

/// Valve actuator specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValveSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actuator_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_position: Option<String>,
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
    /// Schema property `lockable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lockable: Option<bool>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    olet_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch_diameter: Option<f64>,
    /// Schema property `branchSchedule`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch_schedule: Option<String>,
});

piping_component_base!(Gasket {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gasket_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gasket_material: Option<String>,
});

/// Weld inspection specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeldSpec {
    pub weld_number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weld_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub welding_process: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wps_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pwht: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nde_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nde_percentage: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_status: Option<WeldInspectionStatus>,
    /// Schema property `weldorId`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weldor_id: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_type: Option<SupportType>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_type: Option<String>,
    /// Reference to the SteelObject or CivilObject this support is attached to
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_ref: Option<String>,
}

impl SupportSpec {
    /// Returns true if this is a spring support (variable or constant hanger).
    pub fn is_spring(&self) -> bool {
        matches!(self.support_type, Some(SupportType::SpringVariable | SupportType::SpringConstant))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupportType {
    Anchor, Guide, Stop, Resting, SpringVariable, SpringConstant,
    Strut, Sway, Rigid, Dummy, Lugs, ClampOn, Trunnion,
}

piping_component_base!(PipeSupport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    supports_mark: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    support_spec: Option<SupportSpec>,
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
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}
