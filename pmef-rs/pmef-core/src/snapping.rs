//! Deterministic catalog snapping `pmef-snap/1` (RFC-0002, spec 02 §2.9.4).
//!
//! Chooses the catalog entry that a measured / recognised object corresponds to and records
//! the result as a [`CatalogSnap`]. Every conforming implementation produces the same entry,
//! score, runner-up and ambiguity flag for the same input and catalog version; the shared test
//! vectors are `tests/snapping/vectors.json`.

use crate::provenance::{CatalogSnap, CatalogSnapRef, Residual, SnapCandidate};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// Algorithm identifier written to [`CatalogSnap::algorithm`].
pub const ALGORITHM: &str = "pmef-snap/1";
/// A runner-up whose score is closer than this marks the snap as ambiguous.
pub const AMBIGUITY_MARGIN: f64 = 0.1;

/// What is being snapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    /// Straight pipe against a `PIPING_CLASS` catalog (`componentClass = PIPE`).
    Pipe,
    /// I/H-section against a `STEEL_PROFILES` catalog (families HEA, HEB, HEM, IPE, W).
    ISection,
}

impl SnapKind {
    /// Parse the kind name used in the test vectors (`pipe`, `i-section`).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pipe" => Some(Self::Pipe),
            "i-section" => Some(Self::ISection),
            _ => None,
        }
    }

    /// Canonical parameters in fixed order; the first one is the primary parameter.
    fn params(self) -> &'static [&'static str] {
        match self {
            Self::Pipe => &["outsideDiameter", "wallThickness"],
            Self::ISection => &["h", "b", "tw", "tf"],
        }
    }

    fn required(self) -> &'static [&'static str] {
        match self {
            Self::Pipe => &["outsideDiameter"],
            Self::ISection => &["h", "b"],
        }
    }
}

const I_SECTION_FAMILIES: [&str; 5] = ["HEA", "HEB", "HEM", "IPE", "W"];

/// Catalog field names per canonical parameter (first present wins).
fn fields(param: &str) -> &'static [&'static str] {
    match param {
        "outsideDiameter" => &["outsideDiameter_mm"],
        "wallThickness" => &["wallThickness_mm"],
        "h" => &["h_mm", "d_mm"],
        "b" => &["b_mm", "bf_mm"],
        "tw" => &["tw_mm"],
        "tf" => &["tf_mm"],
        _ => &[],
    }
}

/// Default tolerance band `max(absolute, relative × catalog value)` [mm].
fn default_tolerance(param: &str, catalog_value: f64) -> f64 {
    let (abs, rel) = match param {
        "outsideDiameter" => (1.0, 0.01),
        "wallThickness" => (0.5, 0.15),
        "h" | "b" => (2.0, 0.02),
        _ => (1.0, 0.15), // tw, tf
    };
    f64::max(abs, rel * catalog_value.abs())
}

/// Round to 6 decimals, half away from zero.
pub fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// Snapping request.
#[derive(Debug, Clone)]
pub struct SnapRequest<'a> {
    /// What is snapped.
    pub kind: SnapKind,
    /// Measured values [mm] by canonical parameter name.
    pub measured: &'a BTreeMap<String, f64>,
    /// Optional family restriction for I-sections (e.g. `HEB`).
    pub family: Option<&'a str>,
    /// Non-default tolerances [mm]; recorded in the result.
    pub tolerances: Option<&'a BTreeMap<String, f64>>,
}

/// Result of a snapping run.
#[derive(Debug, Clone, PartialEq)]
pub enum SnapOutcome {
    /// An admissible entry was found.
    Snapped(CatalogSnap),
    /// No admissible entry; the nearest candidate (if any) is reported for review.
    NoSnap {
        /// Nearest candidate regardless of admissibility.
        nearest: Option<SnapCandidate>,
    },
}

