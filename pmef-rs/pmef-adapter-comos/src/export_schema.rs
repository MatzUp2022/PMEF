//! COMOS JSON export data types.
//!
//! COMOS organises all engineering data in a hierarchical **object tree**
//! rooted at the project level. Each node is a `ComosObject` with:
//!
//! - A **class** (from the COMOS class library, e.g. `@I10` = instrument,
//!   `@E03` = pump, `@L10` = piping line)
//! - A **CUID** (globally unique COMOS ID, persists across renames)
//! - A set of **attributes** (typed key-value pairs from the class definition)
//! - Child objects
//!
//! The C# exporter (`ComosExporter.cs`) flattens this tree into a typed JSON
//! export consumed by this Rust crate.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Export root
// ─────────────────────────────────────────────────────────────────────────────

/// Root of the COMOS JSON export.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosExport {
    pub schema_version: String,
    pub comos_version: String,
    pub exported_at: String,
    pub project_name: String,
    pub project_cuid: String,
    #[serde(default)]
    pub plant_units: Vec<ComosUnit>,
    #[serde(default)]
    pub equipment: Vec<ComosEquipment>,
    #[serde(default)]
    pub piping_lines: Vec<ComotLine>,
    #[serde(default)]
    pub instruments: Vec<ComosInstrument>,
    #[serde(default)]
    pub instrument_loops: Vec<ComosLoop>,
    #[serde(default)]
    pub cables: Vec<ComosCable>,
    #[serde(default)]
    pub plc_objects: Vec<ComosPlc>,
    #[serde(default)]
    pub documents: Vec<ComosDocument>,
    pub summary: ComosExportSummary,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosExportSummary {
    pub equipment_count: u32,
    pub instrument_count: u32,
    pub loop_count: u32,
    pub line_count: u32,
    pub cable_count: u32,
    pub plc_count: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Plant hierarchy
// ─────────────────────────────────────────────────────────────────────────────

/// A COMOS plant unit (area / functional group).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosUnit {
    pub cuid: String,
    pub name: String,
    pub description: Option<String>,
    /// COMOS class: `@A01` (plant), `@A02` (unit), `@A03` (area)
    pub comos_class: String,
    pub parent_cuid: Option<String>,
    pub iec81346_functional: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Equipment
// ─────────────────────────────────────────────────────────────────────────────

/// COMOS equipment object (from `@E` class branch).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosEquipment {
    /// COMOS unique ID (persists across renames).
    pub cuid: String,
    /// Tag number (from COMOS `TAG` attribute).
    pub tag_number: String,
    /// COMOS class (e.g. `@E03` = centrifugal pump, `@E07` = pressure vessel).
    pub comos_class: String,
    /// User-readable class description.
    pub class_description: String,
    /// Service description.
    pub description: Option<String>,
    /// Parent unit CUID.
    pub unit_cuid: String,
    /// P&ID document reference.
    pub pid_reference: Option<String>,
    /// Revision / status.
    pub status: Option<String>,
    /// IEC 81346 functional designation (e.g. `=U100.M01.A`).
    pub iec81346_functional: Option<String>,
    /// IEC 81346 product designation (e.g. `-P201A`).
    pub iec81346_product: Option<String>,
    /// Process nozzles (from COMOS nozzle sub-objects).
    #[serde(default)]
    pub nozzles: Vec<ComosNozzle>,
    /// Design attributes (typed, from COMOS class attribute definitions).
    pub design_attrs: ComosEquipmentDesign,
    /// Raw COMOS attributes (all other UDAs not in design_attrs).
    #[serde(default)]
    pub raw_attrs: HashMap<String, serde_json::Value>,
    /// Linked documents.
    #[serde(default)]
    pub documents: Vec<ComosDocRef>,
}

