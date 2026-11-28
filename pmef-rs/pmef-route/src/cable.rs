//! Cable routing on cable-tray networks (stage 5).
//!
//! The graph is the tray network itself: tray polylines (`trays`), joined where they share a
//! point. A cable drops from its end point to the nearest point of an allowed tray (at most
//! `maxDrop` away, cost × `dropFactor`), runs along trays and drops to its other end. Rules:
//!
//! - **segregation**: a tray carries only the cable classes it lists (e.g. `power`, `signal`);
//! - **bend radius**: where the path changes direction on or between trays, the cable's
//!   minimum bend radius must not exceed the smallest bend radius of the trays involved;
//! - **fill**: the cables on a tray (cross-section π d²/4 plus the existing fill) must not
//!   exceed `maxFill` of the tray's usable area (width × height). Cables are routed largest
//!   first, each one reserving its area on every tray it uses.
//!
//! The result is a `pmef:CableObject` per cable (route length, main tray) with provenance
//! `generated`, and the fill of every tray in the report.

use crate::geom::P3;
use crate::problem::RouteProblem;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A cable tray (or conduit) run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraySpec {
    /// Tray `@id` (PMEF `CableTrayRun`) or mark.
    pub id: String,
    /// Centreline polyline [mm]; trays that share a point are connected there.
    pub points: Vec<P3>,
    /// Usable width and height [mm].
    pub width: f64,
    pub height: f64,
    /// Cable classes the tray carries (segregation); empty = any.
    #[serde(default)]
    pub classes: Vec<String>,
    /// Inner bend radius of the tray's bends and junctions [mm].
    #[serde(default = "d_tray_bend")]
    pub bend_radius: f64,
    /// Existing fill [% of width × height].
    #[serde(default)]
    pub fill: f64,
    /// Read from a `pmef:CableTrayFitting`: never named as the cable's `cableTrayId`
    /// (spec 02 §6.7 references runs only).
    #[serde(skip)]
    pub fitting: bool,
}
fn d_tray_bend() -> f64 {
    300.0
}

/// Cable end: a point, optionally the PMEF object it terminates at.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CableEnd {
    pub point: P3,
    /// `@id` of the instrument, junction box, PLC etc. (written to `fromId` / `toId`).
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub terminal: Option<String>,
}

/// One cable to route.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CableSpec {
    pub id: String,
    /// Cable tag; default: `id`.
    #[serde(default)]
    pub cable_number: Option<String>,
    /// PMEF `cableType` (e.g. `POWER_LV`, `INSTRUMENTATION_PAIR`).
    #[serde(default)]
    pub cable_type: Option<String>,
    /// Segregation class, matched against the trays' `classes`.
    pub class: String,
    /// Outside diameter [mm].
    pub diameter: f64,
    /// Minimum bend radius [mm] (typically a multiple of the diameter).
    pub min_bend_radius: f64,
    pub from: CableEnd,
    pub to: CableEnd,
    /// Trays (`@id` or mark) the cable may use; empty = any. Cables with trays are routed
    /// before the others, so a viewer can re-route one cable and keep the rest where they are
    /// by giving each of them the trays it used (report `cables[].trays`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trays: Vec<String>,
}

