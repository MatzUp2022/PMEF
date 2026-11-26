//! Line lists as input (stage 6): routes from P&ID lines mapped to PMEF.
//!
//! DEXPI-Manager's `dexpi-pmef` maps a P&ID onto PMEF: every pipe becomes a
//! `pmef:PipingNetworkSystem` (`lineNumber`, `nominalDiameter`, `pipeClass`, design conditions)
//! and every nozzle it attaches to a `pmef:IsConnectedTo` relationship from the equipment to the
//! line, with the nozzle mark as `connectionPointSource`. A P&ID has no 3D placement, so the
//! nozzle positions come from the 3D scene (`pmefScene`): the P&ID equipment is matched to the
//! scene equipment by `@id` (stable `@id`s), else by `isDerivedFrom` (same functional object),
//! else by tag number (`tagNumber` or `equipmentBasic.tagNumber`).
//!
//! Each line with at least two resolved nozzles becomes a route: the first nozzle is `from`,
//! the second `to`, further nozzles are branches (equal tees). The generated segments attach to
//! the P&ID line (`line` = its `@id`), so the 3D piping is linked to the P&ID without new line
//! objects. Lines that cannot be routed (missing DN or class, fewer than two nozzles in the
//! scene) are listed with the reason.

use crate::problem::{BranchSpec, RouteProblem, RouteSpec, Terminal};
use crate::scene::{Reader, Scene, SceneCache, SceneError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// A PMEF package with lines to route (e.g. `dexpi-pmef` output).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineListSpec {
    /// PMEF NDJSON file, relative to the problem file.
    pub file: String,
    /// Line numbers to route; empty = every line of the file.
    #[serde(default)]
    pub lines: Vec<String>,
    /// Piping class for lines without `pipeClass`.
    #[serde(default)]
    pub pipe_class: Option<String>,
    /// Project piping class codes of the P&ID → PMEF catalog classes (e.g. `"75HB13": "A1A2"`).
    #[serde(default)]
    pub class_map: std::collections::BTreeMap<String, String>,
}

/// A line of a line list that was not turned into a route.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkippedLine {
    pub line: String,
    pub line_number: String,
    pub reason: String,
}

