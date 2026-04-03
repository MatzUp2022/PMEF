//! PMEF geometry: parametric primitives, bounding boxes, and clash detection.
//!
//! This crate provides the core geometric types used by the PMEF specification,
//! including 3D vectors, axis-aligned bounding boxes, and the 15 parametric
//! primitive types defined in PMEF-SPEC-04.

// ---------------------------------------------------------------------------
// Vec3
// ---------------------------------------------------------------------------

/// A 3-component vector (or point) in 3D space.
///
/// Coordinates are in millimetres, consistent with PMEF conventions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    /// Create a new vector from (x, y, z) components.
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The zero vector.
    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Euclidean length (magnitude) of the vector.
    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Dot product with another vector.
    pub fn dot(&self, other: &Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Cross product with another vector.
    pub fn cross(&self, other: &Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// Return a unit-length vector in the same direction.
    ///
    /// Returns the zero vector if the length is (near) zero.
    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len < 1e-15 {
            Self::zero()
        } else {
            Self::new(self.x / len, self.y / len, self.z / len)
        }
    }

    /// Euclidean distance to another point.
    pub fn distance_to(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Component-wise minimum.
    pub fn min_components(&self, other: &Self) -> Self {
        Self::new(
            self.x.min(other.x),
            self.y.min(other.y),
            self.z.min(other.z),
        )
    }

    /// Component-wise maximum.
    pub fn max_components(&self, other: &Self) -> Self {
        Self::new(
            self.x.max(other.x),
            self.y.max(other.y),
            self.z.max(other.z),
        )
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

// ---------------------------------------------------------------------------
// Aabb
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box defined by its minimum and maximum corners.
///
/// All coordinates are in millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Create an AABB from explicit min/max corners.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB containing a single point.
    pub fn from_point(p: Vec3) -> Self {
        Self { min: p, max: p }
    }

    /// Return the union of two AABBs (the smallest AABB enclosing both).
    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: self.min.min_components(&other.min),
            max: self.max.max_components(&other.max),
        }
    }

    /// Expand this AABB to include a point.
    pub fn include_point(&mut self, p: Vec3) {
        self.min = self.min.min_components(&p);
        self.max = self.max.max_components(&p);
    }

    /// Test whether a point lies inside (or on the boundary of) this AABB.
    pub fn contains_point(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// Centre point of the AABB.
    pub fn center(&self) -> Vec3 {
        Vec3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// Half-extents along each axis (distance from centre to face).
    pub fn extents(&self) -> Vec3 {
        Vec3::new(
            (self.max.x - self.min.x) * 0.5,
            (self.max.y - self.min.y) * 0.5,
            (self.max.z - self.min.z) * 0.5,
        )
    }

    /// Volume of the AABB.
    pub fn volume(&self) -> f64 {
        let e = self.max - self.min;
        e.x * e.y * e.z
    }

    /// Test whether two AABBs overlap (share at least one point).
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }
}

// ---------------------------------------------------------------------------
// PrimitiveType — the 15 PMEF parametric primitives
// ---------------------------------------------------------------------------

/// The 15 parametric primitive types defined in PMEF-SPEC-04 §4.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    /// Right circular cylinder (pipe runs, vessel shells, nozzle barrels).
    Cylinder,
    /// Rectangular cuboid (tanks, junction boxes, equipment outlines).
    Box,
    /// Full sphere or hemisphere (spherical tanks, ball valve bodies).
    Sphere,
    /// Truncated cone / frustum (concentric reducers, cone-roof tanks).
    Cone,
    /// Full torus (seldom used directly; see CircularTorus for elbows).
    Torus,
    /// Vessel dished head (pressure vessel heads, tank domes).
    Dish,
    /// Parametric nozzle: barrel cylinder + flange disc.
    Nozzle,
    /// Right pyramid (hoppers, bunkers).
    Pyramid,
    /// Eccentric frustum with lateral offset (eccentric reducers).
    Snout,
    /// Circular torus segment (pipe elbows and bends).
    CircularTorus,
    /// Rectangular cross-section torus segment (duct elbows).
    RectangularTorus,
    /// Elliptical vessel head (2:1 elliptical heads).
    EllipticalHead,
    /// Flat cap / blind flange.
    FlatCap,
    /// Linear extrusion of a 2D profile (structural steel members).
    Extrusion,
    /// Surface of revolution (flanges, torispherical heads).
    Revolution,
}