/// Cable routing settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CableSettings {
    /// Maximum fill of a tray [% of width × height].
    #[serde(default = "d_fill")]
    pub max_fill: f64,
    /// Longest drop from a cable end to a tray [mm] (orthogonal distance).
    #[serde(default = "d_drop")]
    pub max_drop: f64,
    /// Cost factor of drops (free-hanging or in conduit) relative to tray length.
    #[serde(default = "d_drop_factor")]
    pub drop_factor: f64,
    /// Route the drops (cable end ↔ tray) through the scene obstacles instead of counting
    /// their orthogonal distance (see `drop.rs`).
    #[serde(default)]
    pub route_drops: bool,
    /// With `routeDrops`: drop candidates per cable end (nearest by L1) that are routed through
    /// the scene during the tray choice.
    #[serde(default = "d_drop_candidates")]
    pub drop_candidates: usize,
    /// Grid spacing of the drop search [mm].
    #[serde(default = "d_drop_step")]
    pub drop_step: f64,
    /// Clearance between a dropped cable and obstacles [mm].
    #[serde(default = "d_drop_clearance")]
    pub drop_clearance: f64,
    /// Read the tray network from the `pmef:CableTrayRun`/`pmef:CableTrayFitting` objects of
    /// `pmefScene` (RFC-0015 centreline and ports). Entries of `trays` with the same `id`
    /// replace a scene tray. See [`trays_from_pmef`].
    #[serde(default)]
    pub trays_from_scene: bool,
    /// Cable classes per tray (`@id` or mark) for trays read from the scene, which carry no
    /// segregation class in PMEF; trays not listed carry any class.
    #[serde(default)]
    pub tray_classes: std::collections::BTreeMap<String, Vec<String>>,
}
fn d_fill() -> f64 {
    40.0
}
fn d_drop() -> f64 {
    5000.0
}
fn d_drop_candidates() -> usize {
    6
}
fn d_drop_step() -> f64 {
    100.0
}
fn d_drop_clearance() -> f64 {
    50.0
}
fn d_drop_factor() -> f64 {
    2.0
}
impl Default for CableSettings {
    fn default() -> Self {
        CableSettings {
            max_fill: d_fill(),
            max_drop: d_drop(),
            drop_factor: d_drop_factor(),
            route_drops: false,
            drop_candidates: d_drop_candidates(),
            drop_step: d_drop_step(),
            drop_clearance: d_drop_clearance(),
            trays_from_scene: false,
            tray_classes: Default::default(),
        }
    }
}

/// A scene tray object that could not become a [`TraySpec`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTray {
    pub id: String,
    pub reason: String,
}

