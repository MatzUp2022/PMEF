//! Route all lines of a problem, one after another; each routed line becomes an obstacle for
//! the following ones.

use crate::geom::{Aabb, P3};
use crate::grid::Grid;
use crate::problem::{BranchSpec, ObstacleKind, Rack, RackAxis, RouteProblem, RouteSpec, Terminal};
use crate::rules::{self, PipeRules, TeeRule};
use crate::scene::{Scene, Shape};
use crate::search::{search, Goal, SearchParams};
use serde::Serialize;
use std::collections::HashMap;

/// Outcome of one route.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedLine {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub pipe_class: String,
    pub dn: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outside_diameter: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipe_entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elbow_entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elbow_centre_to_face: Option<f64>,
    /// Centreline polyline: start, bend points, end [mm].
    pub points: Vec<P3>,
    pub length_mm: f64,
    pub bends: usize,
    /// Number of soft-obstacle penetrations (wall/slab sleeves).
    pub penetrations: usize,
    pub cost: f64,
    pub expansions: usize,
    pub grid_nodes: usize,
    pub blocked_nodes: usize,
    pub millis: u128,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Branches (tee → consumer), in the order of the problem.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<RoutedBranch>,
    /// Pipe supports on horizontal runs (with `supports` in the problem).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub supports: Vec<Support>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tee_entry: Option<String>,
    #[serde(skip)]
    pub rules: Option<PipeRules>,
    #[serde(skip)]
    pub tee: Option<TeeRule>,
    #[serde(skip)]
    pub spec: Option<RouteSpec>,
}

/// A pipe support placed on a horizontal run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Support {
    /// Point on the centreline [mm].
    pub point: P3,
    /// Bottom of the pipe (incl. insulation) below `point`: the support's bearing point [mm].
    pub bearing: P3,
    /// Polyline the support sits on: 0 = run from–to, k = branch k-1.
    pub on: usize,
    /// Scene obstacle (steel) that carries it, if any lies within reach below.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steel: Option<String>,
}

/// Outcome of one branch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedBranch {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Centreline polyline: tee centre, bend points, consumer [mm].
    pub points: Vec<P3>,
    /// Tree polyline the tee sits on: 0 = the run `from`–`to`, k = branch k-1 of the spec.
    pub on: usize,
    pub length_mm: f64,
    pub bends: usize,
    pub cost: f64,
    pub expansions: usize,
    #[serde(skip)]
    pub spec: Option<BranchSpec>,
}

impl RoutedLine {
    /// Centrelines of the run and all routed branches.
    pub fn polylines(&self) -> impl Iterator<Item = &Vec<P3>> {
        std::iter::once(&self.points)
            .chain(self.branches.iter().filter(|b| b.ok).map(|b| &b.points))
    }
}

/// Axis direction index (0:+x 1:-x 2:+y 3:-y 4:+z 5:-z) of a vector, if axis-aligned.
pub fn axis_dir(v: P3) -> Option<usize> {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-9 {
        return None;
    }
    (0..3).find_map(|a| {
        let c = v[a] / len;
        if c > 0.999 {
            Some(2 * a)
        } else if c < -0.999 {
            Some(2 * a + 1)
        } else {
            None
        }
    })
}

fn terminal(t: &Terminal, scene: &Scene, which: &str) -> Result<(P3, usize), String> {
    let (p, d) = match &t.nozzle {
        Some(n) => *scene
            .nozzles
            .get(n)
            .ok_or(format!("{which}: nozzle {n} not in the PMEF scene"))?,
        None => (
            t.point.ok_or(format!("{which}: give point or nozzle"))?,
            t.direction.ok_or(format!("{which}: give direction"))?,
        ),
    };
    let p = t.point.unwrap_or(p);
    let d = t.direction.unwrap_or(d);
    let dir = axis_dir(d).ok_or(format!(
        "{which}: direction {d:?} is not axis-parallel (skewed nozzles: later increment)"
    ))?;
    Ok((p, dir))
}

/// Obstacle id whose clearance a terminal stub may enter.
fn owner(t: &Terminal) -> Option<String> {
    t.owner
        .clone()
        .or_else(|| {
            t.nozzle
                .as_ref()
                .map(|n| n.split('#').next().unwrap_or(n).to_string())
        })
        .or_else(|| t.connected_to.clone())
}