/// Invalid snapping input.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SnapError {
    /// Parameter not defined for the kind.
    #[error("unknown parameter '{0}'")]
    UnknownParameter(String),
    /// Required parameter not measured.
    #[error("required parameter '{0}' missing")]
    MissingParameter(String),
    /// Catalog lacks `catalogId` or `version`.
    #[error("catalog has no catalogId/version")]
    UnversionedCatalog,
}

struct Ranked<'c> {
    score: f64,
    primary_abs: f64,
    entry_id: &'c str,
    entry: &'c Value,
    values: Vec<f64>,
}

fn order(a: &Ranked, b: &Ranked) -> Ordering {
    a.score
        .total_cmp(&b.score)
        .then(a.primary_abs.total_cmp(&b.primary_abs))
        .then(a.entry_id.cmp(b.entry_id))
}

fn designation(catalog: &Value, entry: &Value, kind: SnapKind) -> String {
    match kind {
        SnapKind::Pipe => {
            let dn = entry["nominalDiameter_mm"].as_f64().unwrap_or_default();
            let rating = catalog["rating"].as_str().unwrap_or("");
            format!("DN {dn} {rating}").trim().to_owned()
        }
        SnapKind::ISection => {
            if let Some(d) = entry["designation"].as_str() {
                return d.to_owned();
            }
            let id = entry["catalogId"].as_str().unwrap_or("");
            let fam = entry["family"].as_str().unwrap_or("");
            let local = id.split_once(':').map_or(id, |(_, l)| l);
            format!("{fam} {}", local.strip_prefix(fam).unwrap_or(local))
        }
    }
}

