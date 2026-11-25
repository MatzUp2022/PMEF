//! Piping-class rules for one route, read from the PMEF piping-class catalogs.
//!
//! Exactness comes from the catalogs (PMEF CLAUDE.md): the router never invents dimensions. It
//! takes the outside diameter of the class pipe and the centre-to-face dimension of the class
//! 90° elbow for the requested DN, and checks the line's design conditions against the class
//! limits. Full P/T rating curves are DEXPI-Manager `pipe-class-engine` territory; here only the
//! envelope the class catalog states is checked.

use serde_json::Value;

/// Resolved rules for routing one line in one piping class.
#[derive(Debug, Clone, PartialEq)]
pub struct PipeRules {
    /// Piping class designation, e.g. `A1A2`.
    pub pipe_class: String,
    /// Catalog file id, e.g. `piping-class-a1a2`.
    pub catalog_id: String,
    /// Catalog version, e.g. `2024.2`.
    pub catalog_version: String,
    /// Nominal diameter [mm].
    pub dn: u32,
    /// Outside diameter [mm].
    pub od: f64,
    /// Catalog entry of the straight pipe.
    pub pipe_entry: String,
    /// Catalog entry of the 90° elbow.
    pub elbow_entry: String,
    /// Elbow radius designation as in the catalog (e.g. `LONG_RADIUS`, `1.5D`).
    pub elbow_radius: String,
    /// Centre-to-face of the 90° elbow [mm].
    pub elbow_c2f: f64,
    /// Notes on derived values (e.g. centre-to-face computed from the radius factor).
    pub notes: Vec<String>,
}

/// Rule violations and lookup errors.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RuleError {
    #[error("unknown piping class {0} (no PMEF catalog piping-class-*)")]
    UnknownClass(String),
    #[error("piping class {class} has no {what} for DN {dn}")]
    MissingEntry {
        class: String,
        what: &'static str,
        dn: u32,
    },
    #[error("design {what} {value} exceeds piping class {class} limit {limit}")]
    DesignLimit {
        class: String,
        what: &'static str,
        value: f64,
        limit: f64,
    },
}

fn dn_of(e: &Value) -> Option<u32> {
    e.get("nominalDiameter_mm")
        .or_else(|| e.get("DN"))
        .and_then(Value::as_f64)
        .map(|v| v.round() as u32)
}

/// Find the PMEF piping-class catalog for a class designation (case-insensitive).
pub fn class_catalog(pipe_class: &str) -> Option<Value> {
    pmef_catalogs::ids()
        .filter(|id| id.starts_with("piping-class-"))
        .filter_map(pmef_catalogs::document)
        .find(|d| {
            d.get("pipeClass")
                .and_then(Value::as_str)
                .is_some_and(|c| c.eq_ignore_ascii_case(pipe_class))
        })
}

/// Design pressure limit of the class [Pa gauge-ish as stated by the catalog] and temperature
/// limits [K]. EN catalogs state bar/°C; they are converted.
fn class_limits(doc: &Value) -> (Option<f64>, Option<f64>, Option<f64>) {
    let f = |k: &str| doc.get(k).and_then(Value::as_f64);
    let p = f("designPressure_Pa").or_else(|| f("designPressure_bar").map(|b| b * 1e5));
    let tmax =
        f("designTemperature_max_K").or_else(|| f("designTemperature_max_C").map(|c| c + 273.15));
    let tmin =
        f("designTemperature_min_K").or_else(|| f("designTemperature_min_C").map(|c| c + 273.15));
    (p, tmax, tmin)
}

/// Centre-to-face of a 90° elbow derived from its radius designation (B16.9 / EN 10253:
/// bend radius = factor x nominal size). Returns (value [mm], factor, nominal size [mm]).
fn derived_c2f(radius: &str, dn: u32) -> (f64, f64, f64) {
    let factor = match radius {
        "LONG_RADIUS" => 1.5,
        "SHORT_RADIUS" => 1.0,
        "3D" => 3.0,
        "5D" => 5.0,
        r => r.trim_end_matches('D').parse().unwrap_or(1.5),
    };
    let nps_mm = pmef_catalogs::dn_to_nps(dn)
        .map(|n| n * 25.4)
        .unwrap_or(dn as f64);
    ((factor * nps_mm * 10.0).round() / 10.0, factor, nps_mm)
}