/// Route every line of the problem in order.
pub fn route_all(problem: &RouteProblem, scene: &Scene) -> Vec<RoutedLine> {
    let stubs = reserved_stubs(problem, scene);
    let others = |i: usize| -> Vec<Aabb> {
        stubs
            .iter()
            .filter(|(j, _)| *j != i)
            .map(|(_, b)| *b)
            .collect()
    };
    let route = |i: usize, avoid: &[RoutedLine]| {
        route_one(problem, scene, &problem.routes[i], avoid, &others(i))
    };
    // Lines that are routed, except `skip`.
    let routed = |lines: &[RoutedLine], skip: &[usize]| -> Vec<RoutedLine> {
        lines
            .iter()
            .enumerate()
            .filter(|(j, r)| r.ok && !skip.contains(j))
            .map(|(_, r)| r.clone())
            .collect()
    };

    // 1. Sequential pass in the given order.
    let mut lines: Vec<RoutedLine> = Vec::new();
    for i in 0..problem.routes.len() {
        let r = route(i, &lines);
        lines.push(r);
    }

    // 2. Rip-up and reroute: a line that failed is routed against the scene alone; the lines
    //    its route collides with are ripped up, the failed line goes first, then the ripped-up
    //    lines are routed again. The new solution is kept if it routes more lines (or as many
    //    at lower total cost).
    let mut tried: Vec<usize> = Vec::new();
    for _ in 0..problem.settings.rip_up_rounds {
        let Some(i) = (0..lines.len()).find(|i| !lines[*i].ok && !tried.contains(i)) else {
            break;
        };
        tried.push(i);
        let alone = route(i, &[]);
        if !alone.ok {
            continue; // fails without other lines too (rules, terminals, scene).
        }
        let clear = problem.routes[i]
            .clearance
            .unwrap_or(problem.settings.clearance);
        // The search blocks a node when its *cell* (up to half a step around it) comes within
        // radius plus clearance of a line, so test with that slack: lines a little further
        // away than the clearance still block the graph.
        let slack = problem.grid.step / 2.0;
        let blockers: Vec<usize> = (0..lines.len())
            .filter(|j| *j != i && lines[*j].ok && collides(&alone, &lines[*j], clear + slack))
            .collect();
        if blockers.is_empty() {
            continue;
        }
        let mut cand = lines.clone();
        cand[i] = route(
            i,
            &routed(
                &cand,
                &blockers.iter().copied().chain([i]).collect::<Vec<_>>(),
            ),
        );
        for &b in &blockers {
            // Avoid every routed line except `b` itself and blockers not yet re-routed.
            let pending: Vec<usize> = blockers.iter().copied().skip_while(|x| *x != b).collect();
            cand[b] = route(b, &routed(&cand, &pending));
        }
        let score = |ls: &[RoutedLine]| {
            let ok = ls.iter().filter(|r| r.ok).count();
            let cost: f64 = ls.iter().filter(|r| r.ok).map(|r| r.cost).sum();
            (ok, cost)
        };
        let (n0, c0) = score(&lines);
        let (n1, c1) = score(&cand);
        if n1 > n0 || (n1 == n0 && c1 < c0 - 1e-6) {
            let names: Vec<&str> = blockers
                .iter()
                .map(|b| problem.routes[*b].id.as_str())
                .collect();
            cand[i].notes.push(format!(
                "rip-up: routed before {} (they blocked it in the sequential pass)",
                names.join(", ")
            ));
            for &b in &blockers {
                let id = problem.routes[i].id.clone();
                cand[b].notes.push(format!("rip-up: re-routed after {id}"));
            }
            lines = cand;
            tried.clear();
            tried.push(i);
        }
    }
    if let Some(sup) = &problem.supports {
        for l in lines.iter_mut().filter(|l| l.ok) {
            place_supports(l, scene, sup);
        }
    }
    lines
}

