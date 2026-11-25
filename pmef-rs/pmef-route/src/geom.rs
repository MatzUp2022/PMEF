//! Minimal geometry for the router: points, axis-aligned boxes, triangle–box overlap.

/// Point / vector in mm.
pub type P3 = [f64; 3];

/// Axis-aligned box in mm.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Aabb {
    pub min: P3,
    pub max: P3,
}

impl Aabb {
    /// Box spanned by two points (any order).
    pub fn from_points(a: P3, b: P3) -> Self {
        Aabb {
            min: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        }
    }

    /// Box grown by `d` on every side.
    pub fn inflate(&self, d: f64) -> Self {
        Aabb {
            min: [self.min[0] - d, self.min[1] - d, self.min[2] - d],
            max: [self.max[0] + d, self.max[1] + d, self.max[2] + d],
        }
    }

    /// Smallest box containing both.
    pub fn union(&self, o: &Aabb) -> Self {
        Aabb {
            min: [0, 1, 2].map(|i| self.min[i].min(o.min[i])),
            max: [0, 1, 2].map(|i| self.max[i].max(o.max[i])),
        }
    }

    /// Whether the boxes overlap (touching counts).
    pub fn overlaps(&self, o: &Aabb) -> bool {
        (0..3).all(|i| self.min[i] <= o.max[i] && o.min[i] <= self.max[i])
    }

    /// Overlap of two boxes, if any.
    pub fn intersect(&self, o: &Aabb) -> Option<Aabb> {
        let b = Aabb {
            min: [0, 1, 2].map(|i| self.min[i].max(o.min[i])),
            max: [0, 1, 2].map(|i| self.max[i].min(o.max[i])),
        };
        (0..3).all(|i| b.min[i] <= b.max[i]).then_some(b)
    }

    /// Whether the point lies inside (boundary counts).
    pub fn contains(&self, p: P3) -> bool {
        (0..3).all(|i| self.min[i] <= p[i] && p[i] <= self.max[i])
    }
}

/// A triangle.
pub type Tri = [P3; 3];

/// Bounding box of a triangle.
pub fn tri_aabb(t: &Tri) -> Aabb {
    Aabb::from_points(t[0], t[1]).union(&Aabb::from_points(t[2], t[2]))
}

/// Triangles that are sorted along a Morton (Z-order) curve and grouped into chunks with
/// bounding boxes, so that a query only tests the triangles of chunks that touch its box.
/// Built once per mesh; each route then visits only the part of a large model (IFC plant
/// models with millions of triangles) that lies near its region.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub tris: Vec<Tri>,
    chunks: Vec<(Aabb, usize, usize)>,
}

/// Triangles per chunk.
const CHUNK: usize = 64;

impl Mesh {
    pub fn new(mut tris: Vec<Tri>) -> Self {
        let Some(all) = tris.iter().map(tri_aabb).reduce(|a, b| a.union(&b)) else {
            return Mesh::default();
        };
        let ext = [0, 1, 2].map(|i| (all.max[i] - all.min[i]).max(1e-9));
        let code = |t: &Tri| -> u64 {
            let mut m = 0u64;
            for a in 0..3 {
                let c = (t[0][a] + t[1][a] + t[2][a]) / 3.0;
                let q = (((c - all.min[a]) / ext[a]) * 1_048_575.0) as u64;
                for bit in 0..20 {
                    m |= ((q >> bit) & 1) << (3 * bit + a);
                }
            }
            m
        };
        tris.sort_by_cached_key(code);
        let chunks = tris
            .chunks(CHUNK)
            .enumerate()
            .map(|(i, c)| {
                let b = c.iter().map(tri_aabb).reduce(|a, b| a.union(&b)).unwrap();
                (b, i * CHUNK, i * CHUNK + c.len())
            })
            .collect();
        Mesh { tris, chunks }
    }

    /// Bounding box of all triangles.
    pub fn bounds(&self) -> Option<Aabb> {
        self.chunks.iter().map(|c| c.0).reduce(|a, b| a.union(&b))
    }

    /// Triangles whose bounding box touches `b`.
    pub fn query<'a>(&'a self, b: &'a Aabb) -> impl Iterator<Item = &'a Tri> + 'a {
        self.chunks
            .iter()
            .filter(move |c| c.0.overlaps(b))
            .flat_map(move |c| self.tris[c.1..c.2].iter())
            .filter(move |t| tri_aabb(t).overlaps(b))
    }
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Separating-axis test triangle vs. box (Akenine-Möller): true if they overlap.
pub fn tri_box_overlap(center: P3, half: P3, t: &Tri) -> bool {
    let v = [sub(t[0], center), sub(t[1], center), sub(t[2], center)];
    // 1) box face normals = AABB of the triangle vs box.
    for i in 0..3 {
        let lo = v[0][i].min(v[1][i]).min(v[2][i]);
        let hi = v[0][i].max(v[1][i]).max(v[2][i]);
        if lo > half[i] || hi < -half[i] {
            return false;
        }
    }
    let e = [sub(v[1], v[0]), sub(v[2], v[1]), sub(v[0], v[2])];
    // 2) triangle normal.
    let n = cross(e[0], e[1]);
    let r = half[0] * n[0].abs() + half[1] * n[1].abs() + half[2] * n[2].abs();
    if dot(n, v[0]).abs() > r {
        return false;
    }
    // 3) nine edge x axis cross products.
    let axes: [P3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for ed in &e {
        for ax in &axes {
            let a = cross(*ed, *ax);
            if dot(a, a) < 1e-18 {
                continue;
            }
            let p = [dot(a, v[0]), dot(a, v[1]), dot(a, v[2])];
            let lo = p[0].min(p[1]).min(p[2]);
            let hi = p[0].max(p[1]).max(p[2]);
            let r = half[0] * a[0].abs() + half[1] * a[1].abs() + half[2] * a[2].abs();
            if lo > r || hi < -r {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_box() {
        let t: Tri = [[-5.0, -5.0, 0.0], [5.0, -5.0, 0.0], [0.0, 5.0, 0.0]];
        assert!(tri_box_overlap([0.0, 0.0, 0.5], [1.0, 1.0, 1.0], &t));
        assert!(!tri_box_overlap([0.0, 0.0, 2.5], [1.0, 1.0, 1.0], &t));
        // Box beside the slanted edge, inside the triangle's AABB.
        assert!(!tri_box_overlap([4.5, 4.5, 0.0], [0.5, 0.5, 0.5], &t));
    }

    #[test]
    fn mesh_query_matches_brute_force() {
        // 40 x 40 grid of small triangles in the z = 0 plane.
        let mut tris = Vec::new();
        for i in 0..40 {
            for j in 0..40 {
                let (x, y) = (i as f64 * 100.0, j as f64 * 100.0);
                tris.push([[x, y, 0.0], [x + 50.0, y, 0.0], [x, y + 50.0, 0.0]]);
            }
        }
        let m = Mesh::new(tris.clone());
        let q = Aabb {
            min: [1020.0, 480.0, -1.0],
            max: [1530.0, 910.0, 1.0],
        };
        let brute = tris.iter().filter(|t| tri_aabb(t).overlaps(&q)).count();
        assert_eq!(m.query(&q).count(), brute);
        assert!(brute > 0);
        assert_eq!(m.tris.len(), 1600);
    }
}
