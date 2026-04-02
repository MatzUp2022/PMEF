//! AutoCAD Plant 3D equipment data types and mapping.
//!
//! Plant 3D stores equipment in the Plant 3D Project Data Store (PDS),
//! accessible via the Plant SDK (`Autodesk.ProcessPower.PlantProject` API).
//! This module defines the JSON export schema produced by `PlantExporter.cs`
//! and maps it to PMEF equipment types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Plant 3D equipment export types
// ─────────────────────────────────────────────────────────────────────────────

/// A Plant 3D equipment nozzle (connection point).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P3dNozzle {
    pub nozzle_number: String,
    pub service: Option<String>,
    pub nominal_diameter_in: f64,
    pub flange_rating: Option<String>,
    pub facing_type: Option<String>,
    /// World position [inches — converted to mm on export].
    pub position_mm: [f64; 3],
    /// Direction vector.
    pub direction: [f64; 3],
    /// Connected PCF line tag.
    pub connected_line_tag: Option<String>,
}

impl P3dNozzle {
    /// Nominal diameter converted to mm.
    pub fn dn_mm(&self) -> f64 { self.nominal_diameter_in * 25.4 }
}

/// A Plant 3D equipment object from the PDS.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P3dEquipment {
    /// Plant 3D object handle (hex string, unique in DWG).
    pub handle: String,
    /// Tag number.
    pub tag_number: String,
    /// Plant 3D equipment class (from Equipment Engineering category).
    pub equipment_class: String,
    /// Description.
    pub description: Option<String>,
    /// P&ID reference tag.
    pub pid_tag: Option<String>,
    /// Line number(s) connected to this equipment.
    #[serde(default)]
    pub connected_lines: Vec<String>,
    /// Equipment nozzles.
    #[serde(default)]
    pub nozzles: Vec<P3dNozzle>,
    /// Design pressure [psi g].
    pub design_pressure_psig: Option<f64>,
    /// Design temperature [°F].
    pub design_temperature_f: Option<f64>,
    /// Operating pressure [psi g].
    pub operating_pressure_psig: Option<f64>,
    /// Operating temperature [°F].
    pub operating_temperature_f: Option<f64>,
    /// Material specification.
    pub material: Option<String>,
    /// Design code / standard.
    pub design_code: Option<String>,
    /// Manufacturer.
    pub manufacturer: Option<String>,
    /// Model.
    pub model: Option<String>,
    /// Weight [lbs].
    pub weight_lbs: Option<f64>,
    /// Motor power [hp].
    pub motor_power_hp: Option<f64>,
    /// Capacity / design flow [US gal/min].
    pub design_flow_gpm: Option<f64>,
    /// Design head [ft].
    pub design_head_ft: Option<f64>,
    /// Volume [US gal].
    pub volume_gal: Option<f64>,
    /// Heat duty [BTU/hr].
    pub heat_duty_btuh: Option<f64>,
    /// Heat transfer area [ft²].
    pub heat_transfer_area_ft2: Option<f64>,
    /// Bounding box min [mm].
    pub bbox_min_mm: Option<[f64; 3]>,
    /// Bounding box max [mm].
    pub bbox_max_mm: Option<[f64; 3]>,
    /// User-defined attributes.
    #[serde(default)]
    pub udas: HashMap<String, serde_json::Value>,
}

impl P3dEquipment {
    /// Design pressure converted to Pa absolute.
    pub fn design_pressure_pa(&self) -> Option<f64> {
        self.design_pressure_psig.map(|p| p * 6894.757 + 101_325.0)
    }

    /// Design temperature converted to K.
    pub fn design_temperature_k(&self) -> Option<f64> {
        self.design_temperature_f.map(|f| (f - 32.0) * 5.0 / 9.0 + 273.15)
    }

    /// Weight converted to kg.
    pub fn weight_kg(&self) -> Option<f64> {
        self.weight_lbs.map(|w| w * 0.453592)
    }

    /// Motor power converted to kW.
    pub fn motor_power_kw(&self) -> Option<f64> {
        self.motor_power_hp.map(|hp| hp * 0.7457)
    }

    /// Design flow converted to m³/h.
    pub fn design_flow_m3h(&self) -> Option<f64> {
        self.design_flow_gpm.map(|gpm| gpm * 0.227125)
    }

    /// Design head converted to m.
    pub fn design_head_m(&self) -> Option<f64> {
        self.design_head_ft.map(|ft| ft * 0.3048)
    }

    /// Volume converted to m³.
    pub fn volume_m3(&self) -> Option<f64> {
        self.volume_gal.map(|g| g * 0.003785)
    }

    /// Heat duty converted to W.
    pub fn heat_duty_w(&self) -> Option<f64> {
        self.heat_duty_btuh.map(|b| b * 0.29307)
    }