/// Place supports on the horizontal runs of a routed line: first where steel lies within
/// reach below the pipe, then in between so that no gap exceeds the maximum span (and no run
/// end is more than half a span from a support). Supports stay off elbows and tees.
fn place_supports(l: &mut RoutedLine, scene: &Scene, set: &crate::problem::SupportSettings) {
    let (Some(spec), Some(rules)) = (l.spec.clone(), l.rules.clone()) else {
        return;
    };
    let (span, note) = match set.max_span {
        Some(s) => (s, None),
        None => rules::max_span(spec.dn),
    };
    l.notes.extend(note);
    l.notes.push(format!("supports: maximum span {span} mm"));
    let outer = spec.outer(rules.od);
    let c2f = rules.elbow_c2f;
    let (run_c2e, branch_c2e) = l
        .tee
        .as_ref()
        .map_or((0.0, 0.0), |t| (t.run_c2e, t.branch_c2e));
    let tees: Vec<P3> = l
        .branches
        .iter()
        .filter(|b| b.ok)
        .map(|b| b.points[0])
        .collect();
    let steel: Vec<(&str, Aabb)> = scene
        .obstacles
        .iter()
        .filter(|o| o.kind == ObstacleKind::Hard && o.structural)
        .filter_map(|o| match o.shape {
            Shape::Box(b) => Some((o.id.as_str(), b)),
            Shape::Mesh(_) => None,
        })
        .collect();
    let mut runs: Vec<(usize, Vec<P3>)> = vec![(0, l.points.clone())];
    runs.extend(
        l.branches
            .iter()
            .enumerate()
            .filter(|(_, b)| b.ok)
            .map(|(bi, b)| (bi + 1, b.points.clone())),
    );
    let mut out = Vec::new();
    for (k, poly) in runs {
        let n = poly.len();
        for s in 0..n - 1 {
            let (a, b) = (poly[s], poly[s + 1]);
            let Some(ad) = axis_dir([b[0] - a[0], b[1] - a[1], b[2] - a[2]]) else {
                continue;
            };
            let ax = ad / 2;
            if ax == 2 {
                continue; // vertical
            }
            let sign = if ad % 2 == 0 { 1.0 } else { -1.0 };
            let len = (b[ax] - a[ax]).abs();
            let at = |t: f64| {
                let mut q = a;
                q[ax] += sign * t;
                q
            };
            // Usable part of the run: off the elbows and the tee a branch starts at.
            let off = |idx: usize| {
                if idx == 0 && k > 0 {
                    branch_c2e
                } else if idx == 0 || idx == n - 1 {
                    0.0
                } else {
                    c2f
                }
            };
            let (lo, hi) = (off(s), len - off(s + 1));
            if hi - lo < 1.0 {
                continue;
            }
            let bottom = a[2] - outer;
            // Steel under the run, within reach: (position along the run, id).
            let under = |t: f64| -> Option<&str> {
                let q = at(t);
                steel
                    .iter()
                    .filter(|(_, bx)| {
                        bx.max[2] <= bottom + 1e-6
                            && bx.max[2] >= bottom - set.reach
                            && (0..2).all(|i| bx.min[i] <= q[i] && q[i] <= bx.max[i])
                    })
                    .max_by(|x, y| x.1.max[2].total_cmp(&y.1.max[2]))
                    .map(|(id, _)| *id)
            };
            let mut pos: Vec<f64> = steel
                .iter()
                .filter(|(_, bx)| {
                    let other = 1 - ax;
                    bx.max[2] <= bottom + 1e-6
                        && bx.max[2] >= bottom - set.reach
                        && bx.min[other] <= a[other]
                        && a[other] <= bx.max[other]
                })
                .filter_map(|(_, bx)| {
                    // Overlap of the steel with the usable run, as distances from `a`.
                    let (t0, t1) = if sign > 0.0 {
                        (bx.min[ax] - a[ax], bx.max[ax] - a[ax])
                    } else {
                        (a[ax] - bx.max[ax], a[ax] - bx.min[ax])
                    };
                    let (t0, t1) = (t0.max(lo), t1.min(hi));
                    (t0 <= t1).then_some((t0 + t1) / 2.0)
                })
                .collect();
            pos.sort_by(f64::total_cmp);
            pos.dedup_by(|x, y| (*x - *y).abs() < 1.0);
            // Fill: interior gaps up to `span`, run ends up to `span / 2`.
            let mut fill = Vec::new();
            if pos.is_empty() {
                let m = ((hi - lo) / span).ceil().max(1.0) as usize;
                let w = (hi - lo) / m as f64;
                fill.extend((0..m).map(|i| lo + (i as f64 + 0.5) * w));
            } else {
                let first = pos[0];
                let m = ((first - lo - span / 2.0) / span).ceil().max(0.0) as usize;
                fill.extend((1..=m).map(|i| first - i as f64 * span));
                for w in pos.windows(2) {
                    let m = ((w[1] - w[0]) / span).ceil().max(1.0) as usize;
                    let g = (w[1] - w[0]) / m as f64;
                    fill.extend((1..m).map(|i| w[0] + i as f64 * g));
                }
                let last = *pos.last().unwrap();
                let m = ((hi - last - span / 2.0) / span).ceil().max(0.0) as usize;
                fill.extend((1..=m).map(|i| last + i as f64 * span));
            }
            pos.extend(fill.into_iter().map(|t| t.clamp(lo, hi)));
            // Tee centres on this run (distance from `a`): a support must stay `run_c2e` off.
            let tee_ts: Vec<f64> = tees
                .iter()
                .filter(|tc| (0..3).all(|i| i == ax || (tc[i] - a[i]).abs() < 1e-6))
                .map(|tc| (tc[ax] - a[ax]) * sign)
                .filter(|tt| *tt > lo - run_c2e && *tt < hi + run_c2e)
                .collect();
            let off_tee = |t: f64| {
                let mut t = t;
                for tt in &tee_ts {
                    if (t - tt).abs() < run_c2e {
                        t = if t < *tt { tt - run_c2e } else { tt + run_c2e };
                    }
                }
                t
            };
            // Shift supports off tees, then refill gaps the shift opened (a few rounds: a new
            // support may land on a tee again).
            for _ in 0..8 {
                for t in pos.iter_mut() {
                    *t = off_tee(*t);
                }
                pos.sort_by(f64::total_cmp);
                pos.dedup_by(|x, y| (*x - *y).abs() < 1.0);
                let mut add = Vec::new();
                for w in pos.windows(2) {
                    let m = ((w[1] - w[0]) / span - 1e-9).ceil().max(1.0) as usize;
                    let g = (w[1] - w[0]) / m as f64;
                    add.extend((1..m).map(|i| w[0] + i as f64 * g));
                }
                if let (Some(f), Some(l)) = (pos.first().copied(), pos.last().copied()) {
                    if f - lo > span / 2.0 + 1e-6 {
                        add.push((f - span).max(lo));
                    }
                    if hi - l > span / 2.0 + 1e-6 {
                        add.push((l + span).min(hi));
                    }
                }
                if add.is_empty() {
                    break;
                }
                pos.extend(add);
            }
            for t in pos {
                let p = at(t);
                let mut bearing = p;
                bearing[2] -= outer;
                out.push(Support {
                    point: p,
                    bearing,
                    on: k,
                    steel: under(t).map(str::to_string),
                });
            }
        }
    }
    l.supports = out;
}

/// Whether two routed lines come closer than their outer radii plus `clearance`.
fn collides(a: &RoutedLine, b: &RoutedLine, clearance: f64) -> bool {
    let outer = |r: &RoutedLine| {
        r.spec
            .as_ref()
            .map_or(0.0, |s| s.outer(r.outside_diameter.unwrap_or(0.0)))
    };
    let (oa, ob) = (outer(a), outer(b));
    a.polylines().flat_map(|p| p.windows(2)).any(|s| {
        let sa = Aabb::from_points(s[0], s[1]).inflate(oa + clearance - 1.0);
        b.polylines()
            .flat_map(|p| p.windows(2))
            .any(|t| Aabb::from_points(t[0], t[1]).inflate(ob).overlaps(&sa))
    })
}

