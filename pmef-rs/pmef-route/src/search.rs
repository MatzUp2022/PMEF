//! Orthogonal A* over the [`Grid`] with piping rules.
//!
//! State = (node, travel direction, "has bent", run-length bucket). Rules enforced during the
//! search, not afterwards:
//! - no reversal; turns are 90° elbows placed at a node;
//! - a bend needs a straight run of at least `first_need` after the start terminal and
//!   `between_need` (= 2 x elbow centre-to-face) after the previous bend, so every elbow fits;
//! - no bend inside a soft obstacle (wall/slab penetrations are straight);
//! - the last run into the end terminal is at least `last_need` long.
//!
//! Cost = length x zone factor (x vertical factor) + bend penalty + penetration penalty.

use crate::grid::Grid;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Search parameters.
#[derive(Debug, Clone)]
pub struct SearchParams {
    pub start: usize,
    /// Travel direction leaving the start terminal.
    pub start_dir: usize,
    pub goal: usize,
    /// Travel direction arriving at the end terminal.
    pub goal_dir: usize,
    pub first_need: f64,
    pub between_need: f64,
    pub last_need: f64,
    /// Equivalent length of one elbow [mm].
    pub bend_penalty: f64,
    /// Cost factor for vertical runs.
    pub vertical_factor: f64,
    /// Lowest possible cost factor (zones) — keeps the heuristic admissible.
    pub min_factor: f64,
    pub max_expansions: usize,
}

/// Search result.
#[derive(Debug, Clone)]
pub struct Found {
    /// Node sequence from start to goal.
    pub nodes: Vec<usize>,
    pub cost: f64,
    pub expansions: usize,
}

/// Why the search failed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SearchError {
    #[error("no route within the region ({expansions} states expanded)")]
    NoPath { expansions: usize },
    #[error("the end terminal is not connected to the start through free space (enclosed by obstacles or outside the region)")]
    Unreachable,
    #[error("search limit reached ({0} states expanded)")]
    Limit(usize),
}

const BUCKETS: u64 = 7;

#[derive(Clone, Copy)]
struct Rec {
    g: f64,
    run: f64,
    parent: u32,
    node: u32,
    dir: u8,
    bent: bool,
    closed: bool,
}

#[derive(PartialEq)]
struct Open(f64, u32);
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

fn opposite(d: usize) -> usize {
    d ^ 1
}

/// Run A*. `factor(a, b)` returns the zone cost factor of the edge between two nodes.
pub fn search(
    g: &Grid,
    p: &SearchParams,
    factor: &dyn Fn(usize, usize) -> f64,
) -> Result<Found, SearchError> {
    if !connected(g, p.start, p.goal) {
        return Err(SearchError::Unreachable);
    }
    let goal_pt = g.point(p.goal);
    let need = |bent: bool| if bent { p.between_need } else { p.first_need };
    let key = |node: usize, dir: usize, bent: bool, run: f64| -> u64 {
        let n = need(bent);
        let b = if run >= n - 1e-6 {
            BUCKETS
        } else {
            ((run / n) * BUCKETS as f64) as u64
        };
        ((node as u64) << 8) | ((dir as u64) << 4) | ((bent as u64) << 3) | b.min(BUCKETS)
    };
    let h = |node: usize, dir: usize| -> f64 {
        let q = g.point(node);
        let d = [0, 1, 2].map(|a| goal_pt[a] - q[a]);
        let manhattan = d[0].abs() + d[1].abs() + d[2].abs() * p.vertical_factor.min(1.0);
        // Lower bound on remaining bends.
        let ahead = {
            let a = dir / 2;
            let s = if dir % 2 == 0 { 1.0 } else { -1.0 };
            (0..3).all(|b| b == a || d[b].abs() < 1e-6) && d[a] * s >= -1e-6
        };
        let bends = if dir == p.goal_dir {
            if ahead {
                0.0
            } else {
                2.0
            }
        } else if dir == opposite(p.goal_dir) {
            2.0
        } else {
            1.0
        };
        manhattan * p.min_factor + bends * p.bend_penalty
    };

    let mut recs: Vec<Rec> = Vec::new();
    let mut best: HashMap<u64, u32> = HashMap::new();
    let mut open = BinaryHeap::new();
    recs.push(Rec {
        g: 0.0,
        run: 0.0,
        parent: u32::MAX,
        node: p.start as u32,
        dir: p.start_dir as u8,
        bent: false,
        closed: false,
    });
    best.insert(key(p.start, p.start_dir, false, 0.0), 0);
    open.push(Open(h(p.start, p.start_dir), 0));
    let mut expansions = 0usize;

    while let Some(Open(_, ri)) = open.pop() {
        let r = recs[ri as usize];
        if r.closed {
            continue;
        }
        recs[ri as usize].closed = true;
        let (node, dir) = (r.node as usize, r.dir as usize);
        if node == p.goal && dir == p.goal_dir && (!r.bent || r.run >= p.last_need - 1e-6) {
            let mut nodes = Vec::new();
            let mut i = ri;
            while i != u32::MAX {
                nodes.push(recs[i as usize].node as usize);
                i = recs[i as usize].parent;
            }
            nodes.reverse();
            nodes.dedup();
            return Ok(Found {
                nodes,
                cost: r.g,
                expansions,
            });
        }
        expansions += 1;
        if expansions > p.max_expansions {
            return Err(SearchError::Limit(expansions));
        }
        let can_turn = r.run >= need(r.bent) - 1e-6 && g.soft(node) == 0.0 && node != p.goal;
        for nd in 0..6 {
            if nd == opposite(dir) || (nd != dir && !can_turn) {
                continue;
            }
            let Some(next) = g.step(node, nd) else {
                continue;
            };
            if g.is_hard(next) {
                continue;
            }
            let (a, b) = (g.point(node), g.point(next));
            let len = (b[nd / 2] - a[nd / 2]).abs();
            let mut cost = len * factor(node, next);
            if nd / 2 == 2 {
                cost *= p.vertical_factor;
            }
            let turned = nd != dir;
            if turned {
                cost += p.bend_penalty;
            }
            let (sn, sc) = (g.soft(next), g.soft(node));
            if sn > 0.0 && sc == 0.0 {
                cost += sn as f64;
            }
            let bent = r.bent || turned;
            let run = if turned { len } else { r.run + len };
            let ng = r.g + cost;
            let k = key(next, nd, bent, run);
            if let Some(&bi) = best.get(&k) {
                let b = &recs[bi as usize];
                if b.closed || b.g <= ng + 1e-9 {
                    continue;
                }
            }
            let ni = recs.len() as u32;
            recs.push(Rec {
                g: ng,
                run,
                parent: ri,
                node: next as u32,
                dir: nd as u8,
                bent,
                closed: false,
            });
            best.insert(k, ni);
            open.push(Open(ng + h(next, nd), ni));
        }
    }
    Err(SearchError::NoPath { expansions })
}

/// Flood fill over free nodes: cheap test before the (much larger) state search.
fn connected(g: &Grid, a: usize, b: usize) -> bool {
    let mut seen = vec![0u64; g.len().div_ceil(64)];
    let mut stack = vec![a];
    seen[a / 64] |= 1 << (a % 64);
    while let Some(n) = stack.pop() {
        if n == b {
            return true;
        }
        for d in 0..6 {
            if let Some(m) = g.step(n, d) {
                if !g.is_hard(m) && seen[m / 64] >> (m % 64) & 1 == 0 {
                    seen[m / 64] |= 1 << (m % 64);
                    stack.push(m);
                }
            }
        }
    }
    false
}