/// Design data from COMOS equipment class attributes.
/// Attribute names follow the COMOS standard class library (Siemens).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComosEquipmentDesign {
    /// Design pressure [bar g] — from COMOS `CTA_DesignPressure`.
    pub design_pressure_barg: Option<f64>,
    /// Design temperature max [°C] — from `CTA_DesignTemperature`.
    pub design_temperature_degc: Option<f64>,
    /// Design temperature min / MDMT [°C].
    pub design_temperature_min_degc: Option<f64>,
    /// Operating pressure [bar g] — from `CTA_OperatingPressure`.
    pub operating_pressure_barg: Option<f64>,
    /// Operating temperature [°C] — from `CTA_OperatingTemperature`.
    pub operating_temperature_degc: Option<f64>,
    /// Volume [m³] — from `CTA_Volume`.
    pub volume_m3: Option<f64>,
    /// Shell material — from `CTA_Material`.
    pub material: Option<String>,
    /// Design code — from `CTA_DesignCode`.
    pub design_code: Option<String>,
    /// Weight empty [kg] — from `CTA_Weight`.
    pub weight_empty_kg: Option<f64>,
    /// Weight operating [kg].
    pub weight_operating_kg: Option<f64>,
    /// Manufacturer — from `CTA_Manufacturer`.
    pub manufacturer: Option<String>,
    /// Model — from `CTA_Type`.
    pub model: Option<String>,
    /// Motor power [kW] — for pumps/compressors, from `CTA_MotorPower`.
    pub motor_power_kw: Option<f64>,
    /// Design flow [m³/h] — from `CTA_FlowDesign`.
    pub design_flow_m3h: Option<f64>,
    /// Design head / differential pressure [m / bar].
    pub design_head_m: Option<f64>,
    /// Heat duty [kW] — for heat exchangers, from `CTA_Duty`.
    pub heat_duty_kw: Option<f64>,
    /// Heat transfer area [m²] — from `CTA_HeatTransferArea`.
    pub heat_transfer_area_m2: Option<f64>,
    /// TEMA type designation — for heat exchangers.
    pub tema_type: Option<String>,
    /// Inside diameter [mm] — from `CTA_InsideDiameter`.
    pub inside_diameter_mm: Option<f64>,
    /// Tangent-to-tangent length [mm] — from `CTA_LengthTangentTangent`.
    pub tangent_length_mm: Option<f64>,
    /// Shell-side design pressure [bar g] (HX only).
    pub shell_side_pressure_barg: Option<f64>,
    /// Tube-side design pressure [bar g] (HX only).
    pub tube_side_pressure_barg: Option<f64>,
}

/// Equipment nozzle from COMOS.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosNozzle {
    pub cuid: String,
    pub nozzle_mark: String,
    pub service: Option<String>,
    pub nominal_diameter_mm: Option<f64>,
    pub flange_rating: Option<String>,
    pub facing_type: Option<String>,
    pub connected_line_cuid: Option<String>,
    pub iec81346: Option<String>,
}

/// Reference to a linked document.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosDocRef {
    pub document_cuid: String,
    pub document_type: String,
    pub document_number: Option<String>,
    pub revision: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Piping lines
// ─────────────────────────────────────────────────────────────────────────────

/// A COMOS piping line (`@L10` class).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComotLine {
    pub cuid: String,
    pub line_number: String,
    pub unit_cuid: String,
    pub description: Option<String>,
    pub nominal_diameter_mm: Option<f64>,
    pub pipe_class: Option<String>,
    pub medium_code: Option<String>,
    pub medium_description: Option<String>,
    /// Design pressure [bar g].
    pub design_pressure_barg: Option<f64>,
    /// Design temperature [°C].
    pub design_temperature_degc: Option<f64>,
    /// Operating pressure [bar g].
    pub operating_pressure_barg: Option<f64>,
    /// Operating temperature [°C].
    pub operating_temperature_degc: Option<f64>,
    /// Test pressure [bar g].
    pub test_pressure_barg: Option<f64>,
    pub material: Option<String>,
    pub insulation_type: Option<String>,
    pub heat_tracing: Option<String>,
    pub pid_reference: Option<String>,
    pub iec81346_functional: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub raw_attrs: HashMap<String, serde_json::Value>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Instruments
// ─────────────────────────────────────────────────────────────────────────────

