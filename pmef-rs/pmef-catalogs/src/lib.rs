//! PMEF engineering catalogs, embedded for consumers.
//!
//! PMEF is the single home of engineering catalogs in the ecosystem (`docs/ECOSYSTEM.md`):
//! piping classes, flange, pipe and duct dimensions, materials, steel profiles, pipe support
//! types, cable types and native element type mappings for adapters. Other repositories
//! (e.g. DEXPI-Manager `pipe-class-engine`) read them through this crate instead of keeping
//! their own copies of the tables. The JSON files in `catalogs/` stay the source of truth; this
//! crate only embeds them at compile time.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Every catalog in `catalogs/`, by file stem (= `catalogId`).
const CATALOGS: &[(&str, &str)] = &[
    (
        "cable-types-en",
        include_str!("../../../catalogs/cable-types-en.json"),
    ),
    (
        "caesarII-cii-mapping",
        include_str!("../../../catalogs/caesarII-cii-mapping.json"),
    ),
    (
        "duct-dimensions-en",
        include_str!("../../../catalogs/duct-dimensions-en.json"),
    ),
    (
        "flange-dimensions",
        include_str!("../../../catalogs/flange-dimensions.json"),
    ),
    (
        "flange-dimensions-en",
        include_str!("../../../catalogs/flange-dimensions-en.json"),
    ),
    (
        "instruments-common",
        include_str!("../../../catalogs/instruments-common.json"),
    ),
    (
        "materials-astm",
        include_str!("../../../catalogs/materials-astm.json"),
    ),
    (
        "materials-en",
        include_str!("../../../catalogs/materials-en.json"),
    ),
    (
        "native-types-e3d",
        include_str!("../../../catalogs/native-types-e3d.json"),
    ),
    (
        "pipe-dimensions",
        include_str!("../../../catalogs/pipe-dimensions.json"),
    ),
    (
        "pipe-dimensions-en",
        include_str!("../../../catalogs/pipe-dimensions-en.json"),
    ),
    (
        "pipe-supports-generic",
        include_str!("../../../catalogs/pipe-supports-generic.json"),
    ),
    (
        "piping-class-a1a2",
        include_str!("../../../catalogs/piping-class-a1a2.json"),
    ),
    (
        "piping-class-b3c1",
        include_str!("../../../catalogs/piping-class-b3c1.json"),
    ),
    (
        "piping-class-c5d1",
        include_str!("../../../catalogs/piping-class-c5d1.json"),
    ),
    (
        "piping-class-en-p1a",
        include_str!("../../../catalogs/piping-class-en-p1a.json"),
    ),
    (
        "profiles-aisc",
        include_str!("../../../catalogs/profiles-aisc.json"),
    ),
    (
        "profiles-aisc-extended",
        include_str!("../../../catalogs/profiles-aisc-extended.json"),
    ),
    (
        "profiles-en",
        include_str!("../../../catalogs/profiles-en.json"),
    ),
    (
        "profiles-en-extended",
        include_str!("../../../catalogs/profiles-en-extended.json"),
    ),
    (
        "rdl-uri-map",
        include_str!("../../../catalogs/rdl-uri-map.json"),
    ),
];

/// IDs of all embedded catalogs.
pub fn ids() -> impl Iterator<Item = &'static str> {
    CATALOGS.iter().map(|(id, _)| *id)
}

/// Raw JSON text of a catalog.
pub fn raw(id: &str) -> Option<&'static str> {
    CATALOGS.iter().find(|(i, _)| *i == id).map(|(_, s)| *s)
}

/// Parsed catalog document (as stored in `catalogs/`, e.g. for `pmef_core::snapping::snap`).
pub fn document(id: &str) -> Option<Value> {
    raw(id).map(|s| serde_json::from_str(s).expect("embedded catalogs are valid JSON"))
}

/// One nominal pipe size from the ASME B36.10M/B36.19M pipe-dimensions catalog.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PipeSize {
    /// Nominal diameter (DN, mm-based designation).
    #[serde(rename = "DN")]
    pub dn: u32,
    /// Nominal pipe size in inches.
    #[serde(rename = "NPS")]
    pub nps: f64,
    /// Outside diameter in mm.
    #[serde(rename = "OD_mm")]
    pub od_mm: f64,
}

