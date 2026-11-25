//! Rectilinear routing grid ("voxel graph").
//!
//! Nodes lie on per-axis tick lists: uniform ticks at `step` plus the exact coordinates of the
//! route terminals, so both ends sit exactly on graph nodes. Every node owns the cell between
//! the mid-points to its neighbours; a node is blocked when its cell, grown by the pipe radius
//! plus clearance, touches an obstacle. Because the cells tile space, thin obstacles (walls,
//! plates) between two nodes are never skipped.

use crate::geom::{tri_aabb, tri_box_overlap, Aabb, Tri, P3};

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