/// A COMOS instrument object (`@I` class branch).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosInstrument {
    pub cuid: String,
    pub tag_number: String,
    /// COMOS class (e.g. `@I10` = transmitter, `@I20` = controller,
    /// `@I30` = final element / valve, `@I40` = safety element).
    pub comos_class: String,
    pub class_description: String,
    pub unit_cuid: String,
    pub loop_cuid: Option<String>,
    pub pid_reference: Option<String>,
    pub iec81346_functional: Option<String>,
    pub iec81346_product: Option<String>,
    pub status: Option<String>,
    /// Instrument design data.
    pub design_attrs: ComosInstrumentDesign,
    #[serde(default)]
    pub raw_attrs: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub documents: Vec<ComosDocRef>,
}

/// COMOS instrument design attributes.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComosInstrumentDesign {
    /// Process variable (e.g. `"FLOW"`, `"PRESSURE"`, `"TEMPERATURE"`).
    pub process_variable: Option<String>,
    /// Measured range minimum — from `CTA_RangeMin`.
    pub range_min: Option<f64>,
    /// Measured range maximum — from `CTA_RangeMax`.
    pub range_max: Option<f64>,
    /// Engineering unit — from `CTA_Unit`.
    pub range_unit: Option<String>,
    /// Signal type — from `CTA_SignalType` (e.g. `"4-20mA"`, `"HART"`, `"Profibus"`).
    pub signal_type: Option<String>,
    /// Fail safe position — from `CTA_FailSafe`.
    pub fail_safe: Option<String>,
    /// Safety integrity level (0–4) — from `CTA_SIL`.
    pub sil_level: Option<u8>,
    /// Proof test interval [months] — from `CTA_ProofTestInterval`.
    pub proof_test_interval_months: Option<u32>,
    /// PFDavg — from `CTA_PFD`.
    pub pfd: Option<f64>,
    /// PFH — from `CTA_PFH`.
    pub pfh: Option<f64>,
    /// Architecture type (e.g. `"1oo1"`, `"1oo2"`, `"2oo3"`).
    pub architecture: Option<String>,
    /// Safe state — from `CTA_SafeState`.
    pub safe_state: Option<String>,
    /// Intrinsically safe (Ex-i) — from `CTA_ExProtection`.
    pub intrinsic_safe: Option<bool>,
    /// Hazardous area classification — from `CTA_HazArea`.
    pub hazardous_area: Option<String>,
    /// IP protection rating — from `CTA_IPRating`.
    pub ip_rating: Option<String>,
    /// Manufacturer — from `CTA_Manufacturer`.
    pub manufacturer: Option<String>,
    /// Model — from `CTA_Model`.
    pub model: Option<String>,
    /// TIA Portal PLC address — from COMOS/TIA integration attribute.
    pub tia_plc_address: Option<String>,
    /// EPLAN function text (from COMOS–EPLAN exchange attribute).
    pub eplan_function_text: Option<String>,
    /// Kv value [m³/h] — for control valves.
    pub kv_value: Option<f64>,
    /// Shutoff class — for control valves.
    pub shutoff_class: Option<String>,
    /// Actuator type — for valves.
    pub actuator_type: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Instrument loops
// ─────────────────────────────────────────────────────────────────────────────

/// A COMOS instrument loop (`@I05` class).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosLoop {
    pub cuid: String,
    pub loop_number: String,
    pub loop_type: String,
    pub unit_cuid: String,
    pub sil_level: Option<u8>,
    pub status: Option<String>,
    pub member_cuids: Vec<String>,
    pub controller_cuid: Option<String>,
    pub final_element_cuid: Option<String>,
    pub pid_reference: Option<String>,
    pub iec81346_functional: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Cables
// ─────────────────────────────────────────────────────────────────────────────

/// A COMOS cable object (`@K` class branch).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosCable {
    pub cuid: String,
    pub cable_number: String,
    pub comos_class: String,
    pub unit_cuid: String,
    pub cable_type: Option<String>,
    pub cross_section_mm2: Option<f64>,
    pub number_of_cores: Option<u32>,
    pub voltage_rating_v: Option<u32>,
    pub from_cuid: Option<String>,
    pub to_cuid: Option<String>,
    pub route_length_m: Option<f64>,
    pub cable_tray_cuid: Option<String>,
    pub iec81346_product: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// PLC objects
// ─────────────────────────────────────────────────────────────────────────────