/// Tray network from PMEF objects (RFC-0015, spec 02 §6.8).
///
/// - `pmef:CableTrayRun`: centreline from `path`, else `startPoint`/`endPoint`, else the port
///   coordinates; `width`, `height` and `fillLevel` [%] as given; the bend radius is the
///   smallest `bendRadius` of the fittings joined to the run (default 300 mm).
/// - `pmef:CableTrayFitting` with ports: two ports become one tray from port to port; more
///   ports (tee, cross) become one tray per port from the port centroid, with id
///   `<@id>#<portId>`; width and height from the fitting, else its ports, else a joined run.
///
/// Runs and fittings are joined where their points coincide. Objects without a route, width or
/// height are returned as skipped.
pub fn trays_from_pmef(
    objs: &[serde_json::Value],
    classes: &std::collections::BTreeMap<String, Vec<String>>,
) -> (Vec<TraySpec>, Vec<SkippedTray>) {
    use serde_json::Value;
    let p3 = |v: &Value| -> Option<P3> {
        let a = v.as_array()?;
        (a.len() == 3).then_some(())?;
        Some([a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?])
    };
    let num = |v: &Value| v.as_f64().filter(|x| *x > 0.0);
    // Bend radii may be 0 (sharp junction): only negative or missing values are absent.
    let radius = |v: &Value| v.as_f64().filter(|x| *x >= 0.0);
    let id_of = |o: &Value| o["@id"].as_str().unwrap_or_default().to_string();
    let classes_for = |o: &Value, id: &str| -> Vec<String> {
        for k in [id, o["trayMark"].as_str().unwrap_or(""), o["fittingMark"].as_str().unwrap_or("")] {
            if let Some(c) = classes.get(k).filter(|_| !k.is_empty()) {
                return c.clone();
            }
        }
        Vec::new()
    };
    let runs: Vec<&Value> = objs.iter().filter(|o| o["@type"] == "pmef:CableTrayRun").collect();
    let fittings: Vec<&Value> = objs.iter().filter(|o| o["@type"] == "pmef:CableTrayFitting").collect();
    // Fittings joined to each run (connectedTrayIds or port connectedTo) and their bend radius.
    let joins = |f: &Value, run: &str| -> bool {
        f["connectedTrayIds"].as_array().is_some_and(|a| a.iter().any(|x| x == run))
            || f["ports"].as_array().is_some_and(|a| a.iter().any(|p| p["connectedTo"] == run))
    };
    let mut trays = Vec::new();
    let mut skipped = Vec::new();
    let mut size_of_run = std::collections::HashMap::new();
    for o in &runs {
        let id = id_of(o);
        let ports: Vec<P3> = o["ports"].as_array().into_iter().flatten().filter_map(|p| p3(&p["coordinate"])).collect();
        let path: Vec<P3> = o["path"].as_array().into_iter().flatten().filter_map(p3).collect();
        let points = if path.len() >= 2 {
            path
        } else if let (Some(a), Some(b)) = (p3(&o["startPoint"]), p3(&o["endPoint"])) {
            vec![a, b]
        } else {
            ports
        };
        let (w, h) = (num(&o["width"]), num(&o["height"]));
        let reason = if points.len() < 2 {
            Some("no centreline (path, startPoint/endPoint or two ports)")
        } else if w.is_none() || h.is_none() {
            Some("width or height missing")
        } else {
            None
        };
        if let Some(r) = reason {
            skipped.push(SkippedTray { id, reason: r.into() });
            continue;
        }
        let bend = fittings
            .iter()
            .filter(|f| joins(f, &id))
            .filter_map(|f| radius(&f["bendRadius"]))
            .fold(f64::INFINITY, f64::min);
        size_of_run.insert(id.clone(), (w.unwrap(), h.unwrap()));
        trays.push(TraySpec {
            classes: classes_for(o, &id),
            id,
            points,
            width: w.unwrap(),
            height: h.unwrap(),
            bend_radius: if bend.is_finite() { bend } else { d_tray_bend() },
            fill: o["fillLevel"].as_f64().unwrap_or(0.0),
            fitting: false,
        });
    }
    for f in &fittings {
        let id = id_of(f);
        let ports: Vec<(String, P3, &Value)> = f["ports"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| Some((p["portId"].as_str().unwrap_or("P").to_string(), p3(&p["coordinate"])?, p)))
            .collect();
        if ports.len() < 2 {
            if f["fittingType"] != "END_PIECE" {
                skipped.push(SkippedTray { id, reason: "fewer than two ports with coordinates".into() });
            }
            continue;
        }
        let joined = runs.iter().map(|r| id_of(r)).find(|r| joins(f, r)).and_then(|r| size_of_run.get(&r).copied());
        // Fitting value, else the given ports' (smallest), else the joined run's.
        let size = |ps: &[&Value], k: &str, i: usize| {
            num(&f[k])
                .or_else(|| ps.iter().filter_map(|p| num(&p[k])).reduce(f64::min))
                .or_else(|| joined.map(|s| if i == 0 { s.0 } else { s.1 }))
        };
        let bend = radius(&f["bendRadius"]).unwrap_or_else(d_tray_bend);
        let fill = f["fillLevel"].as_f64().unwrap_or(0.0);
        let mk = |tid: String, pts: Vec<P3>, ps: &[&Value]| -> Result<TraySpec, SkippedTray> {
            match (size(ps, "width", 0), size(ps, "height", 1)) {
                (Some(width), Some(height)) => Ok(TraySpec {
                    classes: classes_for(f, &id),
                    id: tid,
                    points: pts,
                    width,
                    height,
                    bend_radius: bend,
                    fill,
                    fitting: true,
                }),
                _ => Err(SkippedTray { id: tid, reason: "width or height missing".into() }),
            }
        };
        let made: Vec<_> = if ports.len() == 2 {
            // A bend follows its arc when the port directions define one, else the chord.
            let pts = p3(&ports[0].2["direction"])
                .zip(p3(&ports[1].2["direction"]))
                .and_then(|(d1, d2)| bend_arc(ports[0].1, d1, ports[1].1, d2))
                .unwrap_or_else(|| vec![ports[0].1, ports[1].1]);
            vec![mk(id.clone(), pts, &[ports[0].2, ports[1].2])]
        } else {
            let n = ports.len() as f64;
            let c = [0, 1, 2].map(|i| ports.iter().map(|p| p.1[i]).sum::<f64>() / n);
            ports.iter().map(|(pid, p, pv)| mk(format!("{id}#{pid}"), vec![c, *p], &[*pv])).collect()
        };
        for m in made {
            match m {
                Ok(t) => trays.push(t),
                Err(s) => skipped.push(s),
            }
        }
    }
    (trays, skipped)
}

/// Outcome of one cable.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedCable {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Path: cable end, drop point, tray points, drop point, cable end [mm].
    pub points: Vec<P3>,
    /// Trays used, in order.
    pub trays: Vec<String>,
    /// Tray run carrying the longest part of the cable (scene fittings are never chosen).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_tray: Option<String>,
    /// Cable length: tray path (corners as arcs) plus drops [mm].
    pub length_mm: f64,
    pub drop_mm: f64,
    /// How much shorter the corners on tray runs make the path than the sharp polyline [mm].
    pub bend_correction_mm: f64,
    /// Drops routed through the scene (`cableSettings.routeDrops`).
    #[serde(skip_serializing_if = "is_zero")]
    pub routed_drops: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub spec: Option<CableSpec>,
    /// Whether the first / last leg of `points` is a drop.
    #[serde(skip)]
    pub drop_legs: [bool; 2],
}
fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Fill of a tray after routing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayFill {
    pub id: String,
    /// [% of width × height]
    pub fill: f64,
    pub cables: Vec<String>,
}

