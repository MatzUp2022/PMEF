//! Rectilinear routing grid ("voxel graph").
//!
//! Nodes lie on per-axis tick lists: uniform ticks at `step` plus the exact coordinates of the
//! route terminals, so both ends sit exactly on graph nodes. Every node owns the cell between
//! the mid-points to its neighbours; a node is blocked when its cell, grown by the pipe radius
//! plus clearance, touches an obstacle. Because the cells tile space, thin obstacles (walls,
//! plates) between two nodes are never skipped.

use crate::geom::{tri_aabb, tri_box_overlap, Aabb, Mesh, Tri, P3};

/// Grid over a region with occupancy layers.
#[derive(Debug, Clone)]
pub struct Grid {
    /// Node coordinates per axis [mm], ascending.
    pub ax: [Vec<f64>; 3],
    lo: [Vec<f64>; 3],
    hi: [Vec<f64>; 3],
    /// Node count per axis.
    pub n: [usize; 3],
    hard: Vec<u64>,
    /// Penetration penalty per node (soft obstacles, e.g. walls and slabs) [mm equivalent].
    soft: Vec<f32>,
    /// Nodes that `mark` leaves untouched (terminal stubs while their own equipment is
    /// rasterised).
    mask: Vec<usize>,
    /// Clearance zone of the line's own tree (branch routing): entered only straight, and
    /// left only through the tee.
    own: Vec<u64>,
}

fn ticks(lo: f64, hi: f64, step: f64, pins: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = Vec::new();
    let far = |x: f64| pins.iter().all(|p| (p - x).abs() > 0.3 * step);
    // Uniform ticks strictly inside [lo, hi]; the upper bound is added explicitly so that no
    // node (and therefore no route) ever lies outside the region.
    let n = ((hi - lo) / step + 1e-9).floor().max(0.0) as usize;
    for i in 0..=n {
        let x = lo + i as f64 * step;
        if x <= hi + 1e-6 && far(x) {
            v.push(x);
        }
    }
    if v.last().map_or(true, |l| hi - l > 1e-6) && far(hi) {
        v.push(hi);
    }
    v.extend(pins.iter().copied().filter(|p| *p >= lo && *p <= hi));
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    v
}

impl Grid {
    /// Build an empty grid over `region` with spacing `step`, pinning the given points.
    pub fn new(region: Aabb, step: f64, pins: &[P3]) -> Self {
        Self::with_ticks(
            region,
            step,
            [0, 1, 2].map(|a| pins.iter().map(|q| q[a]).collect()),
        )
    }

    /// Build an empty grid over `region` with spacing `step` and extra ticks per axis (e.g.
    /// terminal coordinates, rack level elevations). Pins outside the region are dropped.
    pub fn with_ticks(region: Aabb, step: f64, pins: [Vec<f64>; 3]) -> Self {
        let ax: [Vec<f64>; 3] =
            [0, 1, 2].map(|a| ticks(region.min[a], region.max[a], step, &pins[a]));
        let lo = [0, 1, 2].map(|a| {
            (0..ax[a].len())
                .map(|i| {
                    if i == 0 {
                        ax[a][0] - step / 2.0
                    } else {
                        (ax[a][i - 1] + ax[a][i]) / 2.0
                    }
                })
                .collect::<Vec<_>>()
        });
        let hi = [0, 1, 2].map(|a| {
            let k = ax[a].len();
            (0..k)
                .map(|i| {
                    if i + 1 == k {
                        ax[a][i] + step / 2.0
                    } else {
                        (ax[a][i] + ax[a][i + 1]) / 2.0
                    }
                })
                .collect::<Vec<_>>()
        });
        let n = [ax[0].len(), ax[1].len(), ax[2].len()];
        let total = n[0] * n[1] * n[2];
        Grid {
            ax,
            lo,
            hi,
            n,
            hard: vec![0; total.div_ceil(64)],
            soft: Vec::new(),
            mask: Vec::new(),
            own: Vec::new(),
        }
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    /// Whether the grid has no nodes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Linear index of node (i, j, k).
    #[inline]
    pub fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        (i * self.n[1] + j) * self.n[2] + k
    }

