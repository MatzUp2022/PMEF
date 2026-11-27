//! Cable drops through the obstacle grid (`cableSettings.routeDrops`).
//!
//! The tray search (`cable.rs`) costs a drop by its orthogonal (L1) distance. With
//! `routeDrops` each drop of a routed cable — cable end to the tray point and tray point to
//! the other end — is routed afterwards on an orthogonal grid around both points, past the
//! scene obstacles grown by the cable radius + `dropClearance`. The cable end's own object
//! (`from.id`/`to.id`) and the trays are not obstacles; nodes within one grid step of either
//! end stay free. A drop that cannot be routed keeps its L1 length and adds a warning.

use crate::cable::{RoutedCable, TraySpec};
use crate::geom::{Aabb, P3};
use crate::grid::Grid;
use crate::problem::ObstacleKind;
use crate::scene::{Scene, Shape};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Settings of the drop search.
#[derive(Debug, Clone, Copy)]
pub struct DropSettings {
    /// Grid spacing [mm].
    pub step: f64,
    /// Space around the two drop points [mm].
    pub margin: f64,
    /// Clearance between cable and obstacles [mm].
    pub clearance: f64,
    /// Cost of a direction change [mm of length].
    pub bend_penalty: f64,
    pub max_expansions: usize,
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

/// Route one drop from `a` to `b`. Returns the corner points (from `a` to `b`).
pub fn route_drop(
    scene: &Scene,
    a: P3,
    b: P3,
    radius: f64,
    skip: &[&str],
    set: &DropSettings,
) -> Result<Vec<P3>, String> {
    let region = Aabb::from_points(a, b).inflate(set.margin);
    let mut g = Grid::new(region, set.step, &[a, b]);
    let d = radius + set.clearance;
    let (Some(start), Some(goal)) = (g.node_at(a), g.node_at(b)) else {
        return Err("drop end not on the grid".into());
    };
    let free: Vec<usize> = [a, b]
        .iter()
        .flat_map(|p| g.nodes_in(&Aabb::from_points(*p, *p).inflate(set.step)))
        .collect();
    g.set_mask(&free);
    let reach = region.inflate(d);
    for o in scene.obstacles.iter().filter(|o| o.bounds.overlaps(&reach)) {
        if skip.contains(&o.id.as_str()) {
            continue;
        }
        let soft = (o.kind == ObstacleKind::Soft).then_some(o.penalty as f32);
        match &o.shape {
            Shape::Box(bx) => {
                g.add_box(bx, d, soft);
            }
            Shape::Mesh(m) => {
                for t in m.query(&reach) {
                    g.add_triangle(t, d, soft);
                }
            }
        }
    }
    g.set_mask(&[]);

    // A* over (node, arrival direction); 6 = start (no direction yet).
    let h = |n: usize| {
        let p = g.point(n);
        (0..3).map(|i| (p[i] - b[i]).abs()).sum::<f64>()
    };
    let key = |n: usize, d: usize| n * 7 + d;
    let mut best: std::collections::HashMap<usize, (f64, usize)> = std::collections::HashMap::new();
    let mut heap = BinaryHeap::new();
    best.insert(key(start, 6), (0.0, usize::MAX));
    heap.push(Open(h(start), key(start, 6)));
    let mut expansions = 0;
    let mut end = None;
    while let Some(Open(f, k)) = heap.pop() {
        let (n, dir) = (k / 7, k % 7);
        let g_cost = best[&k].0;
        if f > g_cost + h(n) + 1e-9 {
            continue;
        }
        if n == goal {
            end = Some(k);
            break;
        }
        expansions += 1;
        if expansions > set.max_expansions {
            return Err(format!("drop search limit reached ({expansions} states)"));
        }
        for nd in 0..6 {
            if dir < 6 && nd == (dir ^ 1) {
                continue;
            }
            let Some(m) = g.step(n, nd) else { continue };
            if g.is_hard(m) {
                continue;
            }
            let (p, q) = (g.point(n), g.point(m));
            let len: f64 = (0..3).map(|i| (p[i] - q[i]).abs()).sum();
            let pen = g.soft(m) as f64;
            let bend = if dir < 6 && nd != dir { set.bend_penalty } else { 0.0 };
            let c = g_cost + len + bend + pen;
            let km = key(m, nd);
            if best.get(&km).is_some_and(|x| x.0 <= c + 1e-9) {
                continue;
            }
            best.insert(km, (c, k));
            heap.push(Open(c + h(m), km));
        }
    }
    let Some(mut k) = end else {
        return Err("no free path for the drop between the cable end and the tray".into());
    };
    let mut pts = vec![g.point(k / 7)];
    while let Some(&(_, p)) = best.get(&k) {
        if p == usize::MAX {
            break;
        }
        pts.push(g.point(p / 7));
        k = p;
    }
    pts.reverse();
    // Corner points only; the ends exactly at the input points.
    let mut poly = vec![a];
    for w in pts.windows(3) {
        let d1 = [0, 1, 2].map(|i| w[1][i] - w[0][i]);
        let d2 = [0, 1, 2].map(|i| w[2][i] - w[1][i]);
        // Unit steps of an orthogonal grid: equal directions have equal normalised vectors.
        let n = |d: [f64; 3]| {
            let l = d.iter().map(|x| x.abs()).sum::<f64>().max(1e-12);
            d.map(|x| x / l)
        };
        let (u, v) = (n(d1), n(d2));
        let same = (0..3).all(|i| (u[i] - v[i]).abs() < 1e-9);
        if !same {
            poly.push(w[1]);
        }
    }
    poly.push(b);
    Ok(poly)
}

fn len(poly: &[P3]) -> f64 {
    poly.windows(2)
        .map(|w| (0..3).map(|i| (w[1][i] - w[0][i]).powi(2)).sum::<f64>().sqrt())
        .sum()
}

/// Route the drops of every routed cable through the scene; lengths and points are updated.
pub fn route_drops(cables: &mut [RoutedCable], trays: &[TraySpec], scene: &Scene, set: &DropSettings) {
    let tray_ids: Vec<&str> = trays.iter().map(|t| t.id.split('#').next().unwrap_or(&t.id)).collect();
    for c in cables.iter_mut().filter(|c| c.ok) {
        let Some(spec) = c.spec.clone() else { continue };
        let r = spec.diameter / 2.0;
        let mut skip: Vec<&str> = tray_ids.clone();
        let ids: Vec<String> = [&spec.from.id, &spec.to.id].iter().filter_map(|x| x.as_ref().cloned()).collect();
        skip.extend(ids.iter().map(String::as_str));
        let n = c.points.len();
        // (index of the drop's first point, which end)
        for (i, which, leg) in [(0usize, "from", c.drop_legs[0]), (n.saturating_sub(2), "to", c.drop_legs[1])] {
            if !leg || n < 2 {
                continue;
            }
            let (a, b) = (c.points[i], c.points[i + 1]);
            let l1: f64 = (0..3).map(|k| (a[k] - b[k]).abs()).sum();
            match route_drop(scene, a, b, r, &skip, set) {
                Ok(poly) => {
                    let l = len(&poly);
                    c.length_mm += l - l1;
                    c.drop_mm += l - l1;
                    c.routed_drops += 1;
                    let mut pts = c.points[..i].to_vec();
                    pts.extend(poly);
                    pts.extend_from_slice(&c.points[i + 2..]);
                    c.points = pts;
                }
                Err(e) => c.warnings.push(format!("{which} drop kept as L1 distance: {e}")),
            }
        }
    }
}
