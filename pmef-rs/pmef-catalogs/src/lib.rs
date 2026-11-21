//! PMEF engineering catalogs, embedded for consumers.
//!
//! PMEF is the single home of engineering catalogs in the ecosystem (`docs/ECOSYSTEM.md`):
//! piping classes, flange and pipe dimensions, materials and steel profiles. Other repositories
//! (e.g. DEXPI-Manager `pipe-class-engine`) read them through this crate instead of keeping
//! their own copies of the tables. The JSON files in `catalogs/` stay the source of truth; this
//! crate only embeds them at compile time.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Every catalog in `catalogs/`, by file stem (= `catalogId`).
const CATALOGS: &[(&str, &str)] = &[
    (
        "caesarII-cii-mapping",
        include_str!("../../../catalogs/caesarII-cii-mapping.json"),
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
        "pipe-dimensions",
        include_str!("../../../catalogs/pipe-dimensions.json"),
    ),
    (
        "pipe-dimensions-en",
        include_str!("../../../catalogs/pipe-dimensions-en.json"),
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
