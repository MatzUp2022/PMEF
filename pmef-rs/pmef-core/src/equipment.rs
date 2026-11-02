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
    /// Schema property `purchaseOrderRef`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purchase_order_ref: Option<String>,
    /// Schema property `installationDate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_date: Option<String>,
    /// Nuclear Safety System: true if safety-classified
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nsss: Option<bool>,
}

/// Physical connection point on a piece of equipment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Nozzle {
    pub nozzle_id: String,
    pub nozzle_mark: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nominal_diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flange_rating: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing_type: Option<String>,
    pub coordinate: Coordinate3D,
    pub direction: UnitVector3D,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_line_id: Option<PmefId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_port_id: Option<String>,
    /// Elevation of nozzle face above datum [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation: Option<f64>,
    /// Schema property `normallyOpen`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normally_open: Option<bool>,
    /// Schema property `blindedInService`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blinded_in_service: Option<bool>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pump_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_standard: Option<String>,
    /// Design flow [m³/h].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_flow: Option<f64>,
    /// Design head [m].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_head: Option<f64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motor_power: Option<f64>,
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
    /// Schema property `arrangementType`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrangement_type: Option<String>,
    /// Minimum continuous stable flow [m³/h]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_flow: Option<f64>,
    /// Maximum flow [m³/h]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_flow: Option<f64>,
    /// Shut-off head [m]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shutoff_head: Option<f64>,
    /// [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impeller_diameter: Option<f64>,
    /// Design fluid density [kg/m³]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluid_density: Option<f64>,
    /// Design kinematic viscosity [cSt]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluid_viscosity: Option<f64>,
    /// Design fluid temperature [K]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluid_temperature: Option<f64>,
    /// URI to H-Q performance curve document
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hq_curve_ref: Option<String>,
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
    #[serde(default)]
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
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// PMEF ID of the associated Motor equipment object
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motor_id: Option<String>,
}

// ── Vessel ────────────────────────────────────────────────────────────────

/// Pressure vessel design data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VesselDesign {
    /// Internal design pressure [Pa, absolute].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_pressure_internal: Option<f64>,
    /// External design pressure [Pa].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_pressure_external: Option<f64>,
    /// Maximum design temperature [K].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_temperature_max: Option<f64>,
    /// Minimum design temperature / MDMT [K].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_temperature_min: Option<f64>,
    /// Volume [m³].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_material: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_inside_diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tangent_to_tangent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrosion_allowance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_thickness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insulation_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xray_requirement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stress_relief: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fireproofing_required: Option<bool>,
    /// Schema property `headMaterial`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_material: Option<String>,
    /// Schema property `liningMaterial`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lining_material: Option<String>,
    /// Schema property `numberOfShells`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_shells: Option<i64>,
    /// [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insulation_thickness: Option<f64>,
    /// [Pa] (if integral PSV)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_valve_set_pressure: Option<f64>,
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
    #[serde(default)]
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<RdlUri>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// URI of the DEXPI/P&ID FunctionalObject (tag) this equipment realises
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<String>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
}

// ── HeatExchanger ─────────────────────────────────────────────────────────

/// Shell-and-tube and plate heat exchanger spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatExchangerSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hx_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duty_type: Option<String>,
    /// Heat duty [W].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heat_duty: Option<f64>,
    /// Overall heat transfer coefficient [W/m²K].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_heat_transfer_coeff: Option<f64>,
    /// Heat transfer area [m²].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heat_transfer_area: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_side_medium: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_side_medium: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_side_inlet_temp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_side_outlet_temp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_side_inlet_temp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_side_outlet_temp: Option<f64>,
    /// Shell-side flow [m³/h].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_side_flow: Option<f64>,
    /// Tube-side flow [m³/h].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_side_flow: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_side_design_pressure: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_side_design_pressure: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_shell_passes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_tube_passes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_outside_diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_wall_thickness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_length: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_tubes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube_material: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_material: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fouling_factor_shell: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fouling_factor_tube: Option<f64>,
}