    /// Heat transfer area converted to m².
    pub fn heat_transfer_area_m2(&self) -> Option<f64> {
        self.heat_transfer_area_ft2.map(|a| a * 0.092903)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Equipment class mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map a Plant 3D equipment class string to (PMEF @type, equipmentClass).
///
/// Plant 3D uses human-readable class names from the Engineering
/// specification database (defined per project in the SpecEditor tool).
pub fn p3d_class_to_pmef(p3d_class: &str) -> (&'static str, &'static str) {
    let cls = p3d_class.trim().to_uppercase();
    let cls = cls.as_str();
    match cls {
        // ── Pumps ─────────────────────────────────────────────────────────────
        "CENTRIFUGAL PUMP" | "CENTRIFUGALPUMP" | "PUMP" | "PUMP-CENTRIF"
            => ("pmef:Pump", "CENTRIFUGAL_PUMP"),
        "RECIPROCATING PUMP" | "RECIP PUMP" | "PUMP-RECIP"
            => ("pmef:Pump", "RECIPROCATING_PUMP"),
        "GEAR PUMP" | "GEARPUMP" | "PUMP-GEAR"
            => ("pmef:Pump", "GEAR_PUMP"),
        "SCREW PUMP" | "SCREWPUMP"
            => ("pmef:Pump", "SCREW_PUMP"),
        "DIAPHRAGM PUMP" | "DIAPHRAGM"
            => ("pmef:Pump", "DIAPHRAGM_PUMP"),
        "SUBMERSIBLE PUMP"
            => ("pmef:Pump", "SUBMERSIBLE_PUMP"),

        // ── Compressors ───────────────────────────────────────────────────────
        "CENTRIFUGAL COMPRESSOR" | "COMPRESSOR" | "COMP-CENTRIF"
            => ("pmef:Compressor", "CENTRIFUGAL_COMPRESSOR"),
        "RECIPROCATING COMPRESSOR" | "COMP-RECIP"
            => ("pmef:Compressor", "RECIPROCATING_COMPRESSOR"),
        "SCREW COMPRESSOR"
            => ("pmef:Compressor", "SCREW_COMPRESSOR"),

        // ── Heat exchangers ───────────────────────────────────────────────────
        "SHELL AND TUBE HEAT EXCHANGER" | "HEAT EXCHANGER" | "HX" | "HE"
            => ("pmef:HeatExchanger", "SHELL_AND_TUBE_HEAT_EXCHANGER"),
        "PLATE HEAT EXCHANGER" | "PLATE HX" | "PHX"
            => ("pmef:HeatExchanger", "PLATE_HEAT_EXCHANGER"),
        "AIR COOLER" | "FIN FAN" | "AIR COOLED HX"
            => ("pmef:HeatExchanger", "AIR_COOLED_HEAT_EXCHANGER"),
        "REBOILER"
            => ("pmef:HeatExchanger", "SHELL_AND_TUBE_HEAT_EXCHANGER"),
        "CONDENSER"
            => ("pmef:HeatExchanger", "SHELL_AND_TUBE_HEAT_EXCHANGER"),

        // ── Vessels / drums ───────────────────────────────────────────────────
        "PRESSURE VESSEL" | "VESSEL" | "DRUM"
            => ("pmef:Vessel", "PRESSURE_VESSEL"),
        "KNOCKOUT DRUM" | "KO DRUM" | "FLASH DRUM"
            => ("pmef:Vessel", "KNOCK_OUT_DRUM"),
        "SEPARATOR" | "GAS LIQUID SEPARATOR"
            => ("pmef:Vessel", "SEPARATOR"),
        "ACCUMULATOR"
            => ("pmef:Vessel", "ACCUMULATOR"),
        "SCRUBBER"
            => ("pmef:Vessel", "SCRUBBER"),
        "ABSORBER"
            => ("pmef:Vessel", "ABSORBER"),

        // ── Columns ───────────────────────────────────────────────────────────
        "DISTILLATION COLUMN" | "COLUMN" | "TOWER"
            => ("pmef:Column", "DISTILLATION_COLUMN"),
        "ABSORPTION COLUMN" | "ABSORBER COLUMN"
            => ("pmef:Column", "ABSORPTION_COLUMN"),
        "STRIPPER"
            => ("pmef:Column", "STRIPPER_COLUMN"),

        // ── Tanks ─────────────────────────────────────────────────────────────
        "STORAGE TANK" | "TANK" | "ATMOSPHERIC TANK"
            => ("pmef:Tank", "STORAGE_TANK"),
        "FIXED ROOF TANK"
            => ("pmef:Tank", "FIXED_ROOF_TANK"),
        "FLOATING ROOF TANK"
            => ("pmef:Tank", "FLOATING_ROOF_TANK"),
        "SPHERICAL TANK" | "SPHERE"
            => ("pmef:Tank", "SPHERICAL_TANK"),
        "HORIZONTAL TANK" | "HORIZONTAL VESSEL"
            => ("pmef:Tank", "HORIZONTAL_TANK"),

        // ── Reactors ─────────────────────────────────────────────────────────
        "REACTOR"
            => ("pmef:Reactor", "FIXED_BED_REACTOR"),
        "ELECTRIC ARC FURNACE" | "EAF" | "FURNACE"
            => ("pmef:Reactor", "ELECTRIC_ARC_FURNACE"),
        "CONVERTER"
            => ("pmef:Reactor", "CONVERTER"),

        // ── Filters ───────────────────────────────────────────────────────────
        "STRAINER" | "FILTER" | "Y-STRAINER"
            => ("pmef:Filter", "Y_STRAINER"),
        "BASKET STRAINER"
            => ("pmef:Filter", "BASKET_STRAINER"),
        "CARTRIDGE FILTER"
            => ("pmef:Filter", "CARTRIDGE_FILTER"),

        // ── Turbines ─────────────────────────────────────────────────────────
        "STEAM TURBINE" | "TURBINE"
            => ("pmef:Turbine", "STEAM_TURBINE"),
        "GAS TURBINE"
            => ("pmef:Turbine", "GAS_TURBINE"),

        // ── Fallback ──────────────────────────────────────────────────────────
        _ => ("pmef:GenericEquipment", "GENERIC"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equipment_class_pump() {
        let (t, c) = p3d_class_to_pmef("Centrifugal Pump");
        assert_eq!(t, "pmef:Pump");
        assert_eq!(c, "CENTRIFUGAL_PUMP");
    }

    #[test]
    fn test_equipment_class_hx() {
        let (t, c) = p3d_class_to_pmef("HEAT EXCHANGER");
        assert_eq!(t, "pmef:HeatExchanger");
        assert_eq!(c, "SHELL_AND_TUBE_HEAT_EXCHANGER");
    }

    #[test]
    fn test_equipment_class_eaf() {
        let (t, c) = p3d_class_to_pmef("Electric Arc Furnace");
        assert_eq!(t, "pmef:Reactor");
        assert_eq!(c, "ELECTRIC_ARC_FURNACE");
    }

    #[test]
    fn test_equipment_class_fallback() {
        let (t, c) = p3d_class_to_pmef("UNKNOWN TYPE");
        assert_eq!(t, "pmef:GenericEquipment");
        assert_eq!(c, "GENERIC");
    }

    #[test]
    fn test_unit_conversions() {
        let equip = P3dEquipment {
            handle: "ABC123".to_owned(),
            tag_number: "P-201A".to_owned(),
            equipment_class: "Centrifugal Pump".to_owned(),
            description: None, pid_tag: None,
            connected_lines: vec![], nozzles: vec![],
            design_pressure_psig: Some(217.6),  // 15 barg
            design_temperature_f: Some(140.0),   // 60°C
            operating_pressure_psig: None, operating_temperature_f: None,
            material: None, design_code: None,
            manufacturer: None, model: None,
            weight_lbs: Some(4079.0),   // 1850 kg
            motor_power_hp: Some(73.8), // 55 kW
            design_flow_gpm: Some(1101.0), // 250 m³/h
            design_head_ft: Some(147.6),  // 45 m
            volume_gal: None, heat_duty_btuh: None, heat_transfer_area_ft2: None,
            bbox_min_mm: None, bbox_max_mm: None, udas: Default::default(),
        };

        // Pressure: 217.6 psig × 6894.757 + 101325 ≈ 1,601,325 Pa
        let dp = equip.design_pressure_pa().unwrap();
        assert!((dp - 1_601_325.0).abs() < 500.0, "Got {dp}");

        // Temperature: (140-32)*5/9 + 273.15 = 333.15 K
        let dt = equip.design_temperature_k().unwrap();
        assert!((dt - 333.15).abs() < 0.1, "Got {dt}");

        // Weight: 4079 × 0.453592 ≈ 1850 kg
        let wt = equip.weight_kg().unwrap();
        assert!((wt - 1850.0).abs() < 5.0, "Got {wt}");

        // Power: 73.8 × 0.7457 ≈ 55 kW
        let pw = equip.motor_power_kw().unwrap();
        assert!((pw - 55.0).abs() < 0.5, "Got {pw}");

        // Flow: 1101 × 0.227125 ≈ 250 m³/h
        let fl = equip.design_flow_m3h().unwrap();
        assert!((fl - 250.0).abs() < 2.0, "Got {fl}");

        // Head: 147.6 × 0.3048 ≈ 45 m
        let hd = equip.design_head_m().unwrap();
        assert!((hd - 45.0).abs() < 0.5, "Got {hd}");
    }

    #[test]
    fn test_nozzle_dn_mm() {
        let noz = P3dNozzle {
            nozzle_number: "N1".to_owned(), service: None,
            nominal_diameter_in: 8.0,
            flange_rating: Some("150".to_owned()), facing_type: Some("RF".to_owned()),
            position_mm: [0.0, 0.0, 0.0], direction: [-1.0, 0.0, 0.0],
            connected_line_tag: None,
        };
        assert!((noz.dn_mm() - 203.2).abs() < 0.1);
    }
}