/// Terminal stubs of every route (route index, box of the straight run in front of the
/// terminal grown by the line's outer radius). A line must not block the stub of another
/// line, whether that line is routed before or after it: otherwise an earlier line passing
/// right in front of a nozzle makes the later line impossible.
fn reserved_stubs(problem: &RouteProblem, scene: &Scene) -> Vec<(usize, Aabb)> {
    let mut out = Vec::new();
    for (i, spec) in problem.routes.iter().enumerate() {
        let Ok(rules) = rules::resolve(
            &spec.pipe_class,
            spec.dn,
            spec.design_pressure,
            spec.design_temperature,
        ) else {
            continue;
        };
        let outer = spec.outer(rules.od);
        // Up to the far side of the first elbow: its centre lies at least centre-to-face +
        // terminal straight out, on the next graph tick (≤ one step further), and the pipe
        // surface reaches the outer radius beyond that.
        let ends = [&spec.from, &spec.to]
            .into_iter()
            .chain(spec.branches.iter().map(|b| &b.to));
        for t in ends {
            let straight = t.straight.unwrap_or(problem.settings.terminal_straight);
            let len = rules.elbow_c2f + straight + problem.grid.step + outer;
            if let Ok((p, dir)) = terminal(t, scene, "terminal") {
                let mut q = p;
                q[dir / 2] += if dir % 2 == 0 { len } else { -len };
                out.push((i, Aabb::from_points(p, q).inflate(outer)));
            }
        }
    }
    out
}

fn failed(spec: &RouteSpec, msg: String) -> RoutedLine {
    RoutedLine {
        id: spec.id.clone(),
        ok: false,
        error: Some(msg),
        pipe_class: spec.pipe_class.clone(),
        dn: spec.dn,
        outside_diameter: None,
        pipe_entry: None,
        elbow_entry: None,
        elbow_centre_to_face: None,
        points: Vec::new(),
        length_mm: 0.0,
        bends: 0,
        penetrations: 0,
        cost: 0.0,
        expansions: 0,
        grid_nodes: 0,
        blocked_nodes: 0,
        millis: 0,
        notes: Vec::new(),
        branches: Vec::new(),
        supports: Vec::new(),
        tee_entry: None,
        rules: None,
        tee: None,
        spec: Some(spec.clone()),
    }
}

/// Everything one graph attempt of a route needs.
struct Ctx<'a> {
    problem: &'a RouteProblem,
    scene: &'a Scene,
    done: &'a [RoutedLine],
    /// Terminal stubs of the other routes.
    reserved: &'a [Aabb],
    rules: &'a PipeRules,
    ends: [(P3, usize); 2],
    owners: [Option<String>; 2],
    /// Outer radius + clearance [mm].
    d: f64,
    /// Straight length before the first / after the last elbow (centre-to-face + terminal
    /// straight) at `from` and `to` [mm].
    needs: [f64; 2],
    /// Rack level centrelines of this line: (rack index, elevation).
    rack_z: Vec<(usize, f64)>,
}

/// Result of one graph attempt.
struct Attempt {
    poly: Vec<P3>,
    penetrations: usize,
    cost: f64,
    expansions: usize,
    grid_nodes: usize,
    blocked_nodes: usize,
}

/// Failure of one graph attempt (message, grid size, blocked nodes).
type AttemptErr = (String, usize, usize);

/// Corner points of a node path.
fn corners(pts: &[P3]) -> Vec<P3> {
    let mut poly = vec![pts[0]];
    for w in 1..pts.len().saturating_sub(1) {
        let (p, q, r) = (pts[w - 1], pts[w], pts[w + 1]);
        let d1 = axis_dir([q[0] - p[0], q[1] - p[1], q[2] - p[2]]);
        let d2 = axis_dir([r[0] - q[0], r[1] - q[1], r[2] - q[2]]);
        if d1 != d2 {
            poly.push(q);
        }
    }
    poly.push(*pts.last().unwrap());
    poly
}

/// Rasterise the scene, the other lines' reserved stubs and the lines routed so far, grown by
/// this line's radius + clearance. `stubs`/`owners`: terminal stubs that only their owner
/// obstacle leaves free; `free`: nodes no obstacle marks (coarse pass near terminals).
fn rasterise(
    c: &Ctx,
    g: &mut Grid,
    region: Aabb,
    corridor: Option<&[Aabb]>,
    stubs: &[Vec<usize>],
    owners: &[Option<String>],
    free: &[usize],
) {
    // Rasterise the scene and the lines routed so far, grown by this line's radius + clearance.
    let reach = region.inflate(c.d);
    let reach_boxes: Vec<Aabb> = match corridor {
        Some(keep) => keep.iter().map(|k| k.inflate(c.d)).collect(),
        None => vec![reach],
    };
    for o in &c.scene.obstacles {
        if !reach_boxes.iter().any(|r| o.bounds.overlaps(r)) {
            continue;
        }
        let mut mask = free.to_vec();
        for (s, w) in stubs.iter().zip(owners) {
            if w.as_deref() == Some(o.id.as_str()) {
                mask.extend(s);
            }
        }
        g.set_mask(&mask);
        let soft = (o.kind == ObstacleKind::Soft).then_some(o.penalty as f32);
        match &o.shape {
            Shape::Box(bx) => {
                g.add_box(bx, c.d, soft);
            }
            Shape::Mesh(m) => {
                for r in &reach_boxes {
                    // Triangles in two overlapping corridor boxes are rasterised twice; marking
                    // is idempotent.
                    for t in m.query(r) {
                        g.add_triangle(t, c.d, soft);
                    }
                    // A soft solid (wall, slab) is a penetration over its whole thickness, not
                    // only near its faces: otherwise a line could run along inside a thick wall.
                    if soft.is_some() {
                        g.fill_mesh_interior(m, r, soft);
                    }
                }
            }
        }
    }
    // Other lines' terminal stubs; this line's own stubs stay free (a real clash between two
    // stubs is then reported by the later line).
    let mut own: Vec<usize> = free.to_vec();
    own.extend(stubs.iter().flatten());
    g.set_mask(&own);
    for r in c.reserved {
        if reach_boxes.iter().any(|x| r.overlaps(x)) {
            g.add_box(r, c.d, None);
        }
    }
    g.set_mask(free);
    for prev in c.done.iter().filter(|r| r.ok) {
        let po = prev
            .spec
            .as_ref()
            .map_or(0.0, |s| s.outer(prev.outside_diameter.unwrap_or(0.0)));
        for poly in prev.polylines() {
            for w in poly.windows(2) {
                g.add_box(&Aabb::from_points(w[0], w[1]).inflate(po), c.d, None);
            }
        }
    }
    g.set_mask(&[]);
    for s in stubs {
        g.clear_soft(s);
    }
}

