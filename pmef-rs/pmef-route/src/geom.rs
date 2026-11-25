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
}
