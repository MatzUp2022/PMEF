//! AVEVA E3D → PMEF field mapping tables.

use std::collections::HashMap;
use std::sync::OnceLock;

// ── Component type mapping ────────────────────────────────────────────────────

/// Catalog that maps E3D element types to PMEF (`catalogs/native-types-e3d.json`).
pub const NATIVE_TYPES_CATALOG: &str = "native-types-e3d";

fn native_types() -> &'static [pmef_catalogs::NativeTypeMapping] {
    static TYPES: OnceLock<Vec<pmef_catalogs::NativeTypeMapping>> = OnceLock::new();
    TYPES.get_or_init(|| pmef_catalogs::native_types(NATIVE_TYPES_CATALOG))
}

/// True if `keyword` is a piping element type listed in [`NATIVE_TYPES_CATALOG`].
pub fn is_native_piping_type(keyword: &str) -> bool {
    native_types().iter().any(|m| m.native_type.eq_ignore_ascii_case(keyword))
}

/// Map an E3D element type + SKEY to (PMEF @type, componentClass, listed).
///
/// E3D uses two-level identification: the PML element keyword (ELBO, VALV, FLAN, ...)
/// and the 8-character specification key (SKEY). The mapping is data, not code: the
/// first matching entry of [`NATIVE_TYPES_CATALOG`] wins. An element type that is not
/// listed maps to `pmef:GenericPipingComponent` / `SPECIAL` with `listed = false`, so
/// the caller can warn instead of dropping or mislabelling the component.
pub fn e3d_element_to_pmef(
    element_keyword: &str,
    skey: Option<&str>,
) -> (&'static str, &'static str, bool) {
    match native_types().iter().find(|m| m.applies(element_keyword, skey)) {
        Some(m) => (
            m.pmef_type.as_str(),
            m.component_class.as_deref().unwrap_or("SPECIAL"),
            true,
        ),
        None => ("pmef:GenericPipingComponent", "SPECIAL", false),
    }
}

// ── Equipment type mapping ────────────────────────────────────────────────────

/// Map an E3D DTYP (design type) attribute to (PMEF @type, equipmentClass).
pub fn e3d_dtyp_to_pmef(dtyp: &str) -> (&'static str, &'static str) {
    match dtyp.to_uppercase().replace(['-', '_', ' '], "").as_str() {
        "CENTRIFUGALPUMP" | "PUMP"         => ("pmef:Pump",         "CENTRIFUGAL_PUMP"),
        "RECIPROCATINGPUMP"                 => ("pmef:Pump",         "RECIPROCATING_PUMP"),
        "GEARPUMP"                          => ("pmef:Pump",         "GEAR_PUMP"),
        "CENTRIFUGALCOMPRESSOR"            => ("pmef:Compressor",    "CENTRIFUGAL_COMPRESSOR"),
        "RECIPROCATINGCOMPRESSOR"           => ("pmef:Compressor",    "RECIPROCATING_COMPRESSOR"),
        "PRESSUREVESSEL" | "VESSEL"        => ("pmef:Vessel",        "PRESSURE_VESSEL"),
        "DRUM" | "KNOCKOUTDRUM"            => ("pmef:Vessel",        "KNOCK_OUT_DRUM"),
        "ACCUMULATOR"                       => ("pmef:Vessel",        "ACCUMULATOR"),
        "SEPARATOR"                         => ("pmef:Vessel",        "SEPARATOR"),
        "STORAGETANK" | "TANK"             => ("pmef:Tank",          "STORAGE_TANK"),
        "SHELLANDTUBEHX" | "HEATEXCHANGER"=> ("pmef:HeatExchanger", "SHELL_AND_TUBE_HEAT_EXCHANGER"),
        "PLATEHX"                           => ("pmef:HeatExchanger", "PLATE_HEAT_EXCHANGER"),
        "DISTILLATIONCOLUMN" | "COLUMN"    => ("pmef:Column",        "DISTILLATION_COLUMN"),
        "REACTOR"                           => ("pmef:Reactor",       "FIXED_BED_REACTOR"),
        "ELECTRICARCFURNACE" | "EAF"       => ("pmef:Reactor",       "ELECTRIC_ARC_FURNACE"),
        "FILTER" | "STRAINER"              => ("pmef:Filter",        "STRAINER"),
        "YSTRAINER"                         => ("pmef:Filter",        "Y_STRAINER"),
        "STEAMTURBINE"                      => ("pmef:Turbine",       "STEAM_TURBINE"),
        _                                   => ("pmef:GenericEquipment", "GENERIC"),
    }
}