/// Routes from the lines of `objs` (one line list package) against `scene`. `used` holds the
/// route ids already taken in the problem; every generated id is unique against it and added to
/// it (segment, pipe and fitting `@id`s are derived from the route id). Lines whose `@id` is in
/// `given` (the `line` of an explicit route) are left out: the explicit route routes them.
pub fn routes_from_pmef(
    objs: &[Value],
    scene: &Scene,
    spec: &LineListSpec,
    used: &mut HashSet<String>,
    given: &HashSet<String>,
) -> (Vec<RouteSpec>, Vec<SkippedLine>) {
    // P&ID equipment: @id → candidate names in the scene.
    let mut names: HashMap<&str, Vec<&str>> = HashMap::new();
    for o in objs {
        if let Some(id) = o["@id"].as_str() {
            let n = names.entry(id).or_default();
            n.push(id);
            for v in [
                &o["isDerivedFrom"],
                &o["tagNumber"],
                &o["equipmentBasic"]["tagNumber"],
            ] {
                n.extend(v.as_str());
            }
        }
    }
    // Nozzle connections per line, in file order.
    let mut ends: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
    for o in objs {
        if o["@type"] == "pmef:IsConnectedTo" {
            if let (Some(eq), Some(line), Some(mark)) = (
                o["sourceId"].as_str(),
                o["targetId"].as_str(),
                o["connectionPointSource"].as_str(),
            ) {
                ends.entry(line).or_default().push((eq, mark));
            }
        }
    }
    let resolve = |eq: &str, mark: &str| -> Option<String> {
        let mut cands: Vec<&str> = names.get(eq).cloned().unwrap_or_else(|| vec![eq]);
        if !cands.contains(&eq) {
            cands.insert(0, eq);
        }
        cands.iter().find_map(|c| {
            let id = if scene.nozzles.contains_key(&format!("{c}#{mark}")) {
                c.to_string()
            } else {
                scene.aliases.get(*c)?.clone()
            };
            let key = format!("{id}#{mark}");
            scene.nozzles.contains_key(&key).then_some(key)
        })
    };

    let mut routes = Vec::new();
    let mut skipped = Vec::new();
    for o in objs
        .iter()
        .filter(|o| o["@type"] == "pmef:PipingNetworkSystem")
    {
        let Some(line) = o["@id"].as_str() else {
            continue;
        };
        let number = o["lineNumber"].as_str().unwrap_or(line).to_string();
        if given.contains(line) || (!spec.lines.is_empty() && !spec.lines.contains(&number)) {
            continue;
        }
        let mut skip = |reason: String| {
            skipped.push(SkippedLine {
                line: line.to_string(),
                line_number: number.clone(),
                reason,
            })
        };
        let Some(dn) = o["nominalDiameter"].as_f64() else {
            skip("no nominalDiameter".into());
            continue;
        };
        let Some(class) = o["pipeClass"]
            .as_str()
            .map(|c| {
                spec.class_map
                    .get(c)
                    .cloned()
                    .unwrap_or_else(|| c.to_string())
            })
            .or_else(|| spec.pipe_class.clone())
        else {
            skip("no pipeClass (set lineLists[].pipeClass)".into());
            continue;
        };
        let mut terms = Vec::new();
        let mut missing = Vec::new();
        for (eq, mark) in ends.get(line).into_iter().flatten() {
            match resolve(eq, mark) {
                Some(k) if !terms.contains(&k) => terms.push(k),
                Some(_) => {}
                None => missing.push(format!("{eq}#{mark}")),
            }
        }
        if terms.len() < 2 {
            let why = if missing.is_empty() {
                format!("{} nozzle connection(s), need 2", terms.len())
            } else {
                format!("nozzles not in the 3D scene: {}", missing.join(", "))
            };
            skip(why);
            continue;
        }
        let term = |k: &String| Terminal {
            nozzle: Some(k.clone()),
            ..Terminal::default()
        };
        // Route id: local part of the line @id (stable), else the line number.
        let base = match line.strip_prefix("urn:pmef:line:") {
            Some(rest) => local_id(rest.rsplit(':').next().unwrap_or(rest)),
            None => local_id(&number),
        };
        let mut id = base.clone();
        let mut n = 1;
        while used.contains(&id) {
            n += 1;
            id = format!("{base}-{n}");
        }
        used.insert(id.clone());
        let cond = &o["designConditions"];
        routes.push(RouteSpec {
            id,
            from: term(&terms[0]),
            to: term(&terms[1]),
            pipe_class: class,
            dn: dn.round() as u32,
            line: Some(line.to_string()),
            line_number: Some(number.clone()),
            insulation: o["specification"]["insulationThickness"]
                .as_f64()
                .unwrap_or(0.0),
            tracing: 0.0,
            clearance: None,
            design_pressure: cond["designPressure"].as_f64(),
            design_temperature: cond["designTemperature"].as_f64(),
            branches: terms[2..]
                .iter()
                .enumerate()
                .map(|(i, k)| BranchSpec {
                    id: format!("B{}", i + 1),
                    to: term(k),
                })
                .collect(),
        });
    }
    (routes, skipped)
}

/// Append the routes of every `lineLists` entry of `problem` (read through `read`, parsed
/// files kept in `cache`), after the routes given explicitly. A line that an explicit route
/// names as its `line` is not expanded again: a viewer edits a line-list line by copying its
/// generated route (report `lineListRoutes`) into `routes` and changing it there.
pub fn expand(
    problem: &mut RouteProblem,
    scene: &Scene,
    read: &Reader<'_>,
    cache: &mut SceneCache,
) -> Result<Vec<SkippedLine>, SceneError> {
    let mut skipped = Vec::new();
    let mut used: HashSet<String> = problem.routes.iter().map(|r| r.id.clone()).collect();
    let given: HashSet<String> = problem
        .routes
        .iter()
        .filter_map(|r| r.line.clone())
        .collect();
    for spec in problem.line_lists.clone() {
        let objs = cache.ndjson(&spec.file, read)?;
        let (r, s) = routes_from_pmef(&objs, scene, &spec, &mut used, &given);
        problem.routes.extend(r);
        skipped.extend(s);
    }
    Ok(skipped)
}

/// Route id from a line number: letters, digits and `-`, e.g. `6"-CW-201-A1A2` → `6-CW-201-A1A2`.
fn local_id(number: &str) -> String {
    let mut s = String::new();
    for c in number.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() {
        "LINE".into()
    } else {
        s
    }
}