/// Circular arc of a two-port bend: from `p1` (port direction `d1`, pointing out of the
/// fitting) to `p2` (direction `d2`), tangent to both. `None` when the ports do not lie on such
/// an arc (parallel directions, or the tangent lengths differ by more than 1 mm). Sampled at
/// most every 5°.
pub fn bend_arc(p1: P3, d1: P3, p2: P3, d2: P3) -> Option<Vec<P3>> {
    let unit = |v: P3| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        (l > 1e-12).then(|| [v[0] / l, v[1] / l, v[2] / l])
    };
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (u1, u2) = (unit([-d1[0], -d1[1], -d1[2]])?, unit(d2)?); // travel directions at p1, p2
    let theta = dot(u1, u2).clamp(-1.0, 1.0).acos();
    if !(1e-3..std::f64::consts::PI - 1e-3).contains(&theta) {
        return None;
    }
    // Normal towards the centre: the part of u2 perpendicular to u1.
    let c = dot(u1, u2);
    let m = unit([u2[0] - c * u1[0], u2[1] - c * u1[1], u2[2] - c * u1[2]])?;
    // Chord = 2 R sin(θ/2); the chord must make θ/2 with u1 for a tangent arc.
    let chord = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
    let lc = dist(p1, p2);
    let r = lc / (2.0 * (theta / 2.0).sin());
    let centre = [0, 1, 2].map(|i| p1[i] + r * m[i]);
    if (dist(centre, p2) - r).abs() > 1.0 || (dot(chord, u1) / lc - (theta / 2.0).cos()).abs() > 1e-3 {
        return None;
    }
    let n = ((theta.to_degrees() / 5.0).ceil() as usize).max(2);
    let a = [p1[0] - centre[0], p1[1] - centre[1], p1[2] - centre[2]];
    let mut pts: Vec<P3> = (0..=n)
        .map(|k| {
            let phi = theta * k as f64 / n as f64;
            // Rotate a towards u1 (the travel direction) by phi in the plane (a, u1).
            [0, 1, 2].map(|i| centre[i] + a[i] * phi.cos() + r * u1[i] * phi.sin())
        })
        .collect();
    // The ends are the port coordinates exactly, so that joined runs stay connected even when
    // the ports are within the tolerance but not exactly on the arc.
    pts[0] = p1;
    pts[n] = p2;
    Some(pts)
}

fn dist(a: P3, b: P3) -> f64 {
    ((0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>()).sqrt()
}

fn l1(a: P3, b: P3) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).abs()).sum()
}

fn dir(a: P3, b: P3) -> P3 {
    let l = dist(a, b).max(1e-12);
    [0, 1, 2].map(|i| (b[i] - a[i]) / l)
}

/// Closest point of segment a–b to p, and its parameter in [0, 1].
fn project(p: P3, a: P3, b: P3) -> (P3, f64) {
    let ab = [0, 1, 2].map(|i| b[i] - a[i]);
    let l2: f64 = ab.iter().map(|x| x * x).sum();
    let t = if l2 < 1e-12 {
        0.0
    } else {
        ((0..3).map(|i| (p[i] - a[i]) * ab[i]).sum::<f64>() / l2).clamp(0.0, 1.0)
    };
    ([0, 1, 2].map(|i| a[i] + t * ab[i]), t)
}

/// Graph of one cable: nodes (points), edges (from, to, length, tray index or drop).
struct Graph {
    pts: Vec<P3>,
    adj: Vec<Vec<(usize, f64, Option<usize>)>>,
}

impl Graph {
    fn node(&mut self, p: P3) -> usize {
        if let Some(i) = self.pts.iter().position(|q| dist(*q, p) < 1e-3) {
            return i;
        }
        self.pts.push(p);
        self.adj.push(Vec::new());
        self.pts.len() - 1
    }

    fn edge(&mut self, a: usize, b: usize, len: f64, tray: Option<usize>) {
        if a != b {
            self.adj[a].push((b, len, tray));
            self.adj[b].push((a, len, tray));
        }
    }
}