    /// (i, j, k) of a linear index.
    #[inline]
    pub fn ijk(&self, idx: usize) -> [usize; 3] {
        let k = idx % self.n[2];
        let r = idx / self.n[2];
        [r / self.n[1], r % self.n[1], k]
    }

    /// Coordinates of a node.
    pub fn point(&self, idx: usize) -> P3 {
        let [i, j, k] = self.ijk(idx);
        [self.ax[0][i], self.ax[1][j], self.ax[2][k]]
    }

    /// Node exactly at `p` (pinned points), if any.
    pub fn node_at(&self, p: P3) -> Option<usize> {
        let f = |a: usize| self.ax[a].iter().position(|x| (x - p[a]).abs() < 1e-6);
        Some(self.idx(f(0)?, f(1)?, f(2)?))
    }

    /// Node index range on axis `a` whose cells touch [x0, x1].
    fn range(&self, a: usize, x0: f64, x1: f64) -> std::ops::Range<usize> {
        let s = self.hi[a].partition_point(|h| *h < x0);
        let e = self.lo[a].partition_point(|l| *l <= x1);
        s..e.max(s)
    }

    #[inline]
    pub fn is_hard(&self, idx: usize) -> bool {
        self.hard[idx / 64] >> (idx % 64) & 1 == 1
    }

    fn set_hard(&mut self, idx: usize, v: bool) {
        if v {
            self.hard[idx / 64] |= 1 << (idx % 64);
        } else {
            self.hard[idx / 64] &= !(1 << (idx % 64));
        }
    }

    /// Whether the node lies in the clearance zone of the line's own tree.
    #[inline]
    pub fn is_own(&self, idx: usize) -> bool {
        !self.own.is_empty() && self.own[idx / 64] >> (idx % 64) & 1 == 1
    }

    /// Mark the nodes whose cell, grown by `d`, touches the box as the own tree's zone.
    pub fn add_own_box(&mut self, b: &Aabb, d: f64) {
        if self.own.is_empty() {
            self.own = vec![0; self.len().div_ceil(64)];
        }
        for idx in self.nodes_in(&b.inflate(d)) {
            self.own[idx / 64] |= 1 << (idx % 64);
        }
    }

    /// Penetration penalty of a node (0 = not inside a soft obstacle).
    #[inline]
    pub fn soft(&self, idx: usize) -> f32 {
        self.soft.get(idx).copied().unwrap_or(0.0)
    }

    fn mark(&mut self, idx: usize, soft_penalty: Option<f32>) {
        if self.mask.binary_search(&idx).is_ok() {
            return;
        }
        match soft_penalty {
            None => self.set_hard(idx, true),
            Some(p) => {
                if self.soft.is_empty() {
                    self.soft = vec![0.0; self.len()];
                }
                self.soft[idx] = self.soft[idx].max(p);
            }
        }
    }

    /// Mark every node whose cell, grown by `d`, touches the box. `soft_penalty = None` blocks.
    pub fn add_box(&mut self, b: &Aabb, d: f64, soft_penalty: Option<f32>) -> usize {
        let b = b.inflate(d);
        let r = [0, 1, 2].map(|a| self.range(a, b.min[a], b.max[a]));
        let mut count = 0;
        for i in r[0].clone() {
            for j in r[1].clone() {
                for k in r[2].clone() {
                    let idx = self.idx(i, j, k);
                    self.mark(idx, soft_penalty);
                    count += 1;
                }
            }
        }
        count
    }

    /// Mark every node whose cell, grown by `d`, touches the triangle.
    pub fn add_triangle(&mut self, t: &Tri, d: f64, soft_penalty: Option<f32>) {
        let b = tri_aabb(t).inflate(d);
        let r = [0, 1, 2].map(|a| self.range(a, b.min[a], b.max[a]));
        for i in r[0].clone() {
            for j in r[1].clone() {
                for k in r[2].clone() {
                    let c = [i, j, k];
                    let center = [0, 1, 2].map(|a| (self.lo[a][c[a]] + self.hi[a][c[a]]) / 2.0);
                    let half = [0, 1, 2].map(|a| (self.hi[a][c[a]] - self.lo[a][c[a]]) / 2.0 + d);
                    if tri_box_overlap(center, half, t) {
                        let idx = self.idx(i, j, k);
                        self.mark(idx, soft_penalty);
                    }
                }
            }
        }
    }