/// A COMOS PLC / control system object (`@S` class branch).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosPlc {
    pub cuid: String,
    pub tag_number: String,
    /// COMOS class (e.g. `@S10` = CPU, `@S20` = I/O module, `@S30` = network).
    pub comos_class: String,
    pub class_description: String,
    pub unit_cuid: String,
    pub vendor: Option<String>,
    pub family: Option<String>,
    pub article_number: Option<String>,
    pub rack: Option<u32>,
    pub slot: Option<u32>,
    pub ip_address: Option<String>,
    pub safety_cpu: Option<bool>,
    pub tia_portal_ref: Option<String>,
    pub aml_ref: Option<String>,
    pub iec81346_product: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Documents
// ─────────────────────────────────────────────────────────────────────────────

/// A COMOS document (P&ID, datasheets, etc.).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComosDocument {
    pub cuid: String,
    pub document_number: String,
    pub document_type: String,
    pub title: Option<String>,
    pub revision: Option<String>,
    pub status: Option<String>,
    pub file_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialise_export_root() {
        let json = r#"{
            "schemaVersion": "1.0",
            "comosVersion": "10.4",
            "exportedAt": "2026-03-31T00:00:00Z",
            "projectName": "EAF-2026",
            "projectCuid": "CUID-PROJ-001",
            "equipment": [],
            "pipingLines": [],
            "instruments": [],
            "instrumentLoops": [],
            "cables": [],
            "plcObjects": [],
            "documents": [],
            "plantUnits": [],
            "summary": {
                "equipmentCount": 0, "instrumentCount": 0,
                "loopCount": 0, "lineCount": 0,
                "cableCount": 0, "plcCount": 0
            }
        }"#;
        let export: ComosExport = serde_json::from_str(json).unwrap();
        assert_eq!(export.schema_version, "1.0");
        assert_eq!(export.project_name, "EAF-2026");
        assert_eq!(export.summary.equipment_count, 0);
    }

    #[test]
    fn test_equipment_design_defaults() {
        let d = ComosEquipmentDesign::default();
        assert!(d.design_pressure_barg.is_none());
        assert!(d.heat_duty_kw.is_none());
    }

    #[test]
    fn test_instrument_design_defaults() {
        let d = ComosInstrumentDesign::default();
        assert!(d.sil_level.is_none());
        assert!(d.pfd.is_none());
    }

    #[test]
    fn test_piping_line_deserialise() {
        let json = r#"{
            "cuid": "CUID-LINE-001",
            "lineNumber": "8\"-CW-201-A1A2",
            "unitCuid": "CUID-UNIT-001",
            "nominalDiameterMm": 200.0,
            "designPressureBarg": 15.0,
            "designTemperatureDegc": 60.0
        }"#;
        let line: ComotLine = serde_json::from_str(json).unwrap();
        assert_eq!(line.line_number, "8\"-CW-201-A1A2");
        assert_eq!(line.design_pressure_barg, Some(15.0));
    }

    #[test]
    fn test_instrument_deserialise() {
        let json = r#"{
            "cuid": "CUID-FIC-101",
            "tagNumber": "FIC-10101",
            "comosClass": "@I10",
            "classDescription": "Transmitter",
            "unitCuid": "CUID-UNIT-001",
            "designAttrs": {
                "signalType": "HART",
                "silLevel": 1,
                "processVariable": "FLOW"
            }
        }"#;
        let inst: ComosInstrument = serde_json::from_str(json).unwrap();
        assert_eq!(inst.tag_number, "FIC-10101");
        assert_eq!(inst.design_attrs.signal_type, Some("HART".to_owned()));
        assert_eq!(inst.design_attrs.sil_level, Some(1));
    }

    #[test]
    fn test_loop_deserialise() {
        let json = r#"{
            "cuid": "CUID-LOOP-101",
            "loopNumber": "FIC-10101",
            "loopType": "FLOW_CONTROL",
            "unitCuid": "CUID-UNIT-001",
            "silLevel": 1,
            "memberCuids": ["CUID-FIC-101", "CUID-XV-101"]
        }"#;
        let lp: ComosLoop = serde_json::from_str(json).unwrap();
        assert_eq!(lp.loop_number, "FIC-10101");
        assert_eq!(lp.member_cuids.len(), 2);
        assert_eq!(lp.sil_level, Some(1));
    }
}