#[derive(PartialEq)]
struct Open(f64, usize);
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then_with(|| o.1.cmp(&self.1))
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Route all cables of the problem (largest diameter first) and report the tray fill.
/// Routes a drop from a cable end to a tray point past the scene (`drop.rs`): the corner
/// points from the end to the tray point.
pub type DropRouter<'a> = dyn Fn(&CableSpec, P3, P3) -> Result<Vec<P3>, String> + 'a;

pub fn route_cables(problem: &RouteProblem) -> (Vec<RoutedCable>, Vec<TrayFill>) {
    route_cables_with(problem, None)
}

/// [`route_cables`] with drops routed during the tray choice: each end's nearest
/// `dropCandidates` drops are routed with `drop`, and their routed length (× `dropFactor`)
/// is the drop cost. When none of an end's candidates can be routed, its drops keep the L1
/// distance and the cable gets a warning.
pub fn route_cables_with(problem: &RouteProblem, drop: Option<&DropRouter>) -> (Vec<RoutedCable>, Vec<TrayFill>) {
    let set = problem.cable_settings.clone().unwrap_or_default();
    let trays = &problem.trays;
    // Area in use per tray [mm²], and the cables on it.
    let mut used: Vec<f64> = trays
        .iter()
        .map(|t| t.fill / 100.0 * t.width * t.height)
        .collect();
    let mut on: Vec<Vec<String>> = vec![Vec::new(); trays.len()];
    // Cables bound to trays first (their trays are known), then largest first.
    let mut order: Vec<usize> = (0..problem.cables.len()).collect();
    order.sort_by(|a, b| {
        let (ca, cb) = (&problem.cables[*a], &problem.cables[*b]);
        (ca.trays.is_empty())
            .cmp(&cb.trays.is_empty())
            .then(cb.diameter.total_cmp(&ca.diameter))
    });
    let mut out: Vec<Option<RoutedCable>> = vec![None; problem.cables.len()];
    for ci in order {
        let c = &problem.cables[ci];
        let area = std::f64::consts::PI * c.diameter * c.diameter / 4.0;
        let allowed: Vec<bool> = trays
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (t.classes.is_empty() || t.classes.contains(&c.class))
                    && (c.trays.is_empty() || c.trays.contains(&t.id))
                    && used[i] + area <= set.max_fill / 100.0 * t.width * t.height + 1e-9
            })
            .collect();
        let r = route_cable(c, trays, &allowed, &set, drop);
        if r.ok {
            for t in &r.trays {
                let i = trays.iter().position(|x| &x.id == t).unwrap();
                used[i] += area;
                on[i].push(c.id.clone());
            }
        }
        out[ci] = Some(r);
    }
    let fill = trays
        .iter()
        .enumerate()
        .map(|(i, t)| TrayFill {
            id: t.id.clone(),
            fill: (used[i] / (t.width * t.height) * 1000.0).round() / 10.0,
            cables: on[i].clone(),
        })
        .collect();
    (out.into_iter().map(Option::unwrap).collect(), fill)
}