    /// Mark the nodes inside a closed mesh within `b` (e.g. the inside of a thick wall, which
    /// `add_triangle` leaves free: it only marks the nodes near the faces). A vertical line
    /// through every column of nodes is intersected with the triangles; the winding number of
    /// the crossings (sign of the face normal's z) tells inside from outside, so overlapping
    /// solids (wall junctions) and either face orientation work. Columns whose crossings do not
    /// balance (open surfaces) are skipped.
    pub fn fill_mesh_interior(&mut self, m: &Mesh, b: &Aabb, soft_penalty: Option<f32>) {
        // Off-tick column position: no column passes exactly through a shared edge or vertex.
        const JX: f64 = 0.013_7;
        const JY: f64 = 0.007_1;
        let ri = self.range(0, b.min[0], b.max[0]);
        let rj = self.range(1, b.min[1], b.max[1]);
        let mut hits: Vec<(f64, i32)> = Vec::new();
        for i in ri {
            let x = self.ax[0][i] + JX;
            for j in rj.clone() {
                let y = self.ax[1][j] + JY;
                let line = Aabb {
                    min: [x, y, b.min[2] - 1.0],
                    max: [x, y, b.max[2] + 1.0],
                };
                hits.clear();
                let mut low_side = 0;
                for t in m.query(&Aabb {
                    min: [x, y, f64::NEG_INFINITY],
                    max: [x, y, f64::INFINITY],
                }) {
                    if let Some((z, w)) = vertical_hit(t, x, y) {
                        if z < line.min[2] {
                            low_side += w;
                        } else {
                            hits.push((z, w));
                        }
                    }
                }
                if hits.is_empty() && low_side == 0 {
                    continue;
                }
                hits.sort_by(|a, c| a.0.total_cmp(&c.0));
                if low_side + hits.iter().map(|h| h.1).sum::<i32>() != 0 {
                    continue;
                }
                let mut wind = low_side;
                let mut from = f64::NEG_INFINITY;
                let mut spans: Vec<(f64, f64)> = Vec::new();
                for &(z, w) in &hits {
                    let before = wind;
                    wind += w;
                    if before == 0 && wind != 0 {
                        from = z;
                    } else if before != 0 && wind == 0 {
                        spans.push((from, z));
                    }
                }
                for (z0, z1) in spans {
                    for k in self.range(2, z0.max(b.min[2]), z1.min(b.max[2])) {
                        let z = self.ax[2][k];
                        if z > z0 && z < z1 {
                            let idx = self.idx(i, j, k);
                            self.mark(idx, soft_penalty);
                        }
                    }
                }
            }
        }
    }

    /// Nodes whose cell touches the box.
    pub fn nodes_in(&self, b: &Aabb) -> Vec<usize> {
        let r = [0, 1, 2].map(|a| self.range(a, b.min[a], b.max[a]));
        let mut out = Vec::new();
        for i in r[0].clone() {
            for j in r[1].clone() {
                for k in r[2].clone() {
                    out.push(self.idx(i, j, k));
                }
            }
        }
        out
    }

    /// Block every node that lies outside all `keep` boxes (corridor of a coarse route).
    pub fn block_outside(&mut self, keep: &[Aabb]) {
        for idx in 0..self.len() {
            let p = self.point(idx);
            if !keep.iter().any(|b| b.contains(p)) {
                self.set_hard(idx, true);
            }
        }
    }