// ---------------------------------------------------------------------------
// PrimitiveParams — generic parameter bag for AABB computation
// ---------------------------------------------------------------------------

/// Parameters describing a parametric primitive.
///
/// Not every field applies to every primitive type. Unused fields are `None`.
/// All lengths are in millimetres and all angles in degrees.
#[derive(Debug, Clone, Default)]
pub struct PrimitiveParams {
    // Common
    /// Outer radius (cylinder, sphere, cone r1, torus tube, etc.).
    pub radius: Option<f64>,
    /// Height or axial length.
    pub height: Option<f64>,
    /// Length (box x, extrusion length, nozzle projection, etc.).
    pub length: Option<f64>,
    /// Width (box y, rectangular torus width, etc.).
    pub width: Option<f64>,
    /// Depth (box z, dish depth, etc.).
    pub depth: Option<f64>,

    // Cone / Snout
    /// Second radius for cones, snouts, etc.
    pub radius2: Option<f64>,

    // Snout offsets
    /// Lateral offset of the top circle centre (X component).
    pub offset_x: Option<f64>,
    /// Lateral offset of the top circle centre (Y component).
    pub offset_y: Option<f64>,

    // Torus
    /// Major (centreline bend) radius for torus primitives.
    pub torus_radius: Option<f64>,
    /// Tube (minor) radius for torus primitives.
    pub tube_radius: Option<f64>,
    /// Sweep angle in degrees for circular/rectangular torus.
    pub angle_deg: Option<f64>,

    // Nozzle
    /// Flange outside diameter.
    pub flange_od: Option<f64>,
    /// Flange thickness.
    pub flange_thickness: Option<f64>,

    // Revolution / Extrusion
    /// Maximum radial extent of a 2D profile (for Revolution/Extrusion AABB).
    pub profile_max_r: Option<f64>,
    /// Maximum Z extent of a 2D profile in the local half-plane.
    pub profile_max_z: Option<f64>,
    /// Minimum Z extent of a 2D profile in the local half-plane.
    pub profile_min_z: Option<f64>,

    // Pyramid
    /// Base length (X) for pyramid.
    pub base_length: Option<f64>,
    /// Base width (Y) for pyramid.
    pub base_width: Option<f64>,
    /// Top length (X) for truncated pyramid.
    pub top_length: Option<f64>,
    /// Top width (Y) for truncated pyramid.
    pub top_width: Option<f64>,

    // Axis direction (unit vector). Defaults to Z-up [0,0,1].
    /// Axis direction for oriented primitives.
    pub axis: Option<[f64; 3]>,
}

// ---------------------------------------------------------------------------
// AABB helpers
// ---------------------------------------------------------------------------

/// Helper: given an axis-aligned unit vector, compute the per-axis
/// "radius extent" of a cylinder/cone disc of the given radius.
///
/// For a disc perpendicular to unit vector `a`, the extent in each
/// world axis is `r * sqrt(1 - a_i^2)`.
fn disc_extent(axis: &[f64; 3], radius: f64) -> Vec3 {
    Vec3::new(
        radius * (1.0 - axis[0] * axis[0]).max(0.0).sqrt(),
        radius * (1.0 - axis[1] * axis[1]).max(0.0).sqrt(),
        radius * (1.0 - axis[2] * axis[2]).max(0.0).sqrt(),
    )
}