/// Run `pmef-snap/1` against a catalog document (as stored in `catalogs/`).
pub fn snap(catalog: &Value, req: &SnapRequest<'_>) -> Result<SnapOutcome, SnapError> {
    let params = req.kind.params();
    if let Some(p) = req.measured.keys().find(|p| !params.contains(&p.as_str())) {
        return Err(SnapError::UnknownParameter(p.clone()));
    }
    if let Some(p) = req.kind.required().iter().find(|p| !req.measured.contains_key(**p)) {
        return Err(SnapError::MissingParameter((*p).to_owned()));
    }
    let (Some(catalog_id), Some(version)) = (catalog["catalogId"].as_str(), catalog["version"].as_str()) else {
        return Err(SnapError::UnversionedCatalog);
    };
    let used: Vec<&str> = params.iter().copied().filter(|p| req.measured.contains_key(*p)).collect();
    let tol = |p: &str, c: f64| req.tolerances.and_then(|t| t.get(p).copied()).unwrap_or_else(|| default_tolerance(p, c));

    let ctype = catalog["catalogType"].as_str().unwrap_or("");
    let mut admissible: Vec<Ranked> = Vec::new();
    let mut nearest: Option<Ranked> = None;
    for entry in catalog["entries"].as_array().into_iter().flatten() {
        let fits_kind = match req.kind {
            SnapKind::Pipe => ctype == "PIPING_CLASS" && entry["componentClass"] == "PIPE",
            SnapKind::ISection => {
                let fam = entry["family"].as_str().unwrap_or("");
                ctype == "STEEL_PROFILES"
                    && I_SECTION_FAMILIES.contains(&fam)
                    && (req.family.is_none() || req.family == Some(fam))
            }
        };
        if !fits_kind {
            continue;
        }
        let value_of = |p: &str| fields(p).iter().find_map(|f| entry[*f].as_f64());
        // required catalog parameters present, and every measured parameter available
        if req.kind.required().iter().any(|p| value_of(p).is_none()) {
            continue;
        }
        let Some(values) = used.iter().map(|p| value_of(p)).collect::<Option<Vec<f64>>>() else {
            continue;
        };
        let norm: Vec<f64> = used
            .iter()
            .zip(&values)
            .map(|(p, c)| (req.measured[*p] - c) / tol(p, *c))
            .collect();
        let score = round6((norm.iter().map(|n| n * n).sum::<f64>() / norm.len() as f64).sqrt());
        let ranked = Ranked {
            score,
            primary_abs: (req.measured[used[0]] - values[0]).abs(),
            entry_id: entry["catalogId"].as_str().unwrap_or(""),
            entry,
            values,
        };
        let is_admissible = norm.iter().all(|n| n.abs() <= 1.0);
        let closer = match &nearest {
            None => true,
            Some(n) => order(&ranked, n) == Ordering::Less,
        };
        if closer {
            nearest = Some(Ranked { values: ranked.values.clone(), ..ranked });
        }
        if is_admissible {
            admissible.push(ranked);
        }
    }
    admissible.sort_by(order);

    let Some(best) = admissible.first() else {
        return Ok(SnapOutcome::NoSnap {
            nearest: nearest.map(|n| SnapCandidate { entry_id: n.entry_id.to_owned(), score: n.score }),
        });
    };
    let runner_up = admissible.get(1).map(|r| SnapCandidate { entry_id: r.entry_id.to_owned(), score: r.score });
    let ambiguous = runner_up.as_ref().is_some_and(|r| round6(r.score - best.score) < AMBIGUITY_MARGIN);
    let tolerances = req.tolerances.map(|t| {
        used.iter().filter_map(|p| t.get(*p).map(|v| ((*p).to_owned(), *v))).collect::<BTreeMap<_, _>>()
    });
    Ok(SnapOutcome::Snapped(CatalogSnap {
        catalog_ref: CatalogSnapRef {
            catalog_id: catalog_id.to_owned(),
            catalog_version: version.to_owned(),
            entry_id: best.entry_id.to_owned(),
        },
        snapped_size: designation(catalog, best.entry, req.kind),
        residuals: used
            .iter()
            .zip(&best.values)
            .map(|(p, c)| Residual {
                parameter: (*p).to_owned(),
                measured_mm: req.measured[*p],
                catalog_mm: *c,
                residual_mm: round6(req.measured[*p] - c),
            })
            .collect(),
        algorithm: Some(ALGORITHM.to_owned()),
        score: Some(best.score),
        runner_up,
        ambiguous: Some(ambiguous),
        tolerances: tolerances.filter(|t| !t.is_empty()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog() -> Value {
        json!({"catalogId": "c", "catalogType": "PIPING_CLASS", "version": "1", "rating": "PN16", "entries": [
            {"catalogId": "P-100", "componentClass": "PIPE", "nominalDiameter_mm": 100, "outsideDiameter_mm": 114.3, "wallThickness_mm": 3.6},
            {"catalogId": "P-150", "componentClass": "PIPE", "nominalDiameter_mm": 150, "outsideDiameter_mm": 168.3, "wallThickness_mm": 4.5},
            {"catalogId": "E-150", "componentClass": "ELBOW", "outsideDiameter_mm": 168.3}
        ]})
    }

    fn m(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    #[test]
    fn snaps_to_nearest_admissible_pipe() {
        let measured = m(&[("outsideDiameter", 168.0)]);
        let req = SnapRequest { kind: SnapKind::Pipe, measured: &measured, family: None, tolerances: None };
        let SnapOutcome::Snapped(s) = snap(&catalog(), &req).unwrap() else { panic!() };
        assert_eq!(s.catalog_ref.entry_id, "P-150");
        assert_eq!(s.snapped_size, "DN 150 PN16");
        assert_eq!(s.ambiguous, Some(false));
    }

    #[test]
    fn rejects_bad_input() {
        let measured = m(&[("wallThickness", 4.5)]);
        let req = SnapRequest { kind: SnapKind::Pipe, measured: &measured, family: None, tolerances: None };
        assert_eq!(snap(&catalog(), &req), Err(SnapError::MissingParameter("outsideDiameter".into())));
    }

    #[test]
    fn rounding_is_half_away_from_zero() {
        assert_eq!(round6(0.0000005), 0.000001);
        assert_eq!(round6(-0.0000005), -0.000001);
    }
}