// ── Material mapping ──────────────────────────────────────────────────────────

/// Map an E3D MATI (material) code to a PMEF material string.
pub fn e3d_material_to_pmef(mati: &str) -> &str {
    match mati.trim().to_uppercase().as_str() {
        "CS" | "A106B" | "A106GRB"        => "ASTM A106 Gr. B",
        "SS316L" | "A312316L"             => "ASTM A312 TP316L",
        "SS304L" | "A312304L"             => "ASTM A312 TP304L",
        "A335P11"                          => "ASTM A335 Gr. P11",
        "A335P22"                          => "ASTM A335 Gr. P22",
        "A234WPB" | "WPBFITTING"          => "ASTM A234 WPB",
        "A105" | "FORGINGA105"            => "ASTM A105",
        "A216WCB" | "CASTINGWCB"          => "ASTM A216 WCB",
        "P265GH" | "EN10216P265GH"        => "EN 10216-2 P265GH",
        "SA516GR70" | "SA51670"           => "ASTM A516 Gr. 70",
        _ => mati,
    }
}

// ── SKEY → flange type ────────────────────────────────────────────────────────

/// Derive a PMEF `flangeType` string from an E3D SKEY.
pub fn skey_to_flange_type(skey: &str) -> &'static str {
    let s = skey.to_uppercase();
    if s.starts_with("FLWN")      { "WELD_NECK" }
    else if s.starts_with("FLBL") { "BLIND" }
    else if s.starts_with("FLSO") { "SLIP_ON" }
    else if s.starts_with("FLSW") { "SOCKET_WELD" }
    else if s.starts_with("FLLJ") { "LAP_JOINT" }
    else if s.starts_with("FLOR") { "ORIFICE" }
    else                           { "WELD_NECK" }
}

/// Derive PMEF elbow radius enum from SKEY + actual radius vs DN.
pub fn classify_elbow_radius(skey: Option<&str>, radius_mm: f64, dn_mm: f64) -> &'static str {
    // Check SKEY first (most reliable)
    if let Some(sk) = skey {
        let s = sk.to_uppercase();
        if s.contains("LR") || s.contains("LNG") { return "LONG_RADIUS"; }
        if s.contains("SR") || s.contains("SHT") { return "SHORT_RADIUS"; }
    }
    // Fallback: classify by ratio r/dn
    let ratio = radius_mm / dn_mm;
    if (ratio - 1.5).abs() < 0.15 { "LONG_RADIUS" }
    else if (ratio - 1.0).abs() < 0.15 { "SHORT_RADIUS" }
    else if (ratio - 3.0).abs() < 0.15 { "3D" }
    else if (ratio - 5.0).abs() < 0.15 { "5D" }
    else { "CUSTOM" }
}

// ── Support type mapping ──────────────────────────────────────────────────────