/// Lowest cost factor of any zone or rack (keeps the A* heuristic admissible).
fn min_factor(problem: &RouteProblem) -> f64 {
    problem
        .zones
        .iter()
        .map(|z| z.factor)
        .chain(problem.racks.iter().map(|r| r.factor))
        .fold(1.0_f64, f64::min)
        .max(1e-3)
}

/// Cost factor of the edge u–v: zones by the edge midpoint, racks along their axis on a level.
/// Overlapping zones and racks: the most preferred (lowest) factor wins; elsewhere 1.
fn edge_factor(c: &Ctx, g: &Grid, u: usize, v: usize) -> f64 {
    let zones = &c.problem.zones;
    let racks = &c.problem.racks;
    let (p, q) = (g.point(u), g.point(v));
    let m = [0, 1, 2].map(|i| (p[i] + q[i]) / 2.0);
    let zone = zones
        .iter()
        .filter(|z| z.aabb.contains(m))
        .map(|z| z.factor);
    let along = |r: &Rack| {
        let a = match r.axis {
            RackAxis::X => 0,
            RackAxis::Y => 1,
        };
        (p[a] - q[a]).abs() > 1e-9 && (0..3).all(|i| i == a || (p[i] - q[i]).abs() < 1e-9)
    };
    let rack = c
        .rack_z
        .iter()
        .filter(|(ri, z)| {
            let r = &racks[*ri];
            (m[2] - z).abs() < 1e-6
                && along(r)
                && (0..2).all(|i| r.footprint.min[i] <= m[i] && m[i] <= r.footprint.max[i])
        })
        .map(|(ri, _)| racks[*ri].factor);
    zone.chain(rack).reduce(f64::min).unwrap_or(1.0)
}

/// Build the graph over `region` with spacing `step` and search it.
///
/// `corridor`: only nodes inside these boxes are free (fine pass of coarse-to-fine).
/// `free_near_terminals`: obstacles within this distance of a terminal are not rasterised
/// (coarse pass: large cells near a nozzle would block it; the fine pass resolves that area).
fn attempt(
    c: &Ctx,
    region: Aabb,
    step: f64,
    corridor: Option<&[Aabb]>,
    free_near_terminals: Option<f64>,
) -> Result<Attempt, AttemptErr> {
    let st = &c.problem.settings;
    let [(a, adir), (b, bdir)] = c.ends;
    let mut pins: [Vec<f64>; 3] = [0, 1, 2].map(|i| vec![a[i], b[i]]);
    for (_, z) in &c.rack_z {
        // The level node plus a guard tick 1 mm below it: the level node's cell then ends just
        // under the centreline, so the rack beam (grown by radius + clearance) leaves it free.
        pins[2].push(*z);
        pins[2].push(*z - 1.0);
    }
    let mut g = Grid::with_ticks(region, step, pins);
    if let Some(keep) = corridor {
        g.block_outside(keep);
    }

    // Terminal stubs: straight runs that leave each terminal before the first elbow. Only the
    // terminal's own equipment may touch them (a nozzle starts inside its equipment's
    // clearance zone); other obstacles and earlier lines keep blocking them.
    let stubs = [
        g.stub(a, adir, c.needs[0].max(c.d + step)),
        g.stub(b, bdir, c.needs[1].max(c.d + step)),
    ];
    let free: Vec<usize> = match free_near_terminals {
        Some(r) => [a, b]
            .iter()
            .flat_map(|p| g.nodes_in(&Aabb::from_points(*p, *p).inflate(r)))
            .collect(),
        None => Vec::new(),
    };

    rasterise(c, &mut g, region, corridor, &stubs, &c.owners, &free);
    let blocked_nodes = g.blocked();
    let fail = |m: String| Err((m, g.len(), blocked_nodes));
    for (s, which) in stubs.iter().zip(["from", "to"]) {
        if s.first().is_some_and(|i| g.is_hard(*i)) {
            return fail(format!(
                "{which}: terminal lies in the clearance zone of an obstacle other than \
                 its own equipment (set `owner` if the obstacle belongs to the terminal)"
            ));
        }
    }
    let (Some(start), Some(goal)) = (g.node_at(a), g.node_at(b)) else {
        return fail("terminal not on the grid".into());
    };

    let min_factor = min_factor(c.problem);
    let factor = |u: usize, v: usize| edge_factor(c, &g, u, v);
    let params = SearchParams {
        start,
        start_dir: adir,
        goal: Goal::Node {
            node: goal,
            dir: bdir ^ 1,
        },
        first_need: c.needs[0],
        between_need: 2.0 * c.rules.elbow_c2f,
        last_need: c.needs[1],
        straight_need: 0.0,
        bend_penalty: st.bend_penalty,
        vertical_factor: st.vertical_factor,
        min_factor,
        max_expansions: st.max_expansions,
    };
    let found = match search(&g, &params, &factor) {
        Ok(f) => f,
        Err(e) => return fail(e.to_string()),
    };
    let pts: Vec<P3> = found.nodes.iter().map(|n| g.point(*n)).collect();
    let penetrations = found
        .nodes
        .windows(2)
        .filter(|w| g.soft(w[1]) > 0.0 && g.soft(w[0]) == 0.0)
        .count();
    Ok(Attempt {
        poly: corners(&pts),
        penetrations,
        cost: found.cost,
        expansions: found.expansions,
        grid_nodes: g.len(),
        blocked_nodes,
    })
}