/// Build an AABB enclosing a cylinder (or cylinder-like) shape with the
/// given centre (of one face), axis, radius, and length.
fn cylinder_aabb_inner(center: Vec3, axis: &[f64; 3], radius: f64, length: f64) -> Aabb {
    let end = Vec3::new(
        center.x + axis[0] * length,
        center.y + axis[1] * length,
        center.z + axis[2] * length,
    );
    let ext = disc_extent(axis, radius);
    Aabb::new(
        Vec3::new(
            center.x.min(end.x) - ext.x,
            center.y.min(end.y) - ext.y,
            center.z.min(end.z) - ext.z,
        ),
        Vec3::new(
            center.x.max(end.x) + ext.x,
            center.y.max(end.y) + ext.y,
            center.z.max(end.z) + ext.z,
        ),
    )
}

// ---------------------------------------------------------------------------
// primitive_aabb — conservative AABB for each primitive type
// ---------------------------------------------------------------------------

/// Compute a conservative (over-estimating) axis-aligned bounding box for a
/// PMEF parametric primitive located at `origin` with the given parameters.
///
/// The AABB is guaranteed to fully contain the primitive, but may be larger
/// than the tightest possible enclosure. Correctness is prioritised over
/// tightness.
///
/// # Panics
///
/// Panics if required parameters for the given primitive type are `None`.
pub fn primitive_aabb(
    ptype: &PrimitiveType,
    origin: Vec3,
    params: &PrimitiveParams,
) -> Aabb {
    let axis = params.axis.unwrap_or([0.0, 0.0, 1.0]);

    match ptype {
        // ---------------------------------------------------------------
        // CYLINDER: centre of one face + axis + radius + length
        // ---------------------------------------------------------------
        PrimitiveType::Cylinder => {
            let r = params.radius.expect("Cylinder requires radius");
            let len = params.height.or(params.length).expect("Cylinder requires height/length");
            cylinder_aabb_inner(origin, &axis, r, len)
        }

        // ---------------------------------------------------------------
        // BOX: centre + xLen/yLen/zLen  (conservative: axis-aligned
        //      bounding sphere of the half-diagonal)
        // ---------------------------------------------------------------
        PrimitiveType::Box => {
            let xl = params.length.expect("Box requires length");
            let yl = params.width.expect("Box requires width");
            let zl = params.depth.or(params.height).expect("Box requires depth/height");
            // Conservative: bounding sphere of the box
            let half_diag = (xl * xl + yl * yl + zl * zl).sqrt() * 0.5;
            Aabb::new(
                Vec3::new(origin.x - half_diag, origin.y - half_diag, origin.z - half_diag),
                Vec3::new(origin.x + half_diag, origin.y + half_diag, origin.z + half_diag),
            )
        }

        // ---------------------------------------------------------------
        // SPHERE: centre + radius
        // ---------------------------------------------------------------
        PrimitiveType::Sphere => {
            let r = params.radius.expect("Sphere requires radius");
            Aabb::new(
                Vec3::new(origin.x - r, origin.y - r, origin.z - r),
                Vec3::new(origin.x + r, origin.y + r, origin.z + r),
            )
        }

        // ---------------------------------------------------------------
        // CONE (frustum): two end discs of radii r1 and r2
        // ---------------------------------------------------------------
        PrimitiveType::Cone => {
            let r1 = params.radius.expect("Cone requires radius (r1)");
            let r2 = params.radius2.expect("Cone requires radius2 (r2)");
            let len = params.height.or(params.length).expect("Cone requires height/length");
            let end = Vec3::new(
                origin.x + axis[0] * len,
                origin.y + axis[1] * len,
                origin.z + axis[2] * len,
            );
            let ext1 = disc_extent(&axis, r1);
            let ext2 = disc_extent(&axis, r2);
            Aabb::new(
                Vec3::new(
                    (origin.x - ext1.x).min(end.x - ext2.x),
                    (origin.y - ext1.y).min(end.y - ext2.y),
                    (origin.z - ext1.z).min(end.z - ext2.z),
                ),
                Vec3::new(
                    (origin.x + ext1.x).max(end.x + ext2.x),
                    (origin.y + ext1.y).max(end.y + ext2.y),
                    (origin.z + ext1.z).max(end.z + ext2.z),
                ),
            )
        }

        // ---------------------------------------------------------------
        // TORUS: full torus — bounding box of the enclosing sphere
        //        with radius = torus_radius + tube_radius
        // ---------------------------------------------------------------
        PrimitiveType::Torus => {
            let big_r = params.torus_radius.expect("Torus requires torus_radius");
            let small_r = params.tube_radius.expect("Torus requires tube_radius");
            // The torus lies in the plane perpendicular to `axis`.
            // Conservative: bounding sphere of outer radius.
            let outer = big_r + small_r;
            Aabb::new(
                Vec3::new(origin.x - outer, origin.y - outer, origin.z - outer),
                Vec3::new(origin.x + outer, origin.y + outer, origin.z + outer),
            )
        }

        // ---------------------------------------------------------------
        // DISH: vessel dished head — conservative cylinder of
        //       shellRadius x depth
        // ---------------------------------------------------------------
        PrimitiveType::Dish => {
            let shell_r = params.radius.expect("Dish requires radius (shellRadius)");
            let d = params.depth.or(params.height).expect("Dish requires depth");
            // Conservative: the dish fits within a cylinder of radius
            // shell_r and axial depth d, starting at origin along axis.
            cylinder_aabb_inner(origin, &axis, shell_r, d)
        }

        // ---------------------------------------------------------------
        // NOZZLE: barrel cylinder + flange disc
        // ---------------------------------------------------------------
        PrimitiveType::Nozzle => {
            let proj = params.length.or(params.height).expect("Nozzle requires length (projection)");
            let barrel_r = params.radius.unwrap_or(0.0);
            let flange_od = params.flange_od.unwrap_or(barrel_r * 2.0);
            let flange_r = flange_od * 0.5;
            let flange_t = params.flange_thickness.unwrap_or(0.0);
            let r = barrel_r.max(flange_r);
            let total_len = proj + flange_t;
            cylinder_aabb_inner(origin, &axis, r, total_len)
        }

        // ---------------------------------------------------------------
        // PYRAMID: conservative — bounding box of all 8 corners
        //          projected via the axis
        // ---------------------------------------------------------------
        PrimitiveType::Pyramid => {
            let bl = params.base_length.or(params.length).expect("Pyramid requires base_length");
            let bw = params.base_width.or(params.width).expect("Pyramid requires base_width");
            let tl = params.top_length.unwrap_or(0.0);
            let tw = params.top_width.unwrap_or(0.0);
            let h = params.height.or(params.depth).expect("Pyramid requires height");
            // Conservative: enclosing cylinder of radius = half diagonal
            // of the larger base, height = h.
            let max_half = ((bl.max(tl)).powi(2) + (bw.max(tw)).powi(2)).sqrt() * 0.5;
            cylinder_aabb_inner(origin, &axis, max_half, h)
        }

        // ---------------------------------------------------------------
        // SNOUT: eccentric frustum — like cone but with lateral offset
        // ---------------------------------------------------------------
        PrimitiveType::Snout => {
            let r1 = params.radius.expect("Snout requires radius (r1)");
            let r2 = params.radius2.expect("Snout requires radius2 (r2)");
            let len = params.height.or(params.length).expect("Snout requires height/length");
            let ox = params.offset_x.unwrap_or(0.0);
            let oy = params.offset_y.unwrap_or(0.0);
            let offset_mag = (ox * ox + oy * oy).sqrt();
            // Conservative: treat as a cone with the top radius expanded
            // by the offset magnitude.
            let effective_r2 = r2 + offset_mag;
            let end = Vec3::new(
                origin.x + axis[0] * len,
                origin.y + axis[1] * len,
                origin.z + axis[2] * len,
            );
            let ext1 = disc_extent(&axis, r1);
            let ext2 = disc_extent(&axis, effective_r2);
            Aabb::new(
                Vec3::new(
                    (origin.x - ext1.x).min(end.x - ext2.x),
                    (origin.y - ext1.y).min(end.y - ext2.y),
                    (origin.z - ext1.z).min(end.z - ext2.z),
                ),
                Vec3::new(
                    (origin.x + ext1.x).max(end.x + ext2.x),
                    (origin.y + ext1.y).max(end.y + ext2.y),
                    (origin.z + ext1.z).max(end.z + ext2.z),
                ),
            )
        }

        // ---------------------------------------------------------------
        // CIRCULAR TORUS: pipe elbow — conservative bounding sphere
        // ---------------------------------------------------------------
        PrimitiveType::CircularTorus => {
            let big_r = params.torus_radius.expect("CircularTorus requires torus_radius");
            let small_r = params.tube_radius.expect("CircularTorus requires tube_radius");
            // Conservative: full torus bounding sphere regardless of angle.
            let outer = big_r + small_r;
            Aabb::new(
                Vec3::new(origin.x - outer, origin.y - outer, origin.z - outer),
                Vec3::new(origin.x + outer, origin.y + outer, origin.z + outer),
            )
        }

        // ---------------------------------------------------------------
        // RECTANGULAR TORUS: duct elbow — conservative bounding sphere
        // ---------------------------------------------------------------
        PrimitiveType::RectangularTorus => {
            let big_r = params.torus_radius.expect("RectangularTorus requires torus_radius");
            let w = params.width.unwrap_or(0.0);
            let h = params.height.or(params.depth).unwrap_or(0.0);
            let half_diag = (w * w + h * h).sqrt() * 0.5;
            let outer = big_r + half_diag;
            Aabb::new(
                Vec3::new(origin.x - outer, origin.y - outer, origin.z - outer),
                Vec3::new(origin.x + outer, origin.y + outer, origin.z + outer),
            )
        }

        // ---------------------------------------------------------------
        // ELLIPTICAL HEAD: 2:1 elliptical — cylinder of shellRadius x
        //                  (shellRadius / 2)
        // ---------------------------------------------------------------
        PrimitiveType::EllipticalHead => {
            let shell_r = params.radius.expect("EllipticalHead requires radius");
            let d = params.depth.unwrap_or(shell_r * 0.5);
            cylinder_aabb_inner(origin, &axis, shell_r, d)
        }

        // ---------------------------------------------------------------
        // FLAT CAP: thin disc — cylinder of radius x thickness
        // ---------------------------------------------------------------
        PrimitiveType::FlatCap => {
            let r = params.radius.expect("FlatCap requires radius");
            let t = params.depth.or(params.height).unwrap_or(1.0);
            cylinder_aabb_inner(origin, &axis, r, t)
        }

        // ---------------------------------------------------------------
        // EXTRUSION: linear extrusion of a 2D profile. Conservative:
        //            cylinder enclosing the profile swept along the axis.
        // ---------------------------------------------------------------
        PrimitiveType::Extrusion => {
            let len = params.length.or(params.height).expect("Extrusion requires length");
            let profile_r = params.profile_max_r.or(params.radius).unwrap_or(0.0);
            cylinder_aabb_inner(origin, &axis, profile_r, len)
        }

        // ---------------------------------------------------------------
        // REVOLUTION: surface of revolution. Conservative: cylinder
        //             enclosing the max radial extent.
        // ---------------------------------------------------------------
        PrimitiveType::Revolution => {
            let max_r = params.profile_max_r.or(params.radius).expect("Revolution requires profile_max_r or radius");
            let max_z = params.profile_max_z.unwrap_or(0.0);
            let min_z = params.profile_min_z.unwrap_or(0.0);
            let total_z = max_z - min_z;
            // Origin is the axis point; the profile extends from min_z to max_z
            // along the axis.
            let start = Vec3::new(
                origin.x + axis[0] * min_z,
                origin.y + axis[1] * min_z,
                origin.z + axis[2] * min_z,
            );
            cylinder_aabb_inner(start, &axis, max_r, total_z)
        }
    }
}

