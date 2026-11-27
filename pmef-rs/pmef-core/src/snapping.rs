//! Deterministic catalog snapping `pmef-snap/1` (RFC-0002) and `pmef-snap/2` (RFC-0011),
//! spec 02 §2.9.4.
//!
//! Chooses the catalog entry that a measured / recognised object corresponds to and records
//! the result as a [`CatalogSnap`]. Every conforming implementation produces the same entry,
//! score, runner-up and ambiguity flag for the same input and catalog version; the shared test
//! vectors are `tests/snapping/vectors.json` (version 1) and `tests/snapping/vectors-v2.json`
//! (version 2). Version 2 is a superset: for [`SnapKind::Pipe`] and [`SnapKind::ISection`] it
//! returns the same entry, score, residuals, runner-up and ambiguity flag as version 1.

use crate::provenance::{CatalogSnap, CatalogSnapRef, Residual, SnapCandidate};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// Algorithm identifier of version 1, written to [`CatalogSnap::algorithm`].
pub const ALGORITHM: &str = "pmef-snap/1";
/// Algorithm identifier of version 2 (RFC-0011).
pub const ALGORITHM_V2: &str = "pmef-snap/2";
/// A runner-up whose score is closer than this marks the snap as ambiguous (both versions).
pub const AMBIGUITY_MARGIN: f64 = 0.1;
/// An elbow matches the `angle` selector if its catalog angle is within this many degrees.
pub const ANGLE_SELECTOR_TOL_DEG: f64 = 2.0;

/// Snapping algorithm version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// `pmef-snap/1` (RFC-0002): pipes and I-sections.
    V1,
    /// `pmef-snap/2` (RFC-0011): adds channels, hollow sections, angles, elbows, tees,
    /// reducers and flanges.
    V2,
}

impl Algorithm {
    /// Identifier as written to [`CatalogSnap::algorithm`].
    pub fn id(self) -> &'static str {
        match self {
            Self::V1 => ALGORITHM,
            Self::V2 => ALGORITHM_V2,
        }
    }

    /// Parse an algorithm identifier.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            ALGORITHM => Some(Self::V1),
            ALGORITHM_V2 => Some(Self::V2),
            _ => None,
        }
    }
}

/// What is being snapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    /// Straight pipe against a `PIPING_CLASS` catalog (`componentClass = PIPE`).
    Pipe,
    /// I/H-section against a `STEEL_PROFILES` catalog (families HEA, HEB, HEM, IPE, W).
    ISection,
    /// Channel (families UPE, UPN, C, MC; millimetre fields only). Version 2.
    Channel,
    /// Rectangular or square hollow section (RHS, SHS, HSS_RECT), orientation-free. Version 2.
    RectHollow,
    /// Circular hollow section (CHS, HSS_ROUND). Version 2.
    RoundHollow,
    /// Equal or unequal angle (EQUAL_ANGLE, UNEQUAL_ANGLE, L), orientation-free. Version 2.
    Angle,
    /// Elbow of a piping class. Version 2.
    Elbow,
    /// Tee of a piping class. Version 2.
    Tee,
    /// Concentric or eccentric reducer of a piping class. Version 2.
    Reducer,
    /// Flange from a `FLANGE_DIMENSIONS` table or a piping class (FLANGE, BLIND_FLANGE). Version 2.
    Flange,
}

type Tol = (f64, f64);
const OD: Tol = (1.0, 0.01);
const LEN: Tol = (2.0, 0.02);

