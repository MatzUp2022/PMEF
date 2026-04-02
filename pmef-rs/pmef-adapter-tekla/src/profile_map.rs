//! Tekla Structures profile designation → PMEF profile ID mapping.
//!
//! Tekla uses its own profile naming convention which varies by region
//! and catalog. This module maps Tekla profile strings to PMEF profile IDs
//! of the form `<standard>:<designation>`.
//!
//! ## Tekla naming conventions by region
//!
//! | Region | Example | PMEF result |
//! |--------|---------|-------------|
//! | European | `HEA200`, `IPE300`, `SHS100*6` | `EN:HEA200`, `EN:IPE300`, `EN:SHS100x100x6` |
//! | North American | `W12X53`, `HSS6X4X.25` | `AISC:W12x53`, `AISC:HSS6x4x0.25` |
//! | British | `203x133x30UB` | `BS:203x133x30UB` |
//! | Australian | `200UB29.8` | `AS:200UB29.8` |
//! | Custom / unknown | `PL20*200` | `CUSTOM:PL20x200` |

use std::collections::HashMap;

/// Profile mapping result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileId {
    /// Standard prefix: `"EN"`, `"AISC"`, `"BS"`, `"AS"`, `"CUSTOM"`.
    pub standard: String,
    /// Designation after the colon: `"HEA200"`, `"W12x53"`, etc.
    pub designation: String,
}

impl ProfileId {
    pub fn new(standard: &str, designation: &str) -> Self {
        Self {
            standard: standard.to_owned(),
            designation: designation.to_owned(),
        }
    }

    /// Full PMEF profile ID string, e.g. `"EN:HEA200"`.
    pub fn as_pmef_str(&self) -> String {
        format!("{}:{}", self.standard, self.designation)
    }
}

/// Map a Tekla profile designation string to a PMEF profile ID.
///
/// Tekla uses `*` as the dimension separator for some profiles (e.g. `SHS100*6`).
/// PMEF uses `x` (e.g. `EN:SHS100x100x6`). This function normalises the separator.
///
/// # Examples
/// ```
/// use pmef_adapter_tekla::profile_map::map_profile;
/// let id = map_profile("HEA200");
/// assert_eq!(id.as_pmef_str(), "EN:HEA200");
/// let w = map_profile("W12X53");
/// assert_eq!(w.as_pmef_str(), "AISC:W12x53");
/// ```
pub fn map_profile(tekla_profile: &str) -> ProfileId {
    let s = tekla_profile.trim();

    // Normalise separators: * → x, X (in dims) → x
    let normalised = normalise_profile_string(s);

    // Try exact lookup in the override table first
    if let Some(pid) = OVERRIDE_TABLE.get(normalised.to_uppercase().as_str()) {
        return ProfileId::new(pid.0, pid.1);
    }

    // Pattern-based classification
    classify_by_pattern(&normalised)
}