fn route_one(
    problem: &RouteProblem,
    scene: &Scene,
    spec: &RouteSpec,
    done: &[RoutedLine],
    reserved: &[Aabb],
) -> RoutedLine {
    let t0 = Clock::now();
    let st = &problem.settings;
    let rules = match rules::resolve(
        &spec.pipe_class,
        spec.dn,
        spec.design_pressure,
        spec.design_temperature,
    ) {
        Ok(r) => r,
        Err(e) => return failed(spec, e.to_string()),
    };
    let ((a, adir), (b, bdir)) = match (
        terminal(&spec.from, scene, "from"),
        terminal(&spec.to, scene, "to"),
    ) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(e), _) | (_, Err(e)) => return failed(spec, e),
    };
    let outer = spec.outer(rules.od);
    let clearance = spec.clearance.unwrap_or(st.clearance);
    let d = outer + clearance;
    let step = problem.grid.step;
    let region = problem
        .bounds
        .unwrap_or_else(|| Aabb::from_points(a, b).inflate(problem.grid.margin));
    if !region.contains(a) || !region.contains(b) {
        return failed(spec, "a terminal lies outside the bounds".into());
    }
    let rack_z = problem
        .racks
        .iter()
        .enumerate()
        .flat_map(|(i, r)| {
            r.centrelines(outer, clearance)
                .into_iter()
                .map(move |z| (i, z))
        })
        .filter(|(_, z)| region.min[2] <= *z && *z <= region.max[2])
        .collect();
    let c = Ctx {
        problem,
        scene,
        done,
        reserved,
        rules: &rules,
        ends: [(a, adir), (b, bdir)],
        owners: [owner(&spec.from), owner(&spec.to)],
        d,
        needs: [&spec.from, &spec.to]
            .map(|t| rules.elbow_c2f + t.straight.unwrap_or(st.terminal_straight)),
        rack_z,
    };

    let mut notes = rules.notes.clone();
    let mut result = None;
    if let Some(cs) = problem.grid.coarse_step.filter(|cs| *cs > step) {
        let w = problem.grid.corridor.unwrap_or(cs);
        match attempt(&c, region, cs, None, Some(w)) {
            Ok(coarse) => {
                // Corridor: every coarse run grown by `w`, clipped to the region.
                let keep: Vec<Aabb> = coarse
                    .poly
                    .windows(2)
                    .filter_map(|s| Aabb::from_points(s[0], s[1]).inflate(w).intersect(&region))
                    .collect();
                let sub = keep.iter().copied().reduce(|x, y| x.union(&y)).unwrap();
                match attempt(&c, sub, step, Some(&keep), None) {
                    Ok(fine) => {
                        notes.push(format!(
                            "coarse-to-fine: {cs} mm graph ({} nodes, {} bends), then {step} mm \
                             corridor ±{w} mm",
                            coarse.grid_nodes,
                            coarse.poly.len() - 2
                        ));
                        result = Some(Ok(fine));
                    }
                    Err((e, ..)) => notes.push(format!(
                        "coarse-to-fine: corridor search failed ({e}); full {step} mm graph"
                    )),
                }
            }
            Err((e, ..)) => notes.push(format!(
                "coarse-to-fine: {cs} mm graph failed ({e}); full {step} mm graph"
            )),
        }
    }
    let result = result.unwrap_or_else(|| attempt(&c, region, step, None, None));
    let r = match result {
        Ok(r) => r,
        Err((e, nodes, blocked)) => {
            let mut f = failed(spec, e);
            f.grid_nodes = nodes;
            f.blocked_nodes = blocked;
            f.notes = notes;
            f.millis = t0.elapsed().as_millis();
            return f;
        }
    };
    let length_mm = poly_len(&r.poly);

    let mut line = RoutedLine {
        id: spec.id.clone(),
        ok: true,
        error: None,
        pipe_class: rules.pipe_class.clone(),
        dn: spec.dn,
        outside_diameter: Some(rules.od),
        pipe_entry: Some(rules.pipe_entry.clone()),
        elbow_entry: Some(rules.elbow_entry.clone()),
        elbow_centre_to_face: Some(rules.elbow_c2f),
        bends: r.poly.len() - 2,
        points: r.poly,
        length_mm,
        penetrations: r.penetrations,
        cost: r.cost,
        expansions: r.expansions,
        grid_nodes: r.grid_nodes,
        blocked_nodes: r.blocked_nodes,
        millis: 0,
        notes,
        branches: Vec::new(),
        supports: Vec::new(),
        tee_entry: None,
        rules: None,
        tee: None,
        spec: Some(spec.clone()),
    };
    if !spec.branches.is_empty() {
        if let Err(e) = route_branches(&c, spec, outer, &mut line) {
            let mut f = failed(spec, e);
            f.branches = line.branches;
            f.notes = line.notes;
            f.millis = t0.elapsed().as_millis();
            return f;
        }
    }
    line.millis = t0.elapsed().as_millis();
    line.rules = Some(rules);
    line
}