/// Resolve the rules for `pipe_class` / `dn` and check optional design conditions
/// (pressure [Pa], temperature [K]).
pub fn resolve(
    pipe_class: &str,
    dn: u32,
    design_pressure_pa: Option<f64>,
    design_temperature_k: Option<f64>,
) -> Result<PipeRules, RuleError> {
    let doc =
        class_catalog(pipe_class).ok_or_else(|| RuleError::UnknownClass(pipe_class.into()))?;
    let class = doc["pipeClass"].as_str().unwrap_or(pipe_class).to_string();
    let entries = doc["entries"].as_array().cloned().unwrap_or_default();

    let (pmax, tmax, tmin) = class_limits(&doc);
    if let (Some(v), Some(l)) = (design_pressure_pa, pmax) {
        if v > l {
            return Err(RuleError::DesignLimit {
                class,
                what: "pressure [Pa]",
                value: v,
                limit: l,
            });
        }
    }
    if let Some(t) = design_temperature_k {
        if let Some(l) = tmax.filter(|l| t > *l) {
            return Err(RuleError::DesignLimit {
                class,
                what: "temperature [K]",
                value: t,
                limit: l,
            });
        }
        if let Some(l) = tmin.filter(|l| t < *l) {
            return Err(RuleError::DesignLimit {
                class,
                what: "min. temperature [K]",
                value: t,
                limit: l,
            });
        }
    }

    let pipe = entries
        .iter()
        .find(|e| e["componentClass"] == "PIPE" && dn_of(e) == Some(dn))
        .ok_or(RuleError::MissingEntry {
            class: class.clone(),
            what: "PIPE",
            dn,
        })?;
    let od = pipe["outsideDiameter_mm"]
        .as_f64()
        .or_else(|| {
            pmef_catalogs::pipe_sizes()
                .into_iter()
                .find(|s| s.dn == dn)
                .map(|s| s.od_mm)
        })
        .ok_or(RuleError::MissingEntry {
            class: class.clone(),
            what: "outside diameter",
            dn,
        })?;

    // Prefer the long-radius 90° elbow; any 90° elbow otherwise.
    let elbows: Vec<&Value> = entries
        .iter()
        .filter(|e| e["componentClass"] == "ELBOW" && dn_of(e) == Some(dn))
        .filter(|e| e.get("angle_deg").and_then(Value::as_f64).unwrap_or(90.0) == 90.0)
        .collect();
    let elbow = elbows
        .iter()
        .find(|e| e["radius"] == "LONG_RADIUS")
        .or_else(|| elbows.first())
        .ok_or(RuleError::MissingEntry {
            class: class.clone(),
            what: "90° ELBOW",
            dn,
        })?;
    let radius = elbow["radius"]
        .as_str()
        .unwrap_or("LONG_RADIUS")
        .to_string();
    let mut notes = Vec::new();
    let c2f = match elbow.get("centreToFace_mm").and_then(Value::as_f64) {
        Some(v) => v,
        None => {
            let (v, factor, nps_mm) = derived_c2f(&radius, dn);
            notes.push(format!(
                "{} has no centreToFace_mm; derived {factor} x {nps_mm:.1} mm from radius {radius}",
                elbow["catalogId"].as_str().unwrap_or("?")
            ));
            v
        }
    };

    Ok(PipeRules {
        pipe_class: class,
        catalog_id: doc["catalogId"].as_str().unwrap_or_default().into(),
        catalog_version: doc["version"].as_str().unwrap_or_default().into(),
        dn,
        od,
        pipe_entry: pipe["catalogId"].as_str().unwrap_or_default().into(),
        elbow_entry: elbow["catalogId"].as_str().unwrap_or_default().into(),
        elbow_radius: radius,
        elbow_c2f: c2f,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1a2_dn100() {
        let r = resolve("a1a2", 100, None, None).unwrap();
        assert_eq!(r.pipe_entry, "A1A2-PIPE-100");
        assert_eq!(r.elbow_entry, "A1A2-ELB90LR-100");
        assert_eq!(r.od, 114.3);
        assert_eq!(r.elbow_c2f, 152.0);
        assert!(r.notes.is_empty());
    }

    #[test]
    fn c5d1_small_bore_elbows() {
        // Catalog C5D1 2024.3 has a 90° elbow with centreToFace_mm for every pipe size.
        for (dn, c2f) in [(25, 38.1), (40, 57.2), (50, 76.2)] {
            let r = resolve("C5D1", dn, None, None).unwrap();
            assert_eq!(r.elbow_c2f, c2f);
            assert!(r.notes.is_empty());
        }
    }

    #[test]
    fn derived_centre_to_face() {
        assert_eq!(derived_c2f("1.5D", 50).0, 76.2);
        assert_eq!(derived_c2f("LONG_RADIUS", 100).0, 152.4);
        assert_eq!(derived_c2f("3D", 50).0, 152.4);
    }

    #[test]
    fn limits_and_missing() {
        assert!(matches!(
            resolve("XX", 50, None, None),
            Err(RuleError::UnknownClass(_))
        ));
        assert!(matches!(
            resolve("C5D1", 600, None, None),
            Err(RuleError::MissingEntry { .. })
        ));
        assert!(matches!(
            resolve("A1A2", 100, Some(5e6), None),
            Err(RuleError::DesignLimit { .. })
        ));
        assert!(matches!(
            resolve("EN-P1A", 100, None, Some(700.0)),
            Err(RuleError::DesignLimit { .. })
        ));
    }
}