/// Legacy convenience wrapper: compute an AABB for a cylinder from explicit
/// parameters (kept for backward compatibility).
pub fn cylinder_aabb(center: Vec3, axis: [f64; 3], radius: f64, length: f64) -> Aabb {
    cylinder_aabb_inner(center, &axis, radius, length)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    // -- Vec3 tests --------------------------------------------------------

    #[test]
    fn vec3_length() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert!(approx_eq(v.length(), 5.0));
    }

    #[test]
    fn vec3_dot() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert!(approx_eq(a.dot(&b), 32.0));
    }

    #[test]
    fn vec3_cross() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        let z = x.cross(&y);
        assert!(approx_eq(z.x, 0.0));
        assert!(approx_eq(z.y, 0.0));
        assert!(approx_eq(z.z, 1.0));
    }

    #[test]
    fn vec3_normalize() {
        let v = Vec3::new(0.0, 3.0, 4.0);
        let n = v.normalize();
        assert!(approx_eq(n.length(), 1.0));
        assert!(approx_eq(n.y, 0.6));
        assert!(approx_eq(n.z, 0.8));
    }

    // -- Aabb tests --------------------------------------------------------

    #[test]
    fn aabb_union_and_contains() {
        let a = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let b = Aabb::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(3.0, 3.0, 3.0));
        let u = a.union(&b);
        assert!(u.contains_point(Vec3::new(0.5, 0.5, 0.5)));
        assert!(u.contains_point(Vec3::new(2.5, 2.5, 2.5)));
        assert!(!u.contains_point(Vec3::new(-1.0, 0.0, 0.0)));
    }

    #[test]
    fn aabb_center_extents_volume() {
        let bb = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 6.0, 8.0));
        let c = bb.center();
        assert!(approx_eq(c.x, 2.0));
        assert!(approx_eq(c.y, 3.0));
        assert!(approx_eq(c.z, 4.0));
        let e = bb.extents();
        assert!(approx_eq(e.x, 2.0));
        assert!(approx_eq(e.y, 3.0));
        assert!(approx_eq(e.z, 4.0));
        assert!(approx_eq(bb.volume(), 192.0));
    }

    // -- Cylinder AABB -----------------------------------------------------

    #[test]
    fn cylinder_aabb_z_axis() {
        let origin = Vec3::new(100.0, 200.0, 300.0);
        let params = PrimitiveParams {
            radius: Some(50.0),
            height: Some(200.0),
            axis: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        };
        let bb = primitive_aabb(&PrimitiveType::Cylinder, origin, &params);

        // Cylinder along Z: radius expands X and Y, length extends Z.
        assert!(approx_eq(bb.min.x, 50.0));
        assert!(approx_eq(bb.max.x, 150.0));
        assert!(approx_eq(bb.min.y, 150.0));
        assert!(approx_eq(bb.max.y, 250.0));
        assert!(approx_eq(bb.min.z, 300.0));
        assert!(approx_eq(bb.max.z, 500.0));
    }

    #[test]
    fn cylinder_aabb_x_axis() {
        let origin = Vec3::zero();
        let params = PrimitiveParams {
            radius: Some(10.0),
            height: Some(100.0),
            axis: Some([1.0, 0.0, 0.0]),
            ..Default::default()
        };
        let bb = primitive_aabb(&PrimitiveType::Cylinder, origin, &params);
        assert!(approx_eq(bb.min.x, 0.0));
        assert!(approx_eq(bb.max.x, 100.0));
        assert!(approx_eq(bb.min.y, -10.0));
        assert!(approx_eq(bb.max.y, 10.0));
        assert!(approx_eq(bb.min.z, -10.0));
        assert!(approx_eq(bb.max.z, 10.0));
    }

    // -- Box AABB ----------------------------------------------------------

    #[test]
    fn box_aabb_contains_corners() {
        let origin = Vec3::new(0.0, 0.0, 0.0);
        let params = PrimitiveParams {
            length: Some(4.0),
            width: Some(6.0),
            depth: Some(8.0),
            ..Default::default()
        };
        let bb = primitive_aabb(&PrimitiveType::Box, origin, &params);

        // All 8 corners of the box must be inside the AABB.
        for &sx in &[-1.0, 1.0] {
            for &sy in &[-1.0, 1.0] {
                for &sz in &[-1.0, 1.0] {
                    let corner = Vec3::new(sx * 2.0, sy * 3.0, sz * 4.0);
                    assert!(
                        bb.contains_point(corner),
                        "Corner {:?} not in AABB {:?}",
                        corner,
                        bb
                    );
                }
            }
        }
    }

    // -- Sphere AABB -------------------------------------------------------

    #[test]
    fn sphere_aabb() {
        let origin = Vec3::new(10.0, 20.0, 30.0);
        let params = PrimitiveParams {
            radius: Some(5.0),
            ..Default::default()
        };
        let bb = primitive_aabb(&PrimitiveType::Sphere, origin, &params);
        assert!(approx_eq(bb.min.x, 5.0));
        assert!(approx_eq(bb.max.x, 15.0));
        assert!(approx_eq(bb.min.y, 15.0));
        assert!(approx_eq(bb.max.y, 25.0));
        assert!(approx_eq(bb.min.z, 25.0));
        assert!(approx_eq(bb.max.z, 35.0));
        assert!(approx_eq(bb.volume(), 1000.0)); // 10^3
    }

    // -- Cone AABB ---------------------------------------------------------

    #[test]
    fn cone_aabb_z_axis() {
        let origin = Vec3::zero();
        let params = PrimitiveParams {
            radius: Some(50.0),
            radius2: Some(25.0),
            height: Some(100.0),
            axis: Some([0.0, 0.0, 1.0]),
            ..Default::default()
        };
        let bb = primitive_aabb(&PrimitiveType::Cone, origin, &params);
        // Bottom disc r=50, top disc r=25 at z=100
        assert!(approx_eq(bb.min.x, -50.0));
        assert!(approx_eq(bb.max.x, 50.0));
        assert!(approx_eq(bb.min.z, 0.0));
        assert!(approx_eq(bb.max.z, 100.0));
    }

    // -- Torus AABB --------------------------------------------------------

    #[test]
    fn torus_aabb() {
        let origin = Vec3::new(0.0, 0.0, 0.0);
        let params = PrimitiveParams {
            torus_radius: Some(100.0),
            tube_radius: Some(20.0),
            ..Default::default()
        };
        let bb = primitive_aabb(&PrimitiveType::Torus, origin, &params);
        let outer = 120.0;
        assert!(approx_eq(bb.min.x, -outer));
        assert!(approx_eq(bb.max.x, outer));
        assert!(approx_eq(bb.min.y, -outer));
        assert!(approx_eq(bb.max.y, outer));
    }

    // -- Legacy cylinder_aabb wrapper --------------------------------------

    #[test]
    fn legacy_cylinder_aabb() {
        let bb = cylinder_aabb(Vec3::zero(), [0.0, 0.0, 1.0], 10.0, 100.0);
        assert!(approx_eq(bb.min.x, -10.0));
        assert!(approx_eq(bb.max.z, 100.0));
    }

    // -- Intersects --------------------------------------------------------

    #[test]
    fn aabb_intersects() {
        let a = Aabb::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0));
        let b = Aabb::new(Vec3::new(1.0, 1.0, 1.0), Vec3::new(3.0, 3.0, 3.0));
        let c = Aabb::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0));
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }
}