impl SnapKind {
    /// Parse the kind name used in the test vectors (`pipe`, `i-section`, `channel`,
    /// `rect-hollow`, `round-hollow`, `angle`, `elbow`, `tee`, `reducer`, `flange`).
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pipe" => Self::Pipe,
            "i-section" => Self::ISection,
            "channel" => Self::Channel,
            "rect-hollow" => Self::RectHollow,
            "round-hollow" => Self::RoundHollow,
            "angle" => Self::Angle,
            "elbow" => Self::Elbow,
            "tee" => Self::Tee,
            "reducer" => Self::Reducer,
            "flange" => Self::Flange,
            _ => return None,
        })
    }

    /// Kind name as used in the test vectors.
    pub fn name(self) -> &'static str {
        match self {
            Self::Pipe => "pipe",
            Self::ISection => "i-section",
            Self::Channel => "channel",
            Self::RectHollow => "rect-hollow",
            Self::RoundHollow => "round-hollow",
            Self::Angle => "angle",
            Self::Elbow => "elbow",
            Self::Tee => "tee",
            Self::Reducer => "reducer",
            Self::Flange => "flange",
        }
    }

    /// Whether the kind is defined in the given algorithm version.
    pub fn supported_by(self, algorithm: Algorithm) -> bool {
        matches!(self, Self::Pipe | Self::ISection) || algorithm == Algorithm::V2
    }

    /// Canonical parameters in fixed order; the first one is the primary parameter.
    pub fn params(self) -> &'static [&'static str] {
        match self {
            Self::Pipe | Self::RoundHollow => &["outsideDiameter", "wallThickness"],
            Self::ISection | Self::Channel => &["h", "b", "tw", "tf"],
            Self::RectHollow => &["d1", "d2", "t"],
            Self::Angle => &["leg1", "leg2", "t"],
            Self::Elbow => &["outsideDiameter", "centreToFace"],
            Self::Tee => &["outsideDiameter", "branchOutsideDiameter", "centreToEndRun", "centreToEndBranch"],
            Self::Reducer => &["outsideDiameter", "reducedOutsideDiameter", "length"],
            Self::Flange => &["flangeOD", "flangeThickness", "boltCircle", "boltHoleDiameter"],
        }
    }

    fn required(self) -> &'static [&'static str] {
        match self {
            Self::Pipe | Self::RoundHollow | Self::Elbow => &["outsideDiameter"],
            Self::ISection | Self::Channel => &["h", "b"],
            Self::RectHollow => &["d1", "d2"],
            Self::Angle => &["leg1", "leg2"],
            Self::Tee => &["outsideDiameter", "branchOutsideDiameter"],
            Self::Reducer => &["outsideDiameter", "reducedOutsideDiameter"],
            Self::Flange => &["flangeOD"],
        }
    }

    /// Selectors (exact filters, not scored): `angle` [°] for elbows, `boltCount` for flanges.
    pub fn selectors(self) -> &'static [&'static str] {
        match self {
            Self::Elbow => &["angle"],
            Self::Flange => &["boltCount"],
            _ => &[],
        }
    }

    /// Parameter pairs whose measured values must satisfy first ≥ second.
    fn ordered(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::RectHollow => Some(("d1", "d2")),
            Self::Angle => Some(("leg1", "leg2")),
            _ => None,
        }
    }

    /// Default tolerance band `max(absolute, relative × catalog value)` [mm].
    fn tolerance(self, param: &str) -> Tol {
        match (self, param) {
            (_, "outsideDiameter" | "branchOutsideDiameter" | "reducedOutsideDiameter") => OD,
            (_, "wallThickness") => (0.5, 0.15),
            (_, "h" | "b" | "d1" | "d2" | "leg1" | "leg2" | "centreToFace" | "centreToEndRun")
            | (_, "centreToEndBranch" | "length") => LEN,
            (Self::RectHollow, "t") => (0.5, 0.15),
            (_, "flangeOD") => (2.0, 0.01),
            (_, "flangeThickness") => (1.0, 0.1),
            (_, "boltCircle") => (1.5, 0.01),
            (_, "boltHoleDiameter") => (1.0, 0.05),
            _ => (1.0, 0.15), // tw, tf, angle t
        }
    }
}

const I_SECTION_FAMILIES: [&str; 5] = ["HEA", "HEB", "HEM", "IPE", "W"];

/// First numeric field among `names` (first present wins).
fn num(e: &Value, names: &[&str]) -> Option<f64> {
    names.iter().find_map(|n| e.get(*n).filter(|v| v.is_number()).and_then(Value::as_f64))
}

fn dn(e: &Value) -> Option<f64> {
    num(e, &["nominalDiameter_mm", "DN", "nominalDiameter"])
}

/// OD of the first PIPE entry of the same piping class with this DN (catalog order).
fn class_pipe_od(catalog: &Value, nominal: Option<f64>) -> Option<f64> {
    let nominal = nominal?;
    catalog["entries"]
        .as_array()?
        .iter()
        .filter(|p| p["componentClass"] == "PIPE" && dn(p) == Some(nominal))
        .find_map(|p| num(p, &["outsideDiameter_mm"]))
}

fn sorted_pair(a: Option<f64>, b: Option<f64>, larger: bool) -> Option<f64> {
    let (a, b) = (a?, b?);
    Some(if larger { a.max(b) } else { a.min(b) })
}

fn fitting_od(catalog: &Value, e: &Value) -> Option<f64> {
    num(e, &["outsideDiameter_mm"]).or_else(|| class_pipe_od(catalog, dn(e)))
}