/// Map E3D PSUP specification key to PMEF SupportType.
pub fn e3d_psup_to_support_type(skey: &str) -> &'static str {
    let s = skey.to_uppercase();
    if s.contains("ANCH")  { "ANCHOR" }
    else if s.contains("GUID") { "GUIDE" }
    else if s.contains("STOP") { "STOP" }
    else if s.contains("HANG") || s.contains("HNGR") { "SPRING_VARIABLE" }
    else if s.contains("CONS") || s.contains("CNST") { "SPRING_CONSTANT" }
    else if s.contains("SWAY")  { "SWAY" }
    else { "RESTING" }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_e3d_element_to_pmef() {
        let cases = [
            ("ELBOW", Some("ELBWLR90"), "pmef:Elbow", "ELBOW"),
            ("ELBO", None, "pmef:Elbow", "ELBOW"),
            ("VALV", Some("GTBWFLFL"), "pmef:Valve", "VALVE_GATE"),
            ("VALV", Some("BLBWFLFL"), "pmef:Valve", "VALVE_BALL"),
            ("VALV", Some("XXBWFLFL"), "pmef:Valve", "VALVE_UNSPECIFIED"),
            ("FLAN", Some("FLBLRF  "), "pmef:Flange", "BLIND_FLANGE"),
            ("FLAN", Some("FLWNRF  "), "pmef:Flange", "FLANGE"),
            ("REDU", Some("RDCWECCT"), "pmef:Reducer", "REDUCER_ECCENTRIC"),
            ("TUBI", None, "pmef:Pipe", "PIPE"),
            ("COUP", None, "pmef:GenericPipingComponent", "COUPLING"),
            ("INST", None, "pmef:GenericPipingComponent", "INLINE_INSTRUMENT"),
            ("CLOS", None, "pmef:GenericPipingComponent", "CLOSURE"),
            ("PCOM", Some("ANY"), "pmef:GenericPipingComponent", "SPECIAL"),
        ];
        for (kw, skey, t, c) in cases {
            assert_eq!(e3d_element_to_pmef(kw, skey), (t, c, true), "{kw} {skey:?}");
        }
        assert_eq!(
            e3d_element_to_pmef("XYZZ", None),
            ("pmef:GenericPipingComponent", "SPECIAL", false)
        );
        assert!(is_native_piping_type("tubi") && !is_native_piping_type("XYZZ"));
    }

    #[test]
    fn test_e3d_dtyp_to_pmef() {
        let (t, c) = e3d_dtyp_to_pmef("CENTRIFUGAL_PUMP");
        assert_eq!(t, "pmef:Pump");
        assert_eq!(c, "CENTRIFUGAL_PUMP");

        let (t, c) = e3d_dtyp_to_pmef("EAF");
        assert_eq!(t, "pmef:Reactor");
        assert_eq!(c, "ELECTRIC_ARC_FURNACE");

        let (t, c) = e3d_dtyp_to_pmef("UnknownType");
        assert_eq!(t, "pmef:GenericEquipment");
        assert_eq!(c, "GENERIC");
    }

    #[test]
    fn test_e3d_material_to_pmef() {
        assert_eq!(e3d_material_to_pmef("A106B"), "ASTM A106 Gr. B");
        assert_eq!(e3d_material_to_pmef("CS"), "ASTM A106 Gr. B");
        assert_eq!(e3d_material_to_pmef("SS316L"), "ASTM A312 TP316L");
        assert_eq!(e3d_material_to_pmef("P265GH"), "EN 10216-2 P265GH");
        assert_eq!(e3d_material_to_pmef("EXOTIC"), "EXOTIC"); // passthrough
    }

    #[test]
    fn test_classify_elbow_radius() {
        assert_eq!(classify_elbow_radius(Some("ELBWLR90"), 300., 200.), "LONG_RADIUS");
        assert_eq!(classify_elbow_radius(Some("ELBWSR90"), 200., 200.), "SHORT_RADIUS");
        // Without SKEY, classify by ratio
        assert_eq!(classify_elbow_radius(None, 300., 200.), "LONG_RADIUS"); // 1.5×DN
        assert_eq!(classify_elbow_radius(None, 200., 200.), "SHORT_RADIUS"); // 1.0×DN
        assert_eq!(classify_elbow_radius(None, 600., 200.), "3D"); // 3.0×DN
        assert_eq!(classify_elbow_radius(None, 1000., 200.), "5D"); // 5.0×DN
        assert_eq!(classify_elbow_radius(None, 750., 200.), "CUSTOM"); // 3.75×DN
    }

    #[test]
    fn test_skey_to_flange_type() {
        assert_eq!(skey_to_flange_type("FLWNRF  "), "WELD_NECK");
        assert_eq!(skey_to_flange_type("FLBLRF  "), "BLIND");
        assert_eq!(skey_to_flange_type("FLSORF  "), "SLIP_ON");
        assert_eq!(skey_to_flange_type("UNKNOWN "), "WELD_NECK"); // fallback
    }

    #[test]
    fn test_e3d_psup_to_support_type() {
        assert_eq!(e3d_psup_to_support_type("ANCHRW  "), "ANCHOR");
        assert_eq!(e3d_psup_to_support_type("GUIDERW "), "GUIDE");
        assert_eq!(e3d_psup_to_support_type("HANGRW  "), "SPRING_VARIABLE");
        assert_eq!(e3d_psup_to_support_type("SUPRW   "), "RESTING"); // default
    }
}