/// All nominal pipe sizes, ascending.
pub fn pipe_sizes() -> Vec<PipeSize> {
    #[derive(Deserialize)]
    struct Doc {
        entries: Vec<PipeSize>,
    }
    let doc: Doc = serde_json::from_str(raw("pipe-dimensions").expect("pipe-dimensions catalog"))
        .expect("pipe-dimensions entries");
    doc.entries
}

/// DN for a nominal pipe size in inches (e.g. `1.25` -> `32`).
pub fn nps_to_dn(nps: f64) -> Option<u32> {
    pipe_sizes()
        .into_iter()
        .find(|s| (s.nps - nps).abs() < 1e-6)
        .map(|s| s.dn)
}

/// NPS in inches for a DN (e.g. `50` -> `2.0`).
pub fn dn_to_nps(dn: u32) -> Option<f64> {
    pipe_sizes().into_iter().find(|s| s.dn == dn).map(|s| s.nps)
}

/// Parse an NPS designation such as `"2"`, `"1/2"`, `"1-1/4"` or `"2.5"` to inches.
pub fn parse_nps(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('"').trim();
    let frac = |f: &str| -> Option<f64> {
        let (n, d) = f.split_once('/')?;
        let (n, d): (f64, f64) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
        (d != 0.0).then(|| n / d)
    };
    if let Some((whole, f)) = s.split_once(['-', ' ']) {
        return Some(whole.trim().parse::<f64>().ok()? + frac(f)?);
    }
    if s.contains('/') {
        return frac(s);
    }
    s.parse().ok()
}

fn entries<T: serde::de::DeserializeOwned>(id: &str) -> Vec<T> {
    #[derive(Deserialize)]
    struct Doc<T> {
        entries: Vec<T>,
    }
    let doc: Doc<T> = serde_json::from_str(raw(id).unwrap_or_else(|| panic!("{id} catalog")))
        .unwrap_or_else(|e| panic!("{id} entries: {e}"));
    doc.entries
}

/// Standard duct size series (RFC-0006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DuctSeries {
    /// Round nominal diameter (EN 1506).
    Round,
    /// Preferred rectangular side length (EN 1505).
    Rectangular,
}

/// One standard air-duct size from `duct-dimensions-en`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuctSize {
    /// Entry id, e.g. `R400`.
    #[serde(rename = "catalogId")]
    pub id: String,
    /// Round diameter or rectangular side length.
    pub series: DuctSeries,
    /// Diameter or side length [mm].
    pub size_mm: f64,
    /// Inside area of a round duct [m²].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area_m2: Option<f64>,
}

/// All standard duct sizes of a series, ascending.
pub fn duct_sizes(series: DuctSeries) -> Vec<DuctSize> {
    let mut v: Vec<DuctSize> = entries::<DuctSize>("duct-dimensions-en")
        .into_iter()
        .filter(|e| e.series == series)
        .collect();
    v.sort_by(|a, b| a.size_mm.total_cmp(&b.size_mm));
    v
}

/// Smallest standard size of a series that is at least `min_mm` (sizing a duct to a
/// required dimension).
pub fn next_duct_size(series: DuctSeries, min_mm: f64) -> Option<f64> {
    duct_sizes(series).into_iter().map(|e| e.size_mm).find(|&s| s >= min_mm)
}

/// One pipe support assembly from `pipe-supports-generic`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipeSupportType {
    /// Catalog code, e.g. `GS-SHOE` (referenced as `catalogRef.standard`).
    pub support_code: String,
    /// `SupportSpec.supportType` value.
    pub support_type: String,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Smallest DN.
    pub dn_min: u32,
    /// Largest DN.
    pub dn_max: u32,
    /// Restrained degrees of freedom.
    pub restraints: Vec<String>,
    /// Raises the pipe above its insulation.
    #[serde(default)]
    pub insulated_pipe: bool,
    /// Shoe height [mm].
    #[serde(default, rename = "shoeHeight_mm", skip_serializing_if = "Option::is_none")]
    pub shoe_height_mm: Option<f64>,
    /// Spring travel [mm].
    #[serde(default, rename = "travel_mm", skip_serializing_if = "Option::is_none")]
    pub travel_mm: Option<f64>,
}

