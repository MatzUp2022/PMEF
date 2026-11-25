//! Route all lines of a problem, one after another; each routed line becomes an obstacle for
//! the following ones.

use crate::geom::{Aabb, P3};
use crate::grid::Grid;
use crate::problem::{ObstacleKind, RouteProblem, RouteSpec, Terminal};
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

/// Route every line of the problem in order.
pub fn route_all(problem: &RouteProblem, scene: &Scene) -> Vec<RoutedLine> {
    let mut done: Vec<RoutedLine> = Vec::new();
    for spec in &problem.routes {
        let r = route_one(problem, scene, spec, &done);
        done.push(r);
    }
    done
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

fn route_one(
    problem: &RouteProblem,
    scene: &Scene,
    spec: &RouteSpec,
    done: &[RoutedLine],
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
    let d = outer + spec.clearance.unwrap_or(st.clearance);
    let step = problem.grid.step;
    let region = problem
        .bounds
        .unwrap_or_else(|| Aabb::from_points(a, b).inflate(problem.grid.margin));
    if !region.contains(a) || !region.contains(b) {
        return failed(spec, "a terminal lies outside the bounds".into());
    }
    let mut g = Grid::new(region, step, &[a, b]);

    // Rasterise the scene and the lines routed so far, grown by this line's radius + clearance.
    let reach = region.inflate(d);
    for o in &scene.obstacles {
        if !o.bounds.overlaps(&reach) {
            continue;
        }
        let soft = (o.kind == ObstacleKind::Soft).then_some(o.penalty as f32);
        match &o.shape {
            Shape::Box(bx) => {
                g.add_box(bx, d, soft);
            }
            Shape::Mesh(tris) => {
                for t in tris {
                    if crate::geom::tri_aabb(t).overlaps(&reach) {
                        g.add_triangle(t, d, soft);
                    }
                }
            }
        }
    }
    for prev in done.iter().filter(|r| r.ok) {
        let po = prev.outside_diameter.unwrap_or(0.0) / 2.0
            + prev.spec.as_ref().map(|s| s.insulation).unwrap_or(0.0);
        for w in prev.points.windows(2) {
            g.add_box(&Aabb::from_points(w[0], w[1]).inflate(po), d, None);
        }
    }
    let blocked_nodes = (0..g.len()).filter(|i| g.is_hard(*i)).count();

    let first_need = rules.elbow_c2f + st.terminal_straight;
    g.carve(a, adir, first_need.max(d + step));
    g.carve(b, bdir, first_need.max(d + step));
    let (Some(start), Some(goal)) = (g.node_at(a), g.node_at(b)) else {
        return failed(spec, "terminal not on the grid".into());
    };

    let zones = &problem.zones;
    let min_factor = zones
        .iter()
        .map(|z| z.factor)
        .fold(1.0_f64, f64::min)
        .max(1e-3);
    let factor = |u: usize, v: usize| -> f64 {
        if zones.is_empty() {
            return 1.0;
        }
        let (p, q) = (g.point(u), g.point(v));
        let m = [0, 1, 2].map(|i| (p[i] + q[i]) / 2.0);
        zones
            .iter()
            .filter(|z| z.aabb.contains(m))
            .map(|z| z.factor)
            .fold(1.0, f64::min)
    };
    let params = SearchParams {
        start,
        start_dir: adir,
        goal,
        goal_dir: bdir ^ 1,
        first_need,
        between_need: 2.0 * rules.elbow_c2f,
        last_need: first_need,
        bend_penalty: st.bend_penalty,
        vertical_factor: st.vertical_factor,
        min_factor,
        max_expansions: st.max_expansions,
    };
    let found = match search(&g, &params, &factor) {
        Ok(f) => f,
        Err(e) => {
            let mut r = failed(spec, e.to_string());
            r.grid_nodes = g.len();
            r.blocked_nodes = blocked_nodes;
            r.millis = t0.elapsed().as_millis();
            return r;
        }
    };

    // Corner points.
    let pts: Vec<P3> = found.nodes.iter().map(|n| g.point(*n)).collect();
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
    let length_mm = poly
        .windows(2)
        .map(|w| (0..3).map(|i| (w[1][i] - w[0][i]).abs()).sum::<f64>())
        .sum();
    let penetrations = found
        .nodes
        .windows(2)
        .filter(|w| g.soft(w[1]) > 0.0 && g.soft(w[0]) == 0.0)
        .count();

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
        bends: poly.len() - 2,
        points: poly,
        length_mm,
        penetrations,
        cost: found.cost,
        expansions: found.expansions,
        grid_nodes: g.len(),
        blocked_nodes,
        millis: t0.elapsed().as_millis(),
        notes: rules.notes.clone(),
        rules: Some(rules),
        spec: Some(spec.clone()),
    }
}