fn section_value(e: &Value, p: &str) -> Option<f64> {
    match p {
        "h" => num(e, &["h_mm", "d_mm"]),
        "b" => num(e, &["b_mm", "bf_mm"]),
        "tw" => num(e, &["tw_mm"]),
        "tf" => num(e, &["tf_mm"]),
        _ => None,
    }
}

/// Catalog value of a canonical parameter for an entry.
fn value(kind: SnapKind, catalog: &Value, e: &Value, p: &str) -> Option<f64> {
    match kind {
        SnapKind::Pipe => match p {
            "outsideDiameter" => num(e, &["outsideDiameter_mm"]),
            _ => num(e, &["wallThickness_mm"]),
        },
        SnapKind::ISection | SnapKind::Channel => section_value(e, p),
        SnapKind::RectHollow => match p {
            "t" => num(e, &["t_mm"]),
            _ => sorted_pair(num(e, &["h_mm"]), num(e, &["b_mm"]), p == "d1"),
        },
        SnapKind::Angle => match p {
            "t" => num(e, &["t_mm"]),
            _ => sorted_pair(num(e, &["b1_mm", "h_mm"]), num(e, &["b2_mm", "b_mm"]), p == "leg1"),
        },
        SnapKind::RoundHollow => match p {
            "outsideDiameter" => num(e, &["OD_mm", "outsideDiameter_mm"]),
            _ => num(e, &["t_mm", "wallThickness_mm"]),
        },
        SnapKind::Elbow => match p {
            "outsideDiameter" => fitting_od(catalog, e),
            _ => num(e, &["centreToFace_mm"]),
        },
        SnapKind::Tee => match p {
            "outsideDiameter" => fitting_od(catalog, e),
            "branchOutsideDiameter" => class_pipe_od(catalog, num(e, &["branchDiameter_mm"])),
            "centreToEndRun" => num(e, &["centreToEnd_run_mm", "centreToEnd_mm"]),
            _ => num(e, &["centreToEnd_branch_mm", "centreToEnd_mm"]),
        },
        SnapKind::Reducer => match p {
            "outsideDiameter" => fitting_od(catalog, e),
            "reducedOutsideDiameter" => num(e, &["reducedOD_mm"])
                .or_else(|| class_pipe_od(catalog, num(e, &["reducedDiameter_mm", "reducingDiameter_mm"]))),
            _ => num(e, &["length_mm"]),
        },
        SnapKind::Flange => match p {
            "flangeOD" => num(e, &["flangeOD_mm", "outsideDiameter_mm"]),
            "flangeThickness" => num(e, &["flangeThickness_mm"]),
            "boltCircle" => num(e, &["boltCircle_mm", "boltCircleDiameter_mm"]),
            _ => num(e, &["boltHoleDiameter_mm"]),
        },
    }
}

/// Whether an entry is a candidate of the kind (catalog type, family / component class).
fn is_candidate(kind: SnapKind, catalog: &Value, e: &Value, family: Option<&str>) -> bool {
    let ctype = catalog["catalogType"].as_str().unwrap_or("");
    let fam = e["family"].as_str().unwrap_or("");
    let class = e["componentClass"].as_str().unwrap_or("");
    let profile = |families: &[&str]| {
        ctype == "STEEL_PROFILES" && families.contains(&fam) && (family.is_none() || family == Some(fam))
    };
    let in_class = |classes: &[&str]| {
        ctype == "PIPING_CLASS" && classes.contains(&class) && (family.is_none() || family == Some(class))
    };
    match kind {
        SnapKind::Pipe => ctype == "PIPING_CLASS" && class == "PIPE",
        SnapKind::ISection => profile(&I_SECTION_FAMILIES),
        SnapKind::Channel => profile(&["UPE", "UPN", "C", "MC"]),
        SnapKind::RectHollow => profile(&["RHS", "SHS", "HSS_RECT"]),
        SnapKind::RoundHollow => profile(&["CHS", "HSS_ROUND"]),
        SnapKind::Angle => profile(&["EQUAL_ANGLE", "UNEQUAL_ANGLE", "L"]),
        SnapKind::Elbow => in_class(&["ELBOW"]),
        SnapKind::Tee => in_class(&["TEE"]),
        SnapKind::Reducer => in_class(&["REDUCER", "REDUCER_CONCENTRIC", "REDUCER_ECCENTRIC"]),
        SnapKind::Flange => match family {
            Some(_) => in_class(&["FLANGE", "BLIND_FLANGE"]),
            None => ctype == "FLANGE_DIMENSIONS" || in_class(&["FLANGE", "BLIND_FLANGE"]),
        },
    }
}