impl HeatExchangerSpec {
    /// Log Mean Temperature Difference (LMTD) [K] — counter-current.
    /// `None` unless all four terminal temperatures are given.
    pub fn lmtd_counter_current(&self) -> Option<f64> {
        let dt1 = (self.shell_side_inlet_temp? - self.tube_side_outlet_temp?).abs();
        let dt2 = (self.shell_side_outlet_temp? - self.tube_side_inlet_temp?).abs();
        Some(if (dt1 - dt2).abs() < 0.001 {
            dt1
        } else {
            (dt1 - dt2) / (dt1 / dt2).ln()
        })
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
    #[serde(default)]
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// URI of the DEXPI/P&ID FunctionalObject (tag) this equipment realises
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<String>,
    /// URI pointing to a class in ISO 15926-4 (PCA-RDL), CFIHOS-RDL or a project-level catalog. Must be resolvable via SPARQL endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
}

// ── Compressor ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressorSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressor_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_standard: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_inlet_flow: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_inlet_pressure: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_outlet_pressure: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaft_power: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seal_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_stages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rated_speed: Option<u32>,
    /// Polytropic efficiency [%] (`polytropicEfficiency`). The deprecated spelling
    /// `polytropic_efficiency` (RFC-0004) is accepted on input; output uses camelCase.
    #[serde(alias = "polytropic_efficiency", skip_serializing_if = "Option::is_none")]
    pub polytropic_efficiency: Option<f64>,
    /// Schema property `gasMedium`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gas_medium: Option<String>,
    /// [K]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inlet_temperature: Option<f64>,
    /// Schema property `molecularWeight`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub molecular_weight: Option<f64>,
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
    #[serde(default)]
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// URI of the DEXPI/P&ID FunctionalObject (tag) this equipment realises
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<String>,
    /// URI pointing to a class in ISO 15926-4 (PCA-RDL), CFIHOS-RDL or a project-level catalog. Must be resolvable via SPARQL endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
    /// Schema property `motorId`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motor_id: Option<String>,
    /// IDs of lube oil, seal gas, cooling systems
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aux_system_ids: Option<Vec<String>>,
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
    #[serde(default)]
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic_equipment_subtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// URI of the DEXPI/P&ID FunctionalObject (tag) this equipment realises
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<String>,
    /// Reference designation according to IEC 81346. Prefix defines the aspect: '=' function, '-' product, '+' location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<crate::types::Iec81346Designation>,
    /// URI pointing to a class in ISO 15926-4 (PCA-RDL), CFIHOS-RDL or a project-level catalog. Must be resolvable via SPARQL endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactor_type: Option<String>,
    #[serde(default)]
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_power: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// URI of the DEXPI/P&ID FunctionalObject (tag) this equipment realises
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<String>,
    /// Reference designation according to IEC 81346. Prefix defines the aspect: '=' function, '-' product, '+' location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<crate::types::Iec81346Designation>,
    /// URI pointing to a class in ISO 15926-4 (PCA-RDL), CFIHOS-RDL or a project-level catalog. Must be resolvable via SPARQL endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
    /// Schema property `catalystType`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalyst_type: Option<String>,
    /// e.g. 'Exothermic catalytic', 'Electrochemical smelting'
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction_type: Option<String>,
    /// Schema property `heatRemovalType`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heat_removal_type: Option<String>,
    /// [V] for EAF / electrochemical reactors
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_supply_voltage: Option<f64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tank_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_standard: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_capacity: Option<f64>,
    #[serde(default)]
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// URI of the DEXPI/P&ID FunctionalObject (tag) this equipment realises
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<String>,
    /// Reference designation according to IEC 81346. Prefix defines the aspect: '=' function, '-' product, '+' location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<crate::types::Iec81346Designation>,
    /// URI pointing to a class in ISO 15926-4 (PCA-RDL), CFIHOS-RDL or a project-level catalog. Must be resolvable via SPARQL endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
    /// Tank shell inside diameter [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter: Option<f64>,
    /// Shell height [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_height: Option<f64>,
    /// [Pa]; 0 for atmospheric
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_pressure: Option<f64>,
    /// [K]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_temperature: Option<f64>,
    /// Stored fluid/product
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<String>,
    /// Schema property `heatingCoil`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heating_coil: Option<bool>,
    /// Schema property `coolingCoil`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooling_coil: Option<bool>,
    /// Schema property `mixerInstalled`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mixer_installed: Option<bool>,
    /// Schema property `dikedArea`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diked_area: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    #[serde(rename = "@type")] pub entity_type: String,
    #[serde(rename = "@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filtration_rating: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_flow: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub differential_pressure: Option<f64>,
    #[serde(default)]
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// URI of the DEXPI/P&ID FunctionalObject (tag) this equipment realises
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<String>,
    /// Reference designation according to IEC 81346. Prefix defines the aspect: '=' function, '-' product, '+' location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<crate::types::Iec81346Designation>,
    /// URI pointing to a class in ISO 15926-4 (PCA-RDL), CFIHOS-RDL or a project-level catalog. Must be resolvable via SPARQL endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Link from a PMEF object to one or more geometry representations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<crate::geometry::GeometryReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
    /// Schema property `cleaningType`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleaning_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turbine {
    #[serde(rename = "@type")] pub entity_type: String,
    #[serde(rename = "@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub equipment_basic: EquipmentBasic,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turbine_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inlet_pressure: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outlet_pressure: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inlet_temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaft_power: Option<f64>,
    #[serde(default)]
    pub nozzles: Vec<Nozzle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    /// URI of the DEXPI/P&ID FunctionalObject (tag) this equipment realises
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_derived_from: Option<String>,
    /// Reference designation according to IEC 81346. Prefix defines the aspect: '=' function, '-' product, '+' location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<crate::types::Iec81346Designation>,
    /// URI pointing to a class in ISO 15926-4 (PCA-RDL), CFIHOS-RDL or a project-level catalog. Must be resolvable via SPARQL endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    /// Reference to a piping spec catalog entry, standard profile or equipment datasheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_ref: Option<crate::catalog::CatalogReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
    /// [K]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outlet_temperature: Option<f64>,
    /// [rpm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rated_speed: Option<f64>,
    /// [%]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isentropic_efficiency: Option<f64>,
}