fn poly_len(poly: &[P3]) -> f64 {
    poly.windows(2)
        .map(|w| (0..3).map(|i| (w[1][i] - w[0][i]).abs()).sum::<f64>())
        .sum()
}

/// L1 distance from `q` to the axis-parallel segment a–b.
fn seg_dist(q: P3, a: P3, b: P3) -> f64 {
    (0..3)
        .map(|i| (q[i] - q[i].clamp(a[i].min(b[i]), a[i].max(b[i]))).abs())
        .sum()
}

/// Route the branches of a line one by one onto its tree (greedy Steiner heuristic: the
/// consumer nearest to the tree goes next). Each branch ends in an equal tee on a straight run
/// of the tree, far enough from elbows, terminals and other tees for the catalog fittings.
fn route_branches(
    c: &Ctx,
    spec: &RouteSpec,
    outer: f64,
    line: &mut RoutedLine,
) -> Result<(), String> {
    let tee = rules::tee(&c.rules.pipe_class, c.rules.dn).map_err(|e| e.to_string())?;
    line.notes.extend(tee.notes.iter().cloned());
    line.tee_entry = Some(tee.entry.clone());
    line.branches = spec
        .branches
        .iter()
        .map(|b| RoutedBranch {
            id: b.id.clone(),
            ok: false,
            error: Some("not routed".into()),
            points: Vec::new(),
            on: 0,
            length_mm: 0.0,
            bends: 0,
            cost: 0.0,
            expansions: 0,
            spec: Some(b.clone()),
        })
        .collect();
    let mut ends = Vec::new();
    for b in &spec.branches {
        let which = format!("branch {}", b.id);
        ends.push(terminal(&b.to, c.scene, &which)?);
    }
    let mut left: Vec<usize> = (0..spec.branches.len()).collect();
    while !left.is_empty() {
        // Consumer nearest to the current tree first.
        let tree: Vec<&Vec<P3>> = line.polylines().collect();
        let dist = |k: usize| {
            tree.iter()
                .flat_map(|p| p.windows(2))
                .map(|w| seg_dist(ends[k].0, w[0], w[1]))
                .fold(f64::INFINITY, f64::min)
        };
        let pos = (0..left.len())
            .min_by(|x, y| dist(left[*x]).total_cmp(&dist(left[*y])))
            .unwrap();
        let k = left.remove(pos);
        let b = &spec.branches[k];
        let straight =
            b.to.straight
                .unwrap_or(c.problem.settings.terminal_straight);
        match attempt_branch(c, line, &tee, outer, ends[k], owner(&b.to), straight) {
            Ok((pts, on, cost, expansions)) => {
                let rb = &mut line.branches[k];
                rb.ok = true;
                rb.error = None;
                rb.length_mm = poly_len(&pts);
                rb.bends = pts.len() - 2;
                rb.points = pts;
                rb.on = on;
                rb.cost = cost;
                rb.expansions = expansions;
                line.cost += cost;
            }
            Err(e) => {
                line.branches[k].error = Some(e.clone());
                return Err(format!("branch {}: {e}", b.id));
            }
        }
    }
    line.tee = Some(tee);
    Ok(())
}