    /// Number of blocked nodes.
    pub fn blocked(&self) -> usize {
        self.hard.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Nodes on the straight stub from `p` along axis direction `dir` for `len` mm.
    pub fn stub(&self, p: P3, dir: usize, len: f64) -> Vec<usize> {
        let mut out = Vec::new();
        let Some(start) = self.node_at(p) else {
            return out;
        };
        let a = dir / 2;
        let s = if dir % 2 == 0 { 1.0 } else { -1.0 };
        let mut c = self.ijk(start);
        loop {
            out.push(self.idx(c[0], c[1], c[2]));
            if (self.ax[a][c[a]] - p[a]) * s >= len - 1e-6 {
                break;
            }
            if s > 0.0 && c[a] + 1 < self.n[a] {
                c[a] += 1;
            } else if s < 0.0 && c[a] > 0 {
                c[a] -= 1;
            } else {
                break;
            }
        }
        out
    }

    /// Protect `nodes` from further marking until the mask is cleared again (empty slice).
    /// Used to rasterise a terminal's own equipment without blocking the terminal stub, while
    /// every other obstacle still blocks it.
    pub fn set_mask(&mut self, nodes: &[usize]) {
        self.mask = nodes.to_vec();
        self.mask.sort_unstable();
    }

    /// Remove the soft-obstacle penalty from `nodes` (the terminal sleeve is part of the
    /// connected fixture, not an extra penetration).
    pub fn clear_soft(&mut self, nodes: &[usize]) {
        if !self.soft.is_empty() {
            for &i in nodes {
                self.soft[i] = 0.0;
            }
        }
    }

    /// Neighbour of `idx` in direction `dir` (0:+x 1:-x 2:+y 3:-y 4:+z 5:-z).
    #[inline]
    pub fn step(&self, idx: usize, dir: usize) -> Option<usize> {
        let mut c = self.ijk(idx);
        let a = dir / 2;
        if dir % 2 == 0 {
            if c[a] + 1 >= self.n[a] {
                return None;
            }
            c[a] += 1;
        } else {
            if c[a] == 0 {
                return None;
            }
            c[a] -= 1;
        }
        Some(self.idx(c[0], c[1], c[2]))
    }
}

/// Crossing of the vertical line through (x, y) with a triangle: height and winding sign
/// (+1 where the line enters the solid going up, i.e. the face points down).
fn vertical_hit(t: &Tri, x: f64, y: f64) -> Option<(f64, i32)> {
    let (a, b, c) = (t[0], t[1], t[2]);
    let nz = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if nz.abs() < 1e-12 {
        return None;
    }
    // Barycentric coordinates of (x, y) in the projected triangle.
    let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / nz;
    let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / nz;
    let l3 = 1.0 - l1 - l2;
    if l1 < 0.0 || l2 < 0.0 || l3 < 0.0 {
        return None;
    }
    let z = l1 * a[2] + l2 * b[2] + l3 * c[2];
    Some((z, if nz < 0.0 { 1 } else { -1 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_and_thin_wall() {
        let mut g = Grid::new(
            Aabb {
                min: [0.0; 3],
                max: [1000.0, 1000.0, 0.0],
            },
            100.0,
            &[[320.0, 0.0, 0.0]],
        );
        assert!(g.ax[0].contains(&320.0));
        assert!(!g.ax[0].contains(&300.0));
        // 10 mm thick wall at x = 455..465 lies between ticks 400 and 500: still blocks a node.
        g.add_box(
            &Aabb {
                min: [455.0, -10.0, -10.0],
                max: [465.0, 2000.0, 10.0],
            },
            0.0,
            None,
        );
        let blocked: Vec<f64> = (0..g.n[0])
            .filter(|i| g.is_hard(g.idx(*i, 3, 0)))
            .map(|i| g.ax[0][i])
            .collect();
        assert!(!blocked.is_empty());
        assert!(blocked.iter().all(|x| (400.0..=500.0).contains(x)));
    }

    #[test]
    fn ticks_stay_inside_bounds() {
        let g = Grid::new(
            Aabb {
                min: [0.0, 0.0, 0.0],
                max: [1050.0, 0.0, 300.0],
            },
            100.0,
            &[],
        );
        assert_eq!(g.ax[0].first(), Some(&0.0));
        assert_eq!(g.ax[0].last(), Some(&1050.0));
        assert!(g.ax[0].iter().all(|x| (0.0..=1050.0).contains(x)));
        assert_eq!(g.ax[1], vec![0.0]);
        assert_eq!(g.ax[2].last(), Some(&300.0));
    }
}
