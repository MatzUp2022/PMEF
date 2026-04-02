//! Equipment domain entity types.

use crate::catalog::DocumentLink;
use crate::geometry::GeometryReference;
use crate::revision::RevisionMetadata;
use crate::types::{Coordinate3D, Iec81346Designation, PmefId, RdlUri, UnitVector3D};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Basic equipment attributes common to all equipment subtypes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentBasic {
    pub tag_number: String,
    pub equipment_class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub design_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_area: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
}

/// Physical connection point on a piece of equipment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Nozzle {
    pub nozzle_id: String,
    pub nozzle_mark: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    pub nominal_diameter: f64,
    pub flange_rating: String,
    pub facing_type: String,
    pub coordinate: Coordinate3D,
    pub direction: UnitVector3D,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_line_id: Option<PmefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_port_id: Option<String>,
}

impl Nozzle {
    /// Returns true if this nozzle is connected to a piping line.
    pub fn is_connected(&self) -> bool {
        self.connected_line_id.is_some()
    }
}

// ── Pump ──────────────────────────────────────────────────────────────────

/// Centrifugal or positive displacement pump spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PumpSpec {
    pub pump_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_standard: Option<String>,
    /// Design flow [m³/h].
    pub design_flow: f64,
    /// Design head [m].
    pub design_head: f64,
    /// Efficiency [%].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub efficiency: Option<f64>,
    /// NPSH required [m].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npsh_required: Option<f64>,
    /// NPSH available [m].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npsh_available: Option<f64>,
    /// Rated speed [rpm].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rated_speed: Option<u32>,
    /// Installed motor power [kW].
    pub motor_power: f64,
    /// Motor voltage [V].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motor_voltage: Option<u32>,
    /// Motor frequency [Hz] — 50 or 60.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motor_frequency: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drivetype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seal_type: Option<String>,
    /// True if this is the spare pump in an A/B pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spare_pump: Option<bool>,
}

impl PumpSpec {
    /// NPSH margin = available - required [m]. Returns None if either is absent.
    pub fn npsh_margin(&self) -> Option<f64> {
        Some(self.npsh_available? - self.npsh_required?)
    }

    /// Returns true if NPSH margin is positive.
    pub fn npsh_ok(&self) -> Option<bool> {
        self.npsh_margin().map(|m| m > 0.0)
    }
}

/// Centrifugal pump.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pump {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub pump_spec: PumpSpec,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<PmefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<RdlUri>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub documents: Vec<DocumentLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

// ── Vessel ────────────────────────────────────────────────────────────────

/// Pressure vessel design data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VesselDesign {
    /// Internal design pressure [Pa, absolute].
    pub design_pressure_internal: f64,
    /// External design pressure [Pa].
    pub design_pressure_external: f64,
    /// Maximum design temperature [K].
    pub design_temperature_max: f64,
    /// Minimum design temperature / MDMT [K].
    pub design_temperature_min: f64,
    /// Volume [m³].
    pub volume: f64,
    pub shell_material: String,
    pub shell_inside_diameter: f64,
    pub tangent_to_tangent: f64,
    pub head_type: String,
    pub orientation: String,
    pub corrosion_allowance: f64,
    pub shell_thickness: f64,
    pub insulation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xray_requirement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stress_relief: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fireproofing_required: Option<bool>,
}

/// Pressure vessel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vessel {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub vessel_design: VesselDesign,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vessel_subtype: Option<String>,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<RdlUri>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

// ── HeatExchanger ─────────────────────────────────────────────────────────

/// Shell-and-tube and plate heat exchanger spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatExchangerSpec {
    pub hx_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tema: Option<String>,
    pub duty_type: String,
    /// Heat duty [W].
    pub heat_duty: f64,
    /// Overall heat transfer coefficient [W/m²K].
    pub overall_heat_transfer_coeff: f64,
    /// Heat transfer area [m²].
    pub heat_transfer_area: f64,
    pub shell_side_medium: String,
    pub tube_side_medium: String,
    pub shell_side_inlet_temp: f64,
    pub shell_side_outlet_temp: f64,
    pub tube_side_inlet_temp: f64,
    pub tube_side_outlet_temp: f64,
    /// Shell-side flow [m³/h].
    pub shell_side_flow: f64,
    /// Tube-side flow [m³/h].
    pub tube_side_flow: f64,
    pub shell_side_design_pressure: f64,
    pub tube_side_design_pressure: f64,
    pub number_of_shell_passes: u32,
    pub number_of_tube_passes: u32,
    pub tube_outside_diameter: f64,
    pub tube_wall_thickness: f64,
    pub tube_length: f64,
    pub number_of_tubes: u32,
    pub tube_material: String,
    pub shell_material: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fouling_factor_shell: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fouling_factor_tube: Option<f64>,
}

impl HeatExchangerSpec {
    /// Log Mean Temperature Difference (LMTD) [K] — counter-current.
    pub fn lmtd_counter_current(&self) -> f64 {
        let dt1 = (self.shell_side_inlet_temp - self.tube_side_outlet_temp).abs();
        let dt2 = (self.shell_side_outlet_temp - self.tube_side_inlet_temp).abs();
        if (dt1 - dt2).abs() < 0.001 {
            dt1
        } else {
            (dt1 - dt2) / (dt1 / dt2).ln()
        }
    }
}

/// Shell-and-tube heat exchanger.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatExchanger {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub hx_spec: HeatExchangerSpec,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

// ── Compressor ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressorSpec {
    pub compressor_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_standard: Option<String>,
    pub design_inlet_flow: f64,
    pub design_inlet_pressure: f64,
    pub design_outlet_pressure: f64,
    pub pressure_ratio: f64,
    pub shaft_power: f64,
    pub driver_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seal_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_stages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rated_speed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polytropic_efficiency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Compressor {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub compressor_spec: CompressorSpec,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

// ── Generic Equipment ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericEquipment {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic_equipment_subtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

// ── Reactor ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reactor {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub vessel_design: VesselDesign,
    pub reactor_type: String,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_power: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

// ── Tank, Filter, Turbine (compact) ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tank {
    #[serde(rename = "@type")] pub entity_type: String,
    #[serde(rename = "@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub tank_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_standard: Option<String>,
    pub capacity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_capacity: Option<f64>,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    #[serde(rename = "@type")] pub entity_type: String,
    #[serde(rename = "@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub filter_type: String,
    pub filtration_rating: f64,
    pub design_flow: f64,
    pub differential_pressure: f64,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turbine {
    #[serde(rename = "@type")] pub entity_type: String,
    #[serde(rename = "@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    pub turbine_type: String,
    pub inlet_pressure: f64,
    pub outlet_pressure: f64,
    pub inlet_temperature: f64,
    pub shaft_power: f64,
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}