fn route_cable(
    c: &CableSpec,
    trays: &[TraySpec],
    allowed: &[bool],
    set: &CableSettings,
    router: Option<&DropRouter>,
) -> RoutedCable {
    let mut res = RoutedCable {
        id: c.id.clone(),
        ok: false,
        error: None,
        points: Vec::new(),
        trays: Vec::new(),
        main_tray: None,
        length_mm: 0.0,
        drop_mm: 0.0,
        bend_correction_mm: 0.0,
        routed_drops: 0,
        warnings: Vec::new(),
        drop_legs: [false; 2],
        spec: Some(c.clone()),
    };
    let mut g = Graph {
        pts: Vec::new(),
        adj: Vec::new(),
    };
    let start = g.node(c.from.point);
    let goal = g.node(c.to.point);
    // Split every allowed tray segment at its vertices and at the drop points of this cable.
    let mut drops = 0;
    // Drop candidates per end: (L1 distance, tray point, its node).
    let mut cand: [Vec<(f64, P3, usize)>; 2] = [Vec::new(), Vec::new()];
    for (ti, t) in trays.iter().enumerate().filter(|(i, _)| allowed[*i]) {
        for w in t.points.windows(2) {
            let mut cuts: Vec<(f64, P3)> = vec![(0.0, w[0]), (1.0, w[1])];
            for (k, end) in [c.from.point, c.to.point].into_iter().enumerate() {
                let (q, s) = project(end, w[0], w[1]);
                let d = l1(end, q);
                if d <= set.max_drop {
                    let qn = g.node(q);
                    cand[k].push((d, q, qn));
                    cuts.push((s, q));
                    drops += 1;
                }
            }
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
            for k in cuts.windows(2) {
                let (a, b) = (g.node(k[0].1), g.node(k[1].1));
                g.edge(a, b, dist(k[0].1, k[1].1), Some(ti));
            }
        }
    }
    if drops == 0 {
        res.error = Some(format!(
            "no tray for class {} within {} mm of the cable ends (segregation, fill, distance or the cable's trays)",
            c.class, set.max_drop
        ));
        return res;
    }
    // Drop edges: routed through the scene when a router is given, else the L1 distance.
    // Routed polylines run from the end node to the tray node.
    let mut polys: std::collections::HashMap<(usize, usize), Vec<P3>> = std::collections::HashMap::new();
    for (k, (node, end)) in [(start, c.from.point), (goal, c.to.point)].into_iter().enumerate() {
        let list = &mut cand[k];
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        list.dedup_by_key(|x| x.2);
        // A tray edge already joins the end and this tray point (cable end on a tray): no
        // parallel drop edge, so that every path edge is unambiguous.
        list.retain(|x| x.2 != node && !g.adj[node].iter().any(|e| e.0 == x.2));
        let mut routed = 0;
        let mut last_err = None;
        if let Some(rt) = router {
            for &(_, q, qn) in list.iter().take(set.drop_candidates.max(1)) {
                match rt(c, end, q) {
                    Ok(poly) => {
                        let l: f64 = poly.windows(2).map(|w| dist(w[0], w[1])).sum();
                        g.edge(node, qn, l * set.drop_factor, None);
                        polys.insert((node, qn), poly);
                        routed += 1;
                    }
                    Err(e) => last_err = Some(e),
                }
            }
        }
        if routed == 0 {
            for &(d, _, qn) in list.iter() {
                g.edge(node, qn, d * set.drop_factor, None);
            }
            if let (Some(e), false) = (last_err, list.is_empty()) {
                let which = if k == 0 { "from" } else { "to" };
                res.warnings.push(format!("{which} drop kept as L1 distance: {e}"));
            }
        }
    }

    // Dijkstra over (node, incoming edge direction): bends need the tray radius.
    let bend_ok = |d1: P3, d2: P3, r: f64| {
        let dot: f64 = (0..3).map(|i| d1[i] * d2[i]).sum();
        dot > 1.0 - 1e-6 || c.min_bend_radius <= r + 1e-9
    };
    // State = (node, predecessor node); predecessor gives the incoming direction and tray.
    let mut best: std::collections::HashMap<(usize, usize), (f64, (usize, usize))> =
        std::collections::HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut states: Vec<(usize, usize)> = vec![(start, usize::MAX)];
    best.insert((start, usize::MAX), (0.0, (usize::MAX, usize::MAX)));
    heap.push(Open(0.0, 0));
    let mut found = None;
    while let Some(Open(cost, si)) = heap.pop() {
        let (n, prev) = states[si];
        if best.get(&(n, prev)).is_some_and(|b| b.0 < cost - 1e-9) {
            continue;
        }
        if n == goal {
            found = Some((n, prev));
            break;
        }
        let incoming = (prev != usize::MAX).then(|| {
            let tray = g.adj[prev].iter().find(|e| e.0 == n).and_then(|e| e.2);
            (dir(g.pts[prev], g.pts[n]), tray)
        });
        for &(m, len, tray) in &g.adj[n] {
            if m == prev {
                continue;
            }
            if let (Some((d1, Some(t1))), Some(t2)) = (incoming, tray) {
                let r = trays[t1].bend_radius.min(trays[t2].bend_radius);
                if !bend_ok(d1, dir(g.pts[n], g.pts[m]), r) {
                    continue;
                }
            }
            let nc = cost + len;
            let k = (m, n);
            if best.get(&k).is_some_and(|b| b.0 <= nc + 1e-9) {
                continue;
            }
            best.insert(k, (nc, (n, prev)));
            states.push(k);
            heap.push(Open(nc, states.len() - 1));
        }
    }
    let Some(mut s) = found else {
        res.error =
            Some("no path through the tray network (segregation, fill or bend radius)".into());
        return res;
    };
    let mut nodes = vec![s.0];
    while s.1 != usize::MAX {
        let p = best[&s].1;
        nodes.push(p.0);
        s = p;
    }
    nodes.reverse();
    let mut per_tray: Vec<(usize, f64)> = Vec::new();
    // Corners on tray runs are taken at the centreline bend radius (tray bend radius + half
    // the width): each corner shortens the path by 2 R tan(θ/2) − R θ. Scene fittings already
    // carry their arc; drops and tee branches stay as they are.
    let edge_tray = |a: usize, b: usize| g.adj[a].iter().find(|e| e.0 == b).and_then(|e| e.2);
    let mut corner = vec![0.0; nodes.len()];
    for i in 1..nodes.len().saturating_sub(1) {
        let (a, n, b) = (nodes[i - 1], nodes[i], nodes[i + 1]);
        let (Some(t1), Some(t2)) = (edge_tray(a, n), edge_tray(n, b)) else { continue };
        if trays[t1].fitting || trays[t2].fitting {
            continue;
        }
        let (d1, d2) = (dir(g.pts[a], g.pts[n]), dir(g.pts[n], g.pts[b]));
        let theta = (0..3).map(|k| d1[k] * d2[k]).sum::<f64>().clamp(-1.0, 1.0).acos();
        if !(1e-6..std::f64::consts::PI - 1e-3).contains(&theta) {
            continue;
        }
        let rc = |t: usize| trays[t].bend_radius + trays[t].width / 2.0;
        let half = (theta / 2.0).tan();
        // The tangent points must stay on the neighbouring segments (half of each, so that
        // consecutive corners do not overlap).
        let room = dist(g.pts[a], g.pts[n]).min(dist(g.pts[n], g.pts[b])) / 2.0;
        let r = rc(t1).min(rc(t2)).min(room / half);
        corner[i] = 2.0 * r * half - r * theta;
    }
    res.bend_correction_mm = corner.iter().sum();
    // Routed drop between two path nodes, oriented along the path.
    let drop_poly = |a: usize, b: usize| -> Option<Vec<P3>> {
        if let Some(p) = polys.get(&(a, b)) {
            return Some(p.clone());
        }
        polys.get(&(b, a)).map(|p| p.iter().rev().copied().collect())
    };
    for (i, w) in nodes.windows(2).enumerate() {
        let e = g.adj[w[0]].iter().find(|e| e.0 == w[1]).unwrap();
        match e.2 {
            Some(t) => {
                // Half of each corner's shortening is taken from either side.
                let l = dist(g.pts[w[0]], g.pts[w[1]]) - corner[i] / 2.0 - corner[i + 1] / 2.0;
                res.length_mm += l;
                match per_tray.iter_mut().find(|x| x.0 == t) {
                    Some(x) => x.1 += l,
                    None => per_tray.push((t, l)),
                }
            }
            None => {
                let l = drop_poly(w[0], w[1])
                    .map(|p| p.windows(2).map(|s| dist(s[0], s[1])).sum())
                    .unwrap_or_else(|| l1(g.pts[w[0]], g.pts[w[1]]));
                res.length_mm += l;
                res.drop_mm += l;
            }
        }
    }
    res.ok = true;
    if nodes.len() >= 2 {
        res.drop_legs = [
            edge_tray(nodes[0], nodes[1]).is_none(),
            edge_tray(nodes[nodes.len() - 2], nodes[nodes.len() - 1]).is_none(),
        ];
    }
    res.points = vec![g.pts[nodes[0]]];
    for w in nodes.windows(2) {
        // Only a drop leg expands to its routed polyline: a tray edge between the same two
        // nodes (cable end on a tray) keeps its straight segment.
        let is_drop = edge_tray(w[0], w[1]).is_none();
        match drop_poly(w[0], w[1]).filter(|_| is_drop) {
            Some(p) => {
                res.routed_drops += 1;
                res.points.extend_from_slice(&p[1..]);
            }
            None => res.points.push(g.pts[w[1]]),
        }
    }
    res.trays = per_tray.iter().map(|(t, _)| trays[*t].id.clone()).collect();
    // The run carrying the longest part (fittings are never `cableTrayId`, spec 02 §6.7).
    res.main_tray = per_tray
        .iter()
        .filter(|(t, _)| !trays[*t].fitting)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(t, _)| trays[*t].id.clone());
    res
}
