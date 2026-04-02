//! COMOS class → PMEF mapping tables.
//!
//! COMOS uses a hierarchical class system rooted at `@` (the root class).
//! Each class is identified by a short code (e.g. `@E03`, `@I10`).
//! Class codes vary slightly between projects due to customisation,
//! but the standard Siemens class library is used here.

// ─────────────────────────────────────────────────────────────────────────────
// Equipment class mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map a COMOS class code to (PMEF @type, equipmentClass).
///
/// The COMOS standard class library organises equipment under `@E`:
/// - `@E01` = generic process unit
/// - `@E02` = column / tower
/// - `@E03` = pump (centrifugal)
/// - `@E03.1` = pump (positive displacement)
/// - `@E04` = compressor
/// - `@E05` = heat exchanger
/// - `@E06` = reactor / vessel
/// - `@E07` = pressure vessel / drum
/// - `@E08` = tank (atmospheric)
/// - `@E09` = filter / strainer
/// - `@E10` = turbine / driver
/// - `@E11` = furnace / fired heater
pub fn comos_class_to_equipment(comos_class: &str) -> (&'static str, &'static str) {
    // Normalise: strip trailing whitespace, lowercase
    let cls = comos_class.trim().to_uppercase();
    let cls = cls.as_str();

    match cls {
        // ── Pumps ─────────────────────────────────────────────────────────────
        "@E03" | "@E03.0"   => ("pmef:Pump", "CENTRIFUGAL_PUMP"),
        "@E03.1"             => ("pmef:Pump", "RECIPROCATING_PUMP"),
        "@E03.2"             => ("pmef:Pump", "GEAR_PUMP"),
        "@E03.3"             => ("pmef:Pump", "SCREW_PUMP"),
        "@E03.4"             => ("pmef:Pump", "DIAPHRAGM_PUMP"),
        "@E03.5"             => ("pmef:Pump", "SUBMERSIBLE_PUMP"),

        // ── Compressors ───────────────────────────────────────────────────────
        "@E04" | "@E04.0"   => ("pmef:Compressor", "CENTRIFUGAL_COMPRESSOR"),
        "@E04.1"             => ("pmef:Compressor", "RECIPROCATING_COMPRESSOR"),
        "@E04.2"             => ("pmef:Compressor", "SCREW_COMPRESSOR"),
        "@E04.3"             => ("pmef:Compressor", "LOBE_COMPRESSOR"),

        // ── Heat exchangers ───────────────────────────────────────────────────
        "@E05" | "@E05.0"   => ("pmef:HeatExchanger", "SHELL_AND_TUBE_HEAT_EXCHANGER"),
        "@E05.1"             => ("pmef:HeatExchanger", "PLATE_HEAT_EXCHANGER"),
        "@E05.2"             => ("pmef:HeatExchanger", "AIR_COOLED_HEAT_EXCHANGER"),
        "@E05.3"             => ("pmef:HeatExchanger", "DOUBLE_WALL_HEAT_EXCHANGER"),
        "@E05.4"             => ("pmef:HeatExchanger", "SPIRAL_HEAT_EXCHANGER"),

        // ── Reactors / vessels / drums ────────────────────────────────────────
        "@E06" | "@E06.0"   => ("pmef:Reactor",  "FIXED_BED_REACTOR"),
        "@E06.1"             => ("pmef:Reactor",  "FLUIDISED_BED_REACTOR"),
        "@E06.2"             => ("pmef:Reactor",  "STIRRED_TANK_REACTOR"),
        "@E06.3"             => ("pmef:Reactor",  "ELECTRIC_ARC_FURNACE"),
        "@E07" | "@E07.0"   => ("pmef:Vessel",   "PRESSURE_VESSEL"),
        "@E07.1"             => ("pmef:Vessel",   "KNOCK_OUT_DRUM"),
        "@E07.2"             => ("pmef:Vessel",   "SEPARATOR"),
        "@E07.3"             => ("pmef:Vessel",   "ACCUMULATOR"),
        "@E07.4"             => ("pmef:Vessel",   "SCRUBBER"),

        // ── Columns / towers ─────────────────────────────────────────────────
        "@E02" | "@E02.0"   => ("pmef:Column", "DISTILLATION_COLUMN"),
        "@E02.1"             => ("pmef:Column", "ABSORPTION_COLUMN"),
        "@E02.2"             => ("pmef:Column", "PACKED_COLUMN"),
        "@E02.3"             => ("pmef:Column", "STRIPPER_COLUMN"),

        // ── Tanks ─────────────────────────────────────────────────────────────
        "@E08" | "@E08.0"   => ("pmef:Tank", "FIXED_ROOF_TANK"),
        "@E08.1"             => ("pmef:Tank", "FLOATING_ROOF_TANK"),
        "@E08.2"             => ("pmef:Tank", "SPHERICAL_TANK"),
        "@E08.3"             => ("pmef:Tank", "HORIZONTAL_TANK"),
        "@E08.4"             => ("pmef:Tank", "DAY_TANK"),

        // ── Filters / strainers ───────────────────────────────────────────────
        "@E09" | "@E09.0"   => ("pmef:Filter", "STRAINER"),
        "@E09.1"             => ("pmef:Filter", "BASKET_STRAINER"),
        "@E09.2"             => ("pmef:Filter", "Y_STRAINER"),
        "@E09.3"             => ("pmef:Filter", "CARTRIDGE_FILTER"),
        "@E09.4"             => ("pmef:Filter", "BAG_FILTER"),

        // ── Turbines ─────────────────────────────────────────────────────────
        "@E10" | "@E10.0"   => ("pmef:Turbine", "STEAM_TURBINE"),
        "@E10.1"             => ("pmef:Turbine", "GAS_TURBINE"),
        "@E10.2"             => ("pmef:Turbine", "STEAM_EXPANDER"),

        // ── Furnaces / fired heaters ─────────────────────────────────────────
        "@E11" | "@E11.0"   => ("pmef:Reactor", "FIRED_HEATER"),

        // ── Generic ───────────────────────────────────────────────────────────
        _ => ("pmef:GenericEquipment", "GENERIC"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Instrument class mapping
// ─────────────────────────────────────────────────────────────────────────────

/// A PMEF instrument mapping result.
pub struct InstrumentMapping {
    pub pmef_type: &'static str,
    pub instrument_class: &'static str,
    pub process_variable: Option<&'static str>,
}

/// Map a COMOS instrument class to PMEF instrument attributes.
///
/// The COMOS standard class library organises instruments under `@I`:
/// - `@I05` = instrument loop
/// - `@I10` = measuring element / transmitter
/// - `@I10.F` = flow transmitter
/// - `@I10.P` = pressure transmitter
/// - `@I10.T` = temperature transmitter
/// - `@I10.L` = level transmitter
/// - `@I10.A` = analysis transmitter
/// - `@I20` = controller
/// - `@I30` = final element (control valve, positioner)
/// - `@I30.V` = control valve
/// - `@I40` = safety element (SIS)
/// - `@I40.V` = safety valve / PRV
/// - `@I50` = local indicator / gauge
/// - `@I60` = switch
pub fn comos_class_to_instrument(comos_class: &str) -> InstrumentMapping {
    let cls = comos_class.trim().to_uppercase();
    match cls.as_str() {
        "@I10" | "@I10.0"    => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"TRANSMITTER",          process_variable:None },
        "@I10.F"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"TRANSMITTER",          process_variable:Some("FLOW") },
        "@I10.P"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"TRANSMITTER",          process_variable:Some("PRESSURE") },
        "@I10.T"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"TRANSMITTER",          process_variable:Some("TEMPERATURE") },
        "@I10.L"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"TRANSMITTER",          process_variable:Some("LEVEL") },
        "@I10.A"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"TRANSMITTER",          process_variable:Some("ANALYSIS") },
        "@I10.S"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"TRANSMITTER",          process_variable:Some("SPEED") },
        "@I20" | "@I20.0"    => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"CONTROLLER",           process_variable:None },
        "@I30" | "@I30.0"    => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"VALVE_CONTROL",        process_variable:None },
        "@I30.V"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"VALVE_CONTROL",        process_variable:None },
        "@I30.M"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"MOTOR_CONTROL",        process_variable:None },
        "@I40" | "@I40.0"    => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"SAFETY_ELEMENT",       process_variable:None },
        "@I40.V"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"SAFETY_ELEMENT",       process_variable:None },
        "@I40.F"              => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"SAFETY_ELEMENT",       process_variable:Some("FLOW") },
        "@I50" | "@I50.0"    => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"INDICATOR",            process_variable:None },
        "@I60" | "@I60.0"    => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"SWITCH",               process_variable:None },
        _                     => InstrumentMapping { pmef_type:"pmef:InstrumentObject", instrument_class:"TRANSMITTER",          process_variable:None },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PLC class mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map a COMOS PLC class code to a PMEF PLCObject class.
