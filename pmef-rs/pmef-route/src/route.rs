//! Route all lines of a problem, one after another; each routed line becomes an obstacle for
//! the following ones.

use crate::geom::{Aabb, P3};
use crate::grid::Grid;
use crate::problem::{ObstacleKind, Rack, RackAxis, RouteProblem, RouteSpec, Terminal};
use crate::rules::{self, PipeRules};
use crate::scene::{Scene, Shape};
use crate::search::{search, SearchParams};
use serde::Serialize;

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
    #[serde(skip)]
    pub rules: Option<PipeRules>,
    #[serde(skip)]
    pub spec: Option<RouteSpec>,
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
        let blockers: Vec<usize> = (0..lines.len())
            .filter(|j| *j != i && lines[*j].ok && collides(&alone, &lines[*j], clear))
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
    lines
}

/// Whether two routed lines come closer than their outer radii plus `clearance`.
fn collides(a: &RoutedLine, b: &RoutedLine, clearance: f64) -> bool {
    let outer = |r: &RoutedLine| {
        r.outside_diameter.unwrap_or(0.0) / 2.0 + r.spec.as_ref().map_or(0.0, |s| s.insulation)
    };
    let (oa, ob) = (outer(a), outer(b));
    a.points.windows(2).any(|s| {
        let sa = Aabb::from_points(s[0], s[1]).inflate(oa + clearance - 1.0);
        b.points
            .windows(2)
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
        let outer = rules.od / 2.0 + spec.insulation;
        // Up to the far side of the first elbow: its centre lies at least centre-to-face +
        // terminal straight out, on the next graph tick (≤ one step further), and the pipe
        // surface reaches the outer radius beyond that.
        let len = rules.elbow_c2f + problem.settings.terminal_straight + problem.grid.step + outer;
        for (t, which) in [(&spec.from, "from"), (&spec.to, "to")] {
            if let Ok((p, dir)) = terminal(t, scene, which) {
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
        rules: None,
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
    first_need: f64,
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
    let stub_len = c.first_need.max(c.d + step);
    let stubs = [g.stub(a, adir, stub_len), g.stub(b, bdir, stub_len)];
    let free: Vec<usize> = match free_near_terminals {
        Some(r) => [a, b]
            .iter()
            .flat_map(|p| g.nodes_in(&Aabb::from_points(*p, *p).inflate(r)))
            .collect(),
        None => Vec::new(),
    };

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
        let mut mask = free.clone();
        for (s, w) in stubs.iter().zip(&c.owners) {
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
                }
            }
        }
    }
    // Other lines' terminal stubs; this line's own stubs stay free (a real clash between two
    // stubs is then reported by the later line).
    let mut own: Vec<usize> = free.clone();
    own.extend(stubs.iter().flatten());
    g.set_mask(&own);
    for r in c.reserved {
        if reach_boxes.iter().any(|x| r.overlaps(x)) {
            g.add_box(r, c.d, None);
        }
    }
    g.set_mask(&free);
    for prev in c.done.iter().filter(|r| r.ok) {
        let po = prev.outside_diameter.unwrap_or(0.0) / 2.0
            + prev.spec.as_ref().map(|s| s.insulation).unwrap_or(0.0);
        for w in prev.points.windows(2) {
            g.add_box(&Aabb::from_points(w[0], w[1]).inflate(po), c.d, None);
        }
    }
    g.set_mask(&[]);
    for s in &stubs {
        g.clear_soft(s);
    }
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

    let zones = &c.problem.zones;
    let racks = &c.problem.racks;
    let min_factor = zones
        .iter()
        .map(|z| z.factor)
        .chain(racks.iter().map(|r| r.factor))
        .fold(1.0_f64, f64::min)
        .max(1e-3);
    let factor = |u: usize, v: usize| -> f64 {
        let (p, q) = (g.point(u), g.point(v));
        let m = [0, 1, 2].map(|i| (p[i] + q[i]) / 2.0);
        // Overlapping zones and racks: the most preferred (lowest) factor wins; elsewhere 1.
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
    };
    let params = SearchParams {
        start,
        start_dir: adir,
        goal,
        goal_dir: bdir ^ 1,
        first_need: c.first_need,
        between_need: 2.0 * c.rules.elbow_c2f,
        last_need: c.first_need,
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
    let t0 = std::time::Instant::now();
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
    let outer = rules.od / 2.0 + spec.insulation;
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
        first_need: rules.elbow_c2f + st.terminal_straight,
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
    let length_mm = r
        .poly
        .windows(2)
        .map(|w| (0..3).map(|i| (w[1][i] - w[0][i]).abs()).sum::<f64>())
        .sum();

    RoutedLine {
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
        millis: t0.elapsed().as_millis(),
        notes,
        rules: Some(rules),
        spec: Some(spec.clone()),
    }
}