/// All pipe support types.
pub fn pipe_support_types() -> Vec<PipeSupportType> {
    entries("pipe-supports-generic")
}

/// Support types that fit a pipe of nominal diameter `dn` and restrain every listed
/// degree of freedom (e.g. `["VERTICAL_DOWN", "LATERAL"]` for a guide).
pub fn supports_for(dn: u32, restraints: &[&str]) -> Vec<PipeSupportType> {
    pipe_support_types()
        .into_iter()
        .filter(|s| s.dn_min <= dn && dn <= s.dn_max)
        .filter(|s| restraints.iter().all(|r| s.restraints.iter().any(|x| x == r)))
        .collect()
}

/// One cable type from `cable-types-en`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CableType {
    /// Designation, e.g. `NYY-J 5x2.5` (referenced as `catalogRef.standard`).
    pub designation: String,
    /// `CableObject.cableType` value.
    pub cable_type: String,
    /// Number of cores.
    pub number_of_cores: u32,
    /// Conductor cross-section [mm²].
    #[serde(rename = "crossSection_mm2")]
    pub cross_section_mm2: f64,
    /// Typical outer diameter [mm].
    #[serde(default, rename = "outerDiameter_mm", skip_serializing_if = "Option::is_none")]
    pub outer_diameter_mm: Option<f64>,
    /// Minimum bend radius as a multiple of the outer diameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_bend_radius_factor: Option<f64>,
    /// Rated voltage, e.g. `0.6/1 kV`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rated_voltage: Option<String>,
}

impl CableType {
    /// Minimum bend radius [mm] (outer diameter × factor).
    pub fn min_bend_radius_mm(&self) -> Option<f64> {
        Some(self.outer_diameter_mm? * self.min_bend_radius_factor?)
    }
}

/// All cable types.
pub fn cable_types() -> Vec<CableType> {
    entries("cable-types-en")
}

/// A cable type by designation.
pub fn cable_type(designation: &str) -> Option<CableType> {
    cable_types().into_iter().find(|c| c.designation == designation)
}
/// One entry of a `NATIVE_TYPE_MAPPING` catalog (e.g. `native-types-e3d`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTypeMapping {
    /// Entry identifier.
    pub catalog_id: String,
    /// Element type keyword in the source tool, e.g. `VALV`.
    pub native_type: String,
    /// Condition: SKEY starts with this text (case-insensitive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skey_prefix: Option<String>,
    /// Condition: SKEY contains this text (case-insensitive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skey_contains: Option<String>,
    /// PMEF `@type`.
    pub pmef_type: String,
    /// `componentSpec.componentClass`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_class: Option<String>,
    /// Free-text note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl NativeTypeMapping {
    /// True if this entry maps element type `keyword` with this SKEY.
    pub fn applies(&self, keyword: &str, skey: Option<&str>) -> bool {
        let skey = skey.unwrap_or("").trim().to_uppercase();
        self.native_type.eq_ignore_ascii_case(keyword)
            && self.skey_prefix.as_ref().is_none_or(|p| skey.starts_with(&p.to_uppercase()))
            && self.skey_contains.as_ref().is_none_or(|c| skey.contains(&c.to_uppercase()))
    }
}

/// All entries of a native type mapping catalog, in file order.
pub fn native_types(catalog: &str) -> Vec<NativeTypeMapping> {
    entries(catalog)
}

/// The first entry of `catalog` that maps element type `keyword` with this SKEY.
pub fn native_type(catalog: &str, keyword: &str, skey: Option<&str>) -> Option<NativeTypeMapping> {
    native_types(catalog).into_iter().find(|m| m.applies(keyword, skey))
}