pub fn comos_class_to_plc(comos_class: &str) -> &'static str {
    let cls = comos_class.trim().to_uppercase();
    match cls.as_str() {
        "@S10" | "@S10.0" => "CPU",
        "@S10.1"           => "SAFETY_CPU",
        "@S20" | "@S20.0" => "IO_MODULE",
        "@S20.1"           => "ANALOG_INPUT_MODULE",
        "@S20.2"           => "ANALOG_OUTPUT_MODULE",
        "@S20.3"           => "DIGITAL_INPUT_MODULE",
        "@S20.4"           => "DIGITAL_OUTPUT_MODULE",
        "@S30" | "@S30.0" => "NETWORK_SWITCH",
        "@S40" | "@S40.0" => "HMI",
        _                  => "CPU",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit conversion
// ─────────────────────────────────────────────────────────────────────────────

/// Convert bar gauge → Pa absolute.
pub fn barg_to_pa_abs(barg: f64) -> f64 { barg * 100_000.0 + 101_325.0 }

/// Convert °C → K.
pub fn degc_to_k(degc: f64) -> f64 { degc + 273.15 }

/// Convert kW → W.
pub fn kw_to_w(kw: f64) -> f64 { kw * 1_000.0 }

/// Convert m³/h → m³/s.
pub fn m3h_to_m3s(m3h: f64) -> f64 { m3h / 3_600.0 }

// ─────────────────────────────────────────────────────────────────────────────
// COMOS loop type classification
// ─────────────────────────────────────────────────────────────────────────────

/// Derive a PMEF loop type from the COMOS loop number convention.
/// Instrument loop numbers follow ISA 5.1: first letter = process variable.
pub fn loop_type_from_number(loop_number: &str) -> &'static str {
    // Skip leading numbers (if any) and find first letter
    let first_letter = loop_number.chars().find(|c| c.is_ascii_alphabetic());
    match first_letter {
        Some('F') | Some('f') => "FLOW_CONTROL",
        Some('P') | Some('p') => "PRESSURE_CONTROL",
        Some('T') | Some('t') => "TEMPERATURE_CONTROL",
        Some('L') | Some('l') => "LEVEL_CONTROL",
        Some('A') | Some('a') => "ANALYSIS",
        Some('S') | Some('s') => "SPEED_CONTROL",
        Some('Z') | Some('z') => "POSITION_CONTROL",
        _                      => "GENERIC",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Material map
// ─────────────────────────────────────────────────────────────────────────────

/// Map a COMOS material attribute value to a PMEF material string.
pub fn comos_material_to_pmef(mat: &str) -> &str {
    match mat.trim() {
        "CS" | "A106B" | "Carbon steel"    => "ASTM A106 Gr. B",
        "SS316L" | "Stainless 316L"        => "ASTM A312 TP316L",
        "SS304L" | "Stainless 304L"        => "ASTM A312 TP304L",
        "P265GH" | "P235GH"                => "EN 10216-2 P265GH",
        "SA516-70" | "SA516 Gr 70"         => "ASTM A516 Gr. 70",
        "Hastelloy C276"                   => "Hastelloy C-276",
        "Inconel 625"                      => "Inconel 625",
        "Duplex 2205" | "1.4462"           => "EN 10216-5 X2CrNiMoN22-5-3",
        _ => mat,
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
        let (t, c) = comos_class_to_equipment("@E03");
        assert_eq!(t, "pmef:Pump");
        assert_eq!(c, "CENTRIFUGAL_PUMP");
    }

    #[test]
    fn test_equipment_class_hx() {
        let (t, c) = comos_class_to_equipment("@E05");
        assert_eq!(t, "pmef:HeatExchanger");
        assert_eq!(c, "SHELL_AND_TUBE_HEAT_EXCHANGER");
    }

    #[test]
    fn test_equipment_class_plate_hx() {
        let (t, c) = comos_class_to_equipment("@E05.1");
        assert_eq!(t, "pmef:HeatExchanger");
        assert_eq!(c, "PLATE_HEAT_EXCHANGER");
    }

    #[test]
    fn test_equipment_class_eaf() {
        let (t, c) = comos_class_to_equipment("@E06.3");
        assert_eq!(t, "pmef:Reactor");
        assert_eq!(c, "ELECTRIC_ARC_FURNACE");
    }

    #[test]
    fn test_equipment_class_vessel() {
        let (t, c) = comos_class_to_equipment("@E07");
        assert_eq!(t, "pmef:Vessel");
        assert_eq!(c, "PRESSURE_VESSEL");
    }

    #[test]
    fn test_equipment_class_unknown() {
        let (t, c) = comos_class_to_equipment("@ZZUNKNOWN");
        assert_eq!(t, "pmef:GenericEquipment");
        assert_eq!(c, "GENERIC");
    }

    #[test]
    fn test_instrument_class_flow_transmitter() {
        let m = comos_class_to_instrument("@I10.F");
        assert_eq!(m.pmef_type, "pmef:InstrumentObject");
        assert_eq!(m.instrument_class, "TRANSMITTER");
        assert_eq!(m.process_variable, Some("FLOW"));
    }

    #[test]
    fn test_instrument_class_controller() {
        let m = comos_class_to_instrument("@I20");
        assert_eq!(m.instrument_class, "CONTROLLER");
        assert!(m.process_variable.is_none());
    }

    #[test]
    fn test_instrument_class_safety() {
        let m = comos_class_to_instrument("@I40");
        assert_eq!(m.instrument_class, "SAFETY_ELEMENT");
    }

    #[test]
    fn test_instrument_class_control_valve() {
        let m = comos_class_to_instrument("@I30.V");
        assert_eq!(m.instrument_class, "VALVE_CONTROL");
    }

    #[test]
    fn test_plc_class_cpu() {
        assert_eq!(comos_class_to_plc("@S10"), "CPU");
        assert_eq!(comos_class_to_plc("@S10.1"), "SAFETY_CPU");
        assert_eq!(comos_class_to_plc("@S20.1"), "ANALOG_INPUT_MODULE");
    }

    #[test]
    fn test_unit_conversions() {
        // 15 barg → ~1.601 MPa abs
        let pa = barg_to_pa_abs(15.0);
        assert!((pa - 1_601_325.0).abs() < 10.0, "Got {pa}");
        // 0 barg → atmospheric
        assert!((barg_to_pa_abs(0.0) - 101_325.0).abs() < 1.0);
        // 60°C → 333.15 K
        assert!((degc_to_k(60.0) - 333.15).abs() < 0.01);
        // 0°C → 273.15 K
        assert!((degc_to_k(0.0) - 273.15).abs() < 0.01);
        // 100 kW → 100,000 W
        assert!((kw_to_w(100.0) - 100_000.0).abs() < 0.001);
    }

    #[test]
    fn test_loop_type_from_number() {
        assert_eq!(loop_type_from_number("FIC-10101"), "FLOW_CONTROL");
        assert_eq!(loop_type_from_number("PIC-20201"), "PRESSURE_CONTROL");
        assert_eq!(loop_type_from_number("TIC-30301"), "TEMPERATURE_CONTROL");
        assert_eq!(loop_type_from_number("LIC-40401"), "LEVEL_CONTROL");
        assert_eq!(loop_type_from_number("10101FIC"), "FLOW_CONTROL");
        assert_eq!(loop_type_from_number("UNKNOWN"),  "GENERIC");
    }

    #[test]
    fn test_material_mapping() {
        assert_eq!(comos_material_to_pmef("CS"), "ASTM A106 Gr. B");
        assert_eq!(comos_material_to_pmef("SS316L"), "ASTM A312 TP316L");
        assert_eq!(comos_material_to_pmef("P265GH"), "EN 10216-2 P265GH");
        assert_eq!(comos_material_to_pmef("EXOTIC"), "EXOTIC"); // passthrough
    }
}