/// Search one branch from its consumer to a tee position on the line's tree.
/// Returns the polyline tee → consumer, the tree polyline index of the tee, cost, expansions.
fn attempt_branch(
    c: &Ctx,
    line: &RoutedLine,
    tee: &TeeRule,
    outer: f64,
    (p, dir): (P3, usize),
    owner: Option<String>,
    straight: f64,
) -> Result<(Vec<P3>, usize, f64, usize), String> {
    let first_need = c.rules.elbow_c2f + straight;
    let st = &c.problem.settings;
    let step = c.problem.grid.step;
    let tree: Vec<&Vec<P3>> = line.polylines().collect();
    let region = match c.problem.bounds {
        Some(b) => b,
        None => tree
            .iter()
            .flat_map(|q| q.iter())
            .fold(Aabb::from_points(p, p), |a, q| {
                a.union(&Aabb::from_points(*q, *q))
            })
            .inflate(c.problem.grid.margin),
    };
    if !region.contains(p) {
        return Err("terminal lies outside the bounds".into());
    }
    let mut pins: [Vec<f64>; 3] = [0, 1, 2].map(|i| vec![p[i]]);
    for q in tree.iter().flat_map(|q| q.iter()) {
        for i in 0..3 {
            pins[i].push(q[i]);
        }
    }
    for (_, z) in &c.rack_z {
        pins[2].push(*z);
        pins[2].push(*z - 1.0);
    }
    let mut g = Grid::with_ticks(region, step, pins);
    let stub = g.stub(p, dir, first_need.max(c.d + step));
    rasterise(
        c,
        &mut g,
        region,
        None,
        std::slice::from_ref(&stub),
        &[owner],
        &[],
    );
    for poly in &tree {
        for w in poly.windows(2) {
            g.add_own_box(&Aabb::from_points(w[0], w[1]).inflate(outer), c.d);
        }
    }
    if stub.first().is_some_and(|i| g.is_hard(*i)) {
        return Err(
            "terminal lies in the clearance zone of an obstacle other than its own \
                    equipment (set `owner` if the obstacle belongs to the terminal)"
                .into(),
        );
    }
    let start = g.node_at(p).ok_or("terminal not on the grid")?;

    // Tee positions: nodes on straight runs, clear of the fittings at both ends of the run and
    // of tees already on it.
    let spec = line.spec.as_ref().ok_or("line without spec")?;
    let ts = |t: &Terminal| t.straight.unwrap_or(st.terminal_straight);
    // Tree runs with their stable id (0 = run from–to, k = branch k-1 of the spec, independent of
    // the routing order) and the straight length
    // required at their start / end terminal.
    let mut runs: Vec<(usize, &Vec<P3>, f64, f64)> =
        vec![(0, &line.points, ts(&spec.from), ts(&spec.to))];
    for (bi, b) in line.branches.iter().enumerate().filter(|(_, b)| b.ok) {
        runs.push((bi + 1, &b.points, 0.0, ts(&spec.branches[bi].to)));
    }
    let tees: Vec<P3> = line
        .branches
        .iter()
        .filter(|b| b.ok)
        .map(|b| b.points[0])
        .collect();
    let mut nodes: HashMap<usize, u8> = HashMap::new();
    let mut on_poly: HashMap<usize, usize> = HashMap::new();
    let mut segs = Vec::new();
    for &(k, poly, ts_start, ts_end) in &runs {
        let n = poly.len();
        for s in 0..n - 1 {
            let (a, b) = (poly[s], poly[s + 1]);
            segs.push((a, b));
            let Some(ad) = axis_dir([b[0] - a[0], b[1] - a[1], b[2] - a[2]]) else {
                continue;
            };
            let ax = ad / 2;
            // Required distance from each end of the run.
            let end_need = |idx: usize| {
                if idx == 0 && k > 0 {
                    tee.run_c2e + tee.branch_c2e // the run starts at a tee centre
                } else if idx == 0 {
                    tee.run_c2e + ts_start // terminal
                } else if idx == n - 1 {
                    tee.run_c2e + ts_end // terminal
                } else {
                    tee.run_c2e + c.rules.elbow_c2f // elbow
                }
            };
            let (na, nb) = (end_need(s), end_need(s + 1));
            let mask: u8 = (0..6).filter(|d| d / 2 != ax).map(|d| 1u8 << d).sum();
            for x in &g.ax[ax] {
                let (lo, hi) = (a[ax].min(b[ax]), a[ax].max(b[ax]));
                if *x < lo || *x > hi {
                    continue;
                }
                let (da, db) = ((x - a[ax]).abs(), (x - b[ax]).abs());
                let mut q = a;
                q[ax] = *x;
                let near_tee = tees.iter().any(|t| {
                    (0..3).map(|i| (t[i] - q[i]).abs()).sum::<f64>() < 2.0 * tee.run_c2e - 1e-6
                });
                if da < na - 1e-6 || db < nb - 1e-6 || near_tee {
                    continue;
                }
                if let Some(node) = g.node_at(q) {
                    *nodes.entry(node).or_insert(0) |= mask;
                    on_poly.entry(node).or_insert(k);
                }
            }
        }
    }
    if nodes.is_empty() {
        return Err("no straight run of the line is long enough for a tee".into());
    }
    let params = SearchParams {
        start,
        start_dir: dir,
        goal: Goal::Tree { nodes, segs },
        first_need,
        between_need: 2.0 * c.rules.elbow_c2f,
        last_need: c.rules.elbow_c2f + tee.branch_c2e,
        straight_need: tee.branch_c2e + straight,
        bend_penalty: st.bend_penalty,
        vertical_factor: st.vertical_factor,
        min_factor: min_factor(c.problem),
        max_expansions: st.max_expansions,
    };
    let factor = |u: usize, v: usize| edge_factor(c, &g, u, v);
    let found = search(&g, &params, &factor).map_err(|e| e.to_string())?;
    let tee_node = *found.nodes.last().unwrap();
    let pts: Vec<P3> = found.nodes.iter().map(|n| g.point(*n)).collect();
    let mut poly = corners(&pts);
    poly.reverse();
    Ok((poly, on_poly[&tee_node], found.cost, found.expansions))
}

/// Wall clock for the report; `std::time::Instant` panics on `wasm32-unknown-unknown`, where
/// the time is reported as 0.
struct Clock(#[cfg(not(target_arch = "wasm32"))] std::time::Instant);
impl Clock {
    fn now() -> Clock {
        Clock(
            #[cfg(not(target_arch = "wasm32"))]
            std::time::Instant::now(),
        )
    }
    fn elapsed(&self) -> std::time::Duration {
        #[cfg(not(target_arch = "wasm32"))]
        return self.0.elapsed();
        #[cfg(target_arch = "wasm32")]
        return std::time::Duration::ZERO;
    }
}