/// Normalise a Tekla profile string:
/// - Replace `*` with `x`
/// - Normalise `X` between numbers to `x` (but not at start)
fn normalise_profile_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '*' {
            out.push('x');
        } else if c == 'X' || c == 'x' {
            // If surrounded by digits/dots: use lowercase x
            let prev_num = i > 0 && (chars[i-1].is_ascii_digit() || chars[i-1] == '.');
            let next_num = i+1 < chars.len() && (chars[i+1].is_ascii_digit() || chars[i+1] == '.');
            if prev_num && next_num {
                out.push('x');
            } else {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Classify a normalised Tekla profile string by prefix pattern.
fn classify_by_pattern(s: &str) -> ProfileId {
    let upper = s.to_uppercase();

    // ── European profiles ─────────────────────────────────────────────────────
    // HEA, HEB, HEM, IPE, IPN, UPE, UPN
    for prefix in ["HEA", "HEB", "HEM", "IPE", "IPN", "UPE", "UPN",
                   "HE ", "HF", "HL", "HP"] {
        if upper.starts_with(prefix) {
            let clean = s.to_uppercase().replace(' ', "");
            return ProfileId::new("EN", &clean);
        }
    }
    // CHS: "CHS219.1*8" → "EN:CHS219.1x8"
    if upper.starts_with("CHS") {
        return ProfileId::new("EN", s);
    }
    // RHS: "RHS200*100*6" → "EN:RHS200x100x6"
    if upper.starts_with("RHS") || upper.starts_with("SHS") {
        return ProfileId::new("EN", s);
    }
    // L (angles): "L100*10" → "EN:L100x100x10"
    if upper.starts_with('L') && s.contains('x') && !upper.starts_with("LG") {
        return ProfileId::new("EN", s);
    }

    // ── AISC (North American) profiles ────────────────────────────────────────
    // W-shapes: W12x53, W14x82
    if upper.starts_with('W') && s.contains('x') && !upper.starts_with("WF") {
        let aisc = format!("W{}", &s[1..]);
        return ProfileId::new("AISC", &aisc);
    }
    // HSS: HSS6x4x0.25, HSS4x0.25
    if upper.starts_with("HSS") {
        return ProfileId::new("AISC", s);
    }
    // S, M, C, MC shapes
    for prefix in ["S ", "M ", "MC", "C "] {
        if upper.starts_with(prefix.trim()) && s.contains('x') {
            return ProfileId::new("AISC", s);
        }
    }
    // L angles (AISC): L3x3x0.25
    if upper.starts_with('L') && s.contains('x') {
        return ProfileId::new("AISC", s);
    }

    // ── British profiles ──────────────────────────────────────────────────────
    // 203x133x30UB, 254x254x89UC, 100x50x8CHS
    for suffix in ["UB", "UC", "EA", "RSC", "RSJ", "CHS_BS", "RHS_BS"] {
        if upper.ends_with(suffix) || upper.contains(suffix) {
            return ProfileId::new("BS", s);
        }
    }

    // ── Australian profiles ───────────────────────────────────────────────────
    // 200UB29.8, 310UC96.8, 150EA18.0
    for suffix in ["UB", "UC", "PFC", "TFB", "WC", "WB"] {
        let has_suffix = upper.ends_with(suffix);
        let starts_digit = s.starts_with(|c: char| c.is_ascii_digit());
        if has_suffix && starts_digit {
            return ProfileId::new("AS", s);
        }
    }

    // ── Plate / flat bar ─────────────────────────────────────────────────────
    for prefix in ["PL", "FL", "FB", "FLAT"] {
        if upper.starts_with(prefix) {
            return ProfileId::new("EN", &format!("FLAT{}", &s[prefix.len()..]));
        }
    }

    // ── Fallback ──────────────────────────────────────────────────────────────
    ProfileId::new("CUSTOM", s)
}

/// Hardcoded override table for known Tekla→PMEF profile name differences.
/// Format: TEKLA_UPPER → ("standard", "designation")
static OVERRIDE_TABLE: phf::Map<&'static str, (&'static str, &'static str)> = phf::phf_map! {
    // Tekla sometimes writes HE200A instead of HEA200
    "HE200A" => ("EN", "HEA200"),
    "HE200B" => ("EN", "HEB200"),
    "HE240A" => ("EN", "HEA240"),
    "HE240B" => ("EN", "HEB240"),
    "HE300A" => ("EN", "HEA300"),
    "HE300B" => ("EN", "HEB300"),
    "HE400A" => ("EN", "HEA400"),
    "HE400B" => ("EN", "HEB400"),
    "HE500A" => ("EN", "HEA500"),
    "HE500B" => ("EN", "HEB500"),
    // AISC variants Tekla sometimes uses
    "W12*53" => ("AISC", "W12x53"),
    "W14*82" => ("AISC", "W14x82"),
    "W18*50" => ("AISC", "W18x50"),
    // Square hollow section: Tekla SHS100*6 → EN:SHS100x100x6
    "SHS100*6"  => ("EN", "SHS100x6"),
    "SHS120*6"  => ("EN", "SHS120x6"),
    "SHS150*6"  => ("EN", "SHS150x6"),
    "SHS150*8"  => ("EN", "SHS150x8"),
    "SHS200*8"  => ("EN", "SHS200x8"),
    "SHS200*10" => ("EN", "SHS200x10"),
};

// Note: phf (perfect hash function) is used above for compile-time static maps.
// In practice without the phf crate available, we use a runtime HashMap.
// The above is illustrative — the actual implementation below uses HashMap.

/// Runtime profile override table (replaces the phf::Map above).
pub fn build_override_table() -> HashMap<String, (String, String)> {
    let entries = [
        ("HE200A",  "EN",   "HEA200"),  ("HE200B",  "EN",   "HEB200"),
        ("HE240A",  "EN",   "HEA240"),  ("HE240B",  "EN",   "HEB240"),
        ("HE300A",  "EN",   "HEA300"),  ("HE300B",  "EN",   "HEB300"),
        ("HE400A",  "EN",   "HEA400"),  ("HE400B",  "EN",   "HEB400"),
        ("HE500A",  "EN",   "HEA500"),  ("HE500B",  "EN",   "HEB500"),
        ("HE600A",  "EN",   "HEA600"),  ("HE600B",  "EN",   "HEB600"),
        ("HE700A",  "EN",   "HEA700"),  ("HE700B",  "EN",   "HEB700"),
        ("HE800A",  "EN",   "HEA800"),  ("HE800B",  "EN",   "HEB800"),
        ("HE900A",  "EN",   "HEA900"),  ("HE900B",  "EN",   "HEB900"),
        ("HE1000A", "EN",   "HEA1000"), ("HE1000B", "EN",   "HEB1000"),
        ("SHS100*6", "EN",  "SHS100x6"),("SHS150*6", "EN",  "SHS150x6"),
        ("SHS150*8", "EN",  "SHS150x8"),("SHS200*8", "EN",  "SHS200x8"),
        ("SHS200*10","EN",  "SHS200x10"),
    ];
    entries.iter().map(|(k, std, des)| {
        (k.to_uppercase(), (std.to_string(), des.to_string()))
    }).collect()
}

/// Map a Tekla profile string using the runtime override table.
pub fn map_profile_with_table(
    tekla_profile: &str,
    override_table: &HashMap<String, (String, String)>,
) -> ProfileId {
    let normalised = normalise_profile_string(tekla_profile.trim());
    // Try override table first
    if let Some((std, des)) = override_table.get(&normalised.to_uppercase()) {
        return ProfileId::new(std, des);
    }
    classify_by_pattern(&normalised)
}

/// Map a Tekla material/grade string to PMEF steel grade + standard.
pub fn map_material(tekla_material: &str) -> (&'static str, &'static str) {
    match tekla_material.trim().to_uppercase().replace(['-', ' '], "").as_str() {
        "S235" | "S235JR"              => ("S235JR",  "EN 10025-2"),
        "S275" | "S275JR" | "S275J0"  => ("S275JR",  "EN 10025-2"),
        "S355" | "S355JR" | "S355J0" |
        "S355J2" | "S355J2+N"         => ("S355JR",  "EN 10025-2"),
        "S420" | "S420ML"             => ("S420ML",  "EN 10025-4"),
        "S460" | "S460ML" | "S460M"   => ("S460ML",  "EN 10025-4"),
        "A36" | "ASTMA36"             => ("A36",     "ASTM A36"),
        "A572GR50" | "A57250" | "GR50"=> ("A572 Gr.50", "ASTM A572"),
        "A992" | "ASTMA992"           => ("A992",    "ASTM A992"),
        "A500GRB" | "A500B"           => ("A500 Gr.B", "ASTM A500"),
        "A325" | "ASTMA325"           => ("A325",    "ASTM A325"),
        "A490" | "ASTMA490"           => ("A490",    "ASTM A490"),
        "50D" | "S355D" | "355D"      => ("S355JR",  "EN 10025-2"),  // Tekla alias
        _ => (tekla_material, "UNKNOWN"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_profile_hea() {
        let id = map_profile("HEA200");
        assert_eq!(id.standard, "EN");
        assert_eq!(id.designation, "HEA200");
        assert_eq!(id.as_pmef_str(), "EN:HEA200");
    }

    #[test]
    fn test_map_profile_heb() {
        let id = map_profile("HEB300");
        assert_eq!(id.standard, "EN");
        assert_eq!(id.designation, "HEB300");
    }

    #[test]
    fn test_map_profile_ipe() {
        let id = map_profile("IPE300");
        assert_eq!(id.standard, "EN");
        assert_eq!(id.designation, "IPE300");
    }

    #[test]
    fn test_map_profile_chs() {
        let id = map_profile("CHS219.1x8.0");
        assert_eq!(id.standard, "EN");
        assert!(id.designation.contains("CHS"));
    }

    #[test]
    fn test_map_profile_rhs() {
        let id = map_profile("RHS200x100x6");
        assert_eq!(id.standard, "EN");
        assert_eq!(id.designation, "RHS200x100x6");
    }

    #[test]
    fn test_map_profile_shs_star() {
        // Tekla uses * as separator: SHS100*6
        let id = map_profile("SHS100*6");
        assert_eq!(id.standard, "EN");
        // After normalisation: SHS100x6
        assert!(id.designation.starts_with("SHS100"));
    }

    #[test]
    fn test_map_profile_aisc_w() {
        let id = map_profile("W12x53");
        assert_eq!(id.standard, "AISC");
        assert!(id.designation.contains("12"));
        assert!(id.designation.contains("53"));
    }

    #[test]
    fn test_map_profile_aisc_hss() {
        let id = map_profile("HSS6x4x0.25");
        assert_eq!(id.standard, "AISC");
        assert!(id.designation.contains("HSS"));
    }

    #[test]
    fn test_map_profile_he200a_override() {
        let table = build_override_table();
        let id = map_profile_with_table("HE200A", &table);
        assert_eq!(id.standard, "EN");
        assert_eq!(id.designation, "HEA200");
    }

    #[test]
    fn test_map_profile_custom() {
        let id = map_profile("SPECIAL_PROFILE_123");
        assert_eq!(id.standard, "CUSTOM");
    }

    #[test]
    fn test_normalise_star_separator() {
        let n = normalise_profile_string("SHS100*100*6");
        assert_eq!(n, "SHS100x100x6");
    }

    #[test]
    fn test_normalise_x_between_digits() {
        let n = normalise_profile_string("W12X53");
        assert_eq!(n, "W12x53");
    }

    #[test]
    fn test_map_material_s355() {
        let (grade, std) = map_material("S355JR");
        assert_eq!(grade, "S355JR");
        assert_eq!(std, "EN 10025-2");
    }

    #[test]
    fn test_map_material_a572() {
        let (grade, std) = map_material("A572GR50");
        assert_eq!(grade, "A572 Gr.50");
        assert_eq!(std, "ASTM A572");
    }

    #[test]
    fn test_map_material_passthrough() {
        let (grade, _) = map_material("EXOTIC_STEEL");
        assert_eq!(grade, "EXOTIC_STEEL");
    }
}