fn selector_ok(e: &Value, name: &str, wanted: f64) -> bool {
    match name {
        "angle" => num(e, &["angle_deg"]).is_some_and(|c| (c - wanted).abs() <= ANGLE_SELECTOR_TOL_DEG),
        _ => num(e, &["boltHoles", "numberOfBolts"]).is_some_and(|c| c == wanted),
    }
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
    /// Optional restriction: profile `family` (e.g. `HEB`) or, for piping-class kinds,
    /// `componentClass` (e.g. `FLANGE`). Ignored for [`SnapKind::Pipe`].
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
    /// Kind not defined in the requested algorithm version.
    #[error("unsupported kind '{kind}' for {algorithm}")]
    UnsupportedKind {
        /// Kind name.
        kind: &'static str,
        /// Algorithm identifier.
        algorithm: &'static str,
    },
    /// Selector not defined for the kind.
    #[error("unknown selector '{0}'")]
    UnknownSelector(String),
    /// Orientation-free pair given in the wrong order.
    #[error("parameter order: {0} must be >= {1}")]
    ParameterOrder(&'static str, &'static str),
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

fn fmt(x: Option<f64>) -> String {
    x.map(|v| format!("{v}")).unwrap_or_else(|| "None".to_owned())
}

fn designation(catalog: &Value, entry: &Value, kind: SnapKind, algorithm: Algorithm) -> String {
    let rating = entry["rating"].as_str().or(catalog["rating"].as_str()).unwrap_or("");
    let local = || {
        let id = entry["catalogId"].as_str().unwrap_or("");
        id.split_once(':').map_or(id, |(_, l)| l).to_owned()
    };
    let out = match kind {
        SnapKind::Pipe => {
            let nominal = match algorithm {
                Algorithm::V1 => Some(entry["nominalDiameter_mm"].as_f64().unwrap_or_default()),
                Algorithm::V2 => dn(entry),
            };
            format!("DN {} {}", fmt(nominal), catalog["rating"].as_str().unwrap_or(""))
        }
        SnapKind::ISection | SnapKind::Channel => {
            if let Some(d) = entry["designation"].as_str() {
                return d.to_owned();
            }
            let fam = entry["family"].as_str().unwrap_or("");
            let local = local();
            format!("{fam} {}", local.strip_prefix(fam).unwrap_or(&local))
        }
        SnapKind::RectHollow | SnapKind::RoundHollow | SnapKind::Angle => {
            return entry["designation"].as_str().map_or_else(local, str::to_owned);
        }
        SnapKind::Elbow => format!("DN {} {}° {rating}", fmt(dn(entry)), fmt(num(entry, &["angle_deg"]))),
        SnapKind::Tee => format!("DN {}x{} {rating}", fmt(dn(entry)), fmt(num(entry, &["branchDiameter_mm"]))),
        SnapKind::Reducer => format!(
            "DN {}x{} {rating}",
            fmt(dn(entry)),
            fmt(num(entry, &["reducedDiameter_mm", "reducingDiameter_mm"]))
        ),
        SnapKind::Flange => {
            let rating = match &entry["pressureClass"] {
                Value::Number(n) => format!("Class {}", fmt(n.as_f64())),
                Value::String(s) => s.clone(),
                _ => rating.to_owned(),
            };
            format!("DN {} {rating}", fmt(dn(entry)))
        }
    };
    out.trim().to_owned()
}

/// Run `pmef-snap/1` against a catalog document (as stored in `catalogs/`).
pub fn snap(catalog: &Value, req: &SnapRequest<'_>) -> Result<SnapOutcome, SnapError> {
    snap_with(catalog, req, Algorithm::V1, None)
}

/// Run `pmef-snap/2` (RFC-0011) with optional selectors (`angle` [°], `boltCount`).
pub fn snap_v2(
    catalog: &Value,
    req: &SnapRequest<'_>,
    select: Option<&BTreeMap<String, f64>>,
) -> Result<SnapOutcome, SnapError> {
    snap_with(catalog, req, Algorithm::V2, select)
}

/// Run the given algorithm version against a catalog document.
pub fn snap_with(
    catalog: &Value,
    req: &SnapRequest<'_>,
    algorithm: Algorithm,
    select: Option<&BTreeMap<String, f64>>,
) -> Result<SnapOutcome, SnapError> {
    let kind = req.kind;
    if !kind.supported_by(algorithm) {
        return Err(SnapError::UnsupportedKind { kind: kind.name(), algorithm: algorithm.id() });
    }
    let params = kind.params();
    if let Some(p) = req.measured.keys().find(|p| !params.contains(&p.as_str())) {
        return Err(SnapError::UnknownParameter(p.clone()));
    }
    if let Some(p) = kind.required().iter().find(|p| !req.measured.contains_key(**p)) {
        return Err(SnapError::MissingParameter((*p).to_owned()));
    }
    let empty = BTreeMap::new();
    let select = select.unwrap_or(&empty);
    if let Some(s) = select.keys().find(|s| !kind.selectors().contains(&s.as_str())) {
        return Err(SnapError::UnknownSelector(s.clone()));
    }
    if let Some((a, b)) = kind.ordered() {
        if req.measured[a] < req.measured[b] {
            return Err(SnapError::ParameterOrder(a, b));
        }
    }
    let (Some(catalog_id), Some(version)) = (catalog["catalogId"].as_str(), catalog["version"].as_str()) else {
        return Err(SnapError::UnversionedCatalog);
    };
    let used: Vec<&str> = params.iter().copied().filter(|p| req.measured.contains_key(*p)).collect();
    let tol = |p: &str, c: f64| {
        req.tolerances.and_then(|t| t.get(p).copied()).unwrap_or_else(|| {
            let (abs, rel) = kind.tolerance(p);
            f64::max(abs, rel * c.abs())
        })
    };
    // pmef-snap/1 ignores the family restriction for pipes; version 2 keeps that.
    let family = if kind == SnapKind::Pipe { None } else { req.family };

    let mut admissible: Vec<Ranked> = Vec::new();
    let mut nearest: Option<Ranked> = None;
    for entry in catalog["entries"].as_array().into_iter().flatten() {
        if !is_candidate(kind, catalog, entry, family) {
            continue;
        }
        if !select.iter().all(|(s, v)| selector_ok(entry, s, *v)) {
            continue;
        }
        let value_of = |p: &str| value(kind, catalog, entry, p);
        // required catalog parameters present, and every measured parameter available
        if kind.required().iter().any(|p| value_of(p).is_none()) {
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
        snapped_size: designation(catalog, best.entry, kind, algorithm),
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
        algorithm: Some(algorithm.id().to_owned()),
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
    fn v2_kinds_are_rejected_by_v1() {
        let measured = m(&[("outsideDiameter", 168.3)]);
        let req = SnapRequest { kind: SnapKind::Elbow, measured: &measured, family: None, tolerances: None };
        assert_eq!(
            snap(&catalog(), &req),
            Err(SnapError::UnsupportedKind { kind: "elbow", algorithm: ALGORITHM })
        );
    }

    #[test]
    fn v2_elbow_takes_od_from_entry_and_v2_pipe_equals_v1() {
        let measured = m(&[("outsideDiameter", 168.0)]);
        let req = SnapRequest { kind: SnapKind::Elbow, measured: &measured, family: None, tolerances: None };
        let SnapOutcome::Snapped(s) = snap_v2(&catalog(), &req, None).unwrap() else { panic!() };
        assert_eq!(s.catalog_ref.entry_id, "E-150");
        assert_eq!(s.algorithm.as_deref(), Some(ALGORITHM_V2));

        let req = SnapRequest { kind: SnapKind::Pipe, ..req };
        let (SnapOutcome::Snapped(a), SnapOutcome::Snapped(b)) =
            (snap(&catalog(), &req).unwrap(), snap_v2(&catalog(), &req, None).unwrap())
        else {
            panic!()
        };
        assert_eq!((a.catalog_ref, a.score, a.residuals), (b.catalog_ref, b.score, b.residuals));
    }

    #[test]
    fn v2_orientation_free_pairs_must_be_ordered() {
        let measured = m(&[("leg1", 60.0), ("leg2", 80.0)]);
        let req = SnapRequest { kind: SnapKind::Angle, measured: &measured, family: None, tolerances: None };
        assert_eq!(snap_v2(&catalog(), &req, None), Err(SnapError::ParameterOrder("leg1", "leg2")));
    }

    #[test]
    fn rounding_is_half_away_from_zero() {
        assert_eq!(round6(0.0000005), 0.000001);
        assert_eq!(round6(-0.0000005), -0.000001);
    }
}
