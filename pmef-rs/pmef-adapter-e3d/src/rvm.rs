//! RVM (PDMS Review Model) binary geometry parser.
//!
//! RVM is the binary geometry format exported by AVEVA PDMS and E3D.
//! This module parses RVM files and produces PMEF `GeometryReference` objects.
//!
//! ## RVM File Structure
//!
//! An RVM file is a binary stream with 4-byte aligned records:
//!
//! ```text
//! FILE header  ("HEAD" magic, version, date, info)
//! MODEL block
//!   GROUP block  (hierarchy node)
//!     GROUP ...
//!       PRIM block  (primitive: BOX, CYL, SNOUT, etc.)
//!         PRIM ...
//! ENDH footer
//! ```
//!
//! Each block starts with a 4-byte chunk type identifier (ASCII) followed by
//! a 4-byte chunk length (big-endian u32), then the chunk data.

use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek, SeekFrom};
use thiserror::Error;

/// RVM parsing errors.
#[derive(Debug, Error)]
pub enum RvmError {
    #[error("Invalid RVM magic bytes: expected 'HEAD', got '{0}'")]
    InvalidMagic(String),
    #[error("Unsupported RVM version: {0}")]
    UnsupportedVersion(u32),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unexpected chunk type '{0}' at offset {1}")]
    UnexpectedChunk(String, u64),
}

/// An axis-aligned bounding box from the RVM file [mm].
#[derive(Debug, Clone, PartialEq)]
pub struct RvmBbox {
    pub xmin: f32, pub xmax: f32,
    pub ymin: f32, pub ymax: f32,
    pub zmin: f32, pub zmax: f32,
}

impl RvmBbox {
    /// Volume of the bounding box [mm³].
    pub fn volume(&self) -> f32 {
        (self.xmax - self.xmin).max(0.0) *
        (self.ymax - self.ymin).max(0.0) *
        (self.zmax - self.zmin).max(0.0)
    }

    /// Centre point of the bounding box [mm].
    pub fn centre(&self) -> (f32, f32, f32) {
        (
            (self.xmin + self.xmax) / 2.0,
            (self.ymin + self.ymax) / 2.0,
            (self.zmin + self.zmax) / 2.0,
        )
    }
}

/// An RVM primitive geometry shape.
#[derive(Debug, Clone, PartialEq)]
pub enum RvmPrimitive {
    /// Rectangular box: [origin_x, origin_y, origin_z, x_len, y_len, z_len].
    Box    { origin: [f32;3], extents: [f32;3] },
    /// Cylinder: [origin_x, origin_y, origin_z, radius, height].
    Cyl    { origin: [f32;3], radius: f32, height: f32 },
    /// Sphere: [origin_x, origin_y, origin_z, radius].
    Sphere { origin: [f32;3], radius: f32 },
    /// Snout (eccentric frustum): two radii + height + offsets.
    Snout  { origin: [f32;3], bot_radius: f32, top_radius: f32, height: f32, off_x: f32, off_y: f32 },
    /// Circular torus (elbow): torus radius, tube radius, angle.
    CTorus { origin: [f32;3], offset: f32, radius: f32, angle: f32 },
    /// Rectangular torus (square-section torus): rarely used.
    RTorus { origin: [f32;3], offset: f32, radius: f32, angle: f32 },
    /// Pyramid (slope surfaces).
    Pyramid { origin: [f32;3], bot: [f32;2], top: [f32;2], height: f32, top_off: [f32;2] },
    /// Dish (ellipsoid cap for vessel heads).
    Dish { origin: [f32;3], radius: f32, height: f32 },
    /// Generic mesh (not commonly produced by E3D for standard components).
    Mesh { vertex_count: u32, triangle_count: u32, bbox: RvmBbox },
}

impl RvmPrimitive {
    /// Returns the primitive's bounding box [mm].
    pub fn bounding_box(&self) -> RvmBbox {
        match self {
            Self::Box { origin: [ox,oy,oz], extents: [xl,yl,zl] } => RvmBbox {
                xmin: *ox, xmax: ox + xl,
                ymin: *oy, ymax: oy + yl,
                zmin: *oz, zmax: oz + zl,
            },
            Self::Cyl { origin: [ox,oy,oz], radius: r, height: h } => RvmBbox {
                xmin: ox - r, xmax: ox + r,
                ymin: oy - r, ymax: oy + r,
                zmin: *oz,   zmax: oz + h,
            },
            Self::Sphere { origin: [ox,oy,oz], radius: r } => RvmBbox {
                xmin: ox - r, xmax: ox + r,
                ymin: oy - r, ymax: oy + r,
                zmin: oz - r, zmax: oz + r,
            },
            Self::Snout { origin: [ox,oy,oz], bot_radius: br, top_radius: tr, height: h, off_x, off_y } => {
                let rmax = br.max(*tr);
                RvmBbox {
                    xmin: ox - rmax - off_x.abs(), xmax: ox + rmax + off_x.abs(),
                    ymin: oy - rmax - off_y.abs(), ymax: oy + rmax + off_y.abs(),
                    zmin: *oz, zmax: oz + h,
                }
            },
            Self::CTorus { origin: [ox,oy,oz], offset: off, radius: r, .. } => {
                let total = off + r;
                RvmBbox {
                    xmin: ox - total, xmax: ox + total,
                    ymin: oy - total, ymax: oy + total,
                    zmin: oz - total, zmax: oz + total,
                }
            },
            Self::Dish { origin: [ox,oy,oz], radius: r, height: h } => RvmBbox {
                xmin: ox - r, xmax: ox + r,
                ymin: oy - r, ymax: oy + r,
                zmin: *oz, zmax: oz + h,
            },
            _ => RvmBbox { xmin:0., xmax:0., ymin:0., ymax:0., zmin:0., zmax:0. }
        }
    }

    /// Returns a PMEF primitive type name for this RVM primitive.
    pub fn pmef_primitive_type(&self) -> &'static str {
        match self {
            Self::Box    { .. } => "BOX",
            Self::Cyl    { .. } => "CYLINDER",
            Self::Sphere { .. } => "SPHERE",
            Self::Snout  { .. } => "SNOUT",
            Self::CTorus { .. } => "CIRC_TORUS",
            Self::RTorus { .. } => "CIRC_TORUS",
            Self::Pyramid{ .. } => "BOX",  // approximate
            Self::Dish   { .. } => "DISH",
            Self::Mesh   { .. } => "MESH_REF",
        }
    }
}

/// An RVM group — corresponds to an E3D element.
#[derive(Debug, Clone)]
pub struct RvmGroup {
    /// E3D database address embedded in the RVM file.
    pub name: String,
    /// 4×3 transformation matrix (row-major, last row implicit [0,0,0,1]).
    pub transform: [[f32; 3]; 4],
    /// Bounding box in the parent coordinate system.
    pub bbox: RvmBbox,
    /// Primitive shapes in this group.
    pub primitives: Vec<RvmPrimitive>,
    /// Child groups (hierarchy).
    pub children: Vec<RvmGroup>,
}

impl RvmGroup {
    /// Compute the combined bounding box of all primitives in this group
    /// (not transformed — in local coordinates).
    pub fn local_bbox(&self) -> Option<RvmBbox> {
        let bboxes: Vec<RvmBbox> = self.primitives.iter()
            .map(|p| p.bounding_box())
            .collect();
        if bboxes.is_empty() { return None; }
        Some(RvmBbox {
            xmin: bboxes.iter().map(|b| b.xmin).fold(f32::INFINITY,  f32::min),
            xmax: bboxes.iter().map(|b| b.xmax).fold(f32::NEG_INFINITY, f32::max),
            ymin: bboxes.iter().map(|b| b.ymin).fold(f32::INFINITY,  f32::min),
            ymax: bboxes.iter().map(|b| b.ymax).fold(f32::NEG_INFINITY, f32::max),
            zmin: bboxes.iter().map(|b| b.zmin).fold(f32::INFINITY,  f32::min),
            zmax: bboxes.iter().map(|b| b.zmax).fold(f32::NEG_INFINITY, f32::max),
        })
    }
}

/// Parsed RVM file contents.
#[derive(Debug)]
pub struct RvmFile {
    /// RVM format version.
    pub version: u32,
    /// Creation date string.
    pub date: String,
    /// Top-level group (MODEL).
    pub model: RvmGroup,
    /// Total number of primitives across all groups.
    pub primitive_count: u32,
}

impl RvmFile {
    /// Find a group by E3D name/address (substring match).
    pub fn find_group(&self, name: &str) -> Option<&RvmGroup> {
        find_group_recursive(&self.model, name)
    }
}

fn find_group_recursive<'a>(group: &'a RvmGroup, name: &str) -> Option<&'a RvmGroup> {
    if group.name.contains(name) {
        return Some(group);
    }
    for child in &group.children {
        if let Some(found) = find_group_recursive(child, name) {
            return Some(found);
        }
    }
    None
}

// ── Binary RVM parser ─────────────────────────────────────────────────────────

/// Parse an RVM binary file from a byte slice.
///
/// # RVM Binary Format (version 2)
///
/// The file is a sequence of 4-byte-aligned chunks:
/// - 4 bytes: chunk type (ASCII, e.g. "HEAD", "MODL", "CNTB", "PRIM", "ENDG", "ENDH")
/// - 4 bytes: chunk length (big-endian u32), in 4-byte units
/// - N bytes: chunk data
///
/// Primitives are identified by a 1-byte type code within the PRIM chunk:
/// - 1 = BOX, 2 = CYL, 3 = SPHERE, 4 = SNOUT, 5 = CTOR, 6 = RTOR,
/// - 7 = PYRA, 8 = CIRC, 9 = DISH, 10 = NSHELL (mesh)
pub fn parse_rvm_bytes(data: &[u8]) -> Result<RvmFile, RvmError> {
    let mut cursor = Cursor::new(data);

    // Read and validate FILE header
    let magic = read_string_4(&mut cursor)?;
    if magic != "HEAD" {
        return Err(RvmError::InvalidMagic(magic));
    }
    let _head_len = cursor.read_u32::<BigEndian>()?;
    let version = cursor.read_u32::<BigEndian>()?;
    if version > 2 {
        return Err(RvmError::UnsupportedVersion(version));
    }
    let _encoding = read_counted_string(&mut cursor)?;
    let _note     = read_counted_string(&mut cursor)?;
    let date      = read_counted_string(&mut cursor)?;
    let _user     = read_counted_string(&mut cursor)?;

    // Read MODEL block
    let model_type = read_string_4(&mut cursor)?;
    if model_type != "MODL" {
        return Err(RvmError::UnexpectedChunk(model_type, cursor.position()));
    }
    let _modl_len  = cursor.read_u32::<BigEndian>()?;
    let _proj_name = read_counted_string(&mut cursor)?;
    let _modl_name = read_counted_string(&mut cursor)?;

    // Parse group tree
    let mut primitive_count = 0u32;
    let model = parse_group(&mut cursor, &mut primitive_count)?;

    Ok(RvmFile { version, date, model, primitive_count })
}

/// Parse a CNTB (container/group) block recursively.
fn parse_group(cursor: &mut Cursor<&[u8]>, prim_count: &mut u32) -> Result<RvmGroup, RvmError> {
    // CNTB header
    let chunk = read_string_4(cursor)?;
    if chunk != "CNTB" {
        return Err(RvmError::UnexpectedChunk(chunk, cursor.position()));
    }
    let _len = cursor.read_u32::<BigEndian>()?;

    // Version, transform matrix (4×3 floats), bounding box (6 floats), name
    let _version = cursor.read_u32::<BigEndian>()?;
    let transform = read_transform(cursor)?;
    let bbox      = read_bbox(cursor)?;
    let name      = read_counted_string(cursor)?;

    let mut group = RvmGroup {
        name, transform, bbox,
        primitives: Vec::new(),
        children: Vec::new(),
    };

    // Read children until ENDG
    loop {
        let child_chunk = read_string_4(cursor)?;
        match child_chunk.as_str() {
            "CNTB" => {
                // Back up 4 bytes and recurse
                cursor.seek(SeekFrom::Current(-4))?;
                group.children.push(parse_group(cursor, prim_count)?);
            }
            "PRIM" => {
                let prim = parse_primitive(cursor)?;
                group.primitives.push(prim);
                *prim_count += 1;
            }
            "ENDG" => {
                let _len = cursor.read_u32::<BigEndian>()?;
                break;
            }
            other => {
                // Skip unknown chunk
                let len = cursor.read_u32::<BigEndian>()?;
                cursor.seek(SeekFrom::Current(len as i64 * 4))?;
                tracing::debug!("Skipping unknown RVM chunk '{}' ({} words)", other, len);
            }
        }
    }

    Ok(group)
}

/// Parse a PRIM chunk.
fn parse_primitive(cursor: &mut Cursor<&[u8]>) -> Result<RvmPrimitive, RvmError> {
    let len  = cursor.read_u32::<BigEndian>()?;
    let _ver = cursor.read_u32::<BigEndian>()?;
    let kind = cursor.read_u32::<BigEndian>()?; // primitive type code

    let prim = match kind {
        1 => { // BOX: ox, oy, oz, xl, yl, zl
            let ox = cursor.read_f32::<BigEndian>()?;
            let oy = cursor.read_f32::<BigEndian>()?;
            let oz = cursor.read_f32::<BigEndian>()?;
            let xl = cursor.read_f32::<BigEndian>()?;
            let yl = cursor.read_f32::<BigEndian>()?;
            let zl = cursor.read_f32::<BigEndian>()?;
            RvmPrimitive::Box { origin: [ox, oy, oz], extents: [xl, yl, zl] }
        }
        2 => { // CYL: ox, oy, oz, radius, height
            let ox = cursor.read_f32::<BigEndian>()?;
            let oy = cursor.read_f32::<BigEndian>()?;
            let oz = cursor.read_f32::<BigEndian>()?;
            let r  = cursor.read_f32::<BigEndian>()?;
            let h  = cursor.read_f32::<BigEndian>()?;
            RvmPrimitive::Cyl { origin: [ox, oy, oz], radius: r, height: h }
        }
        3 => { // SPHERE: ox, oy, oz, radius
            let ox = cursor.read_f32::<BigEndian>()?;
            let oy = cursor.read_f32::<BigEndian>()?;
            let oz = cursor.read_f32::<BigEndian>()?;
            let r  = cursor.read_f32::<BigEndian>()?;
            RvmPrimitive::Sphere { origin: [ox, oy, oz], radius: r }
        }
        4 => { // SNOUT: ox, oy, oz, bot_r, top_r, height, off_x, off_y
            let ox = cursor.read_f32::<BigEndian>()?;
            let oy = cursor.read_f32::<BigEndian>()?;
            let oz = cursor.read_f32::<BigEndian>()?;
            let br = cursor.read_f32::<BigEndian>()?;
            let tr = cursor.read_f32::<BigEndian>()?;
            let h  = cursor.read_f32::<BigEndian>()?;
            let ox2= cursor.read_f32::<BigEndian>()?;
            let oy2= cursor.read_f32::<BigEndian>()?;
            RvmPrimitive::Snout { origin: [ox, oy, oz], bot_radius: br, top_radius: tr,
                                   height: h, off_x: ox2, off_y: oy2 }
        }
        5 => { // CTOR (circular torus): ox, oy, oz, offset, radius, angle
            let ox  = cursor.read_f32::<BigEndian>()?;
            let oy  = cursor.read_f32::<BigEndian>()?;
            let oz  = cursor.read_f32::<BigEndian>()?;
            let off = cursor.read_f32::<BigEndian>()?;
            let r   = cursor.read_f32::<BigEndian>()?;
            let ang = cursor.read_f32::<BigEndian>()?;
            RvmPrimitive::CTorus { origin: [ox, oy, oz], offset: off, radius: r, angle: ang }
        }
        6 => { // RTOR (rectangular torus)
            let ox  = cursor.read_f32::<BigEndian>()?;
            let oy  = cursor.read_f32::<BigEndian>()?;
            let oz  = cursor.read_f32::<BigEndian>()?;
            let off = cursor.read_f32::<BigEndian>()?;
            let r   = cursor.read_f32::<BigEndian>()?;
            let ang = cursor.read_f32::<BigEndian>()?;
            RvmPrimitive::RTorus { origin: [ox, oy, oz], offset: off, radius: r, angle: ang }
        }
        8 => { // DISH: ox, oy, oz, radius, height
            let ox = cursor.read_f32::<BigEndian>()?;
            let oy = cursor.read_f32::<BigEndian>()?;
            let oz = cursor.read_f32::<BigEndian>()?;
            let r  = cursor.read_f32::<BigEndian>()?;
            let h  = cursor.read_f32::<BigEndian>()?;
            RvmPrimitive::Dish { origin: [ox, oy, oz], radius: r, height: h }
        }
        _ => {
            // Skip unknown primitive body
            let remaining = (len as i64 - 3) * 4; // -3 for version+kind already read
            if remaining > 0 {
                cursor.seek(SeekFrom::Current(remaining))?;
            }
            RvmPrimitive::Box { origin: [0.,0.,0.], extents: [1.,1.,1.] } // placeholder
        }
    };
    Ok(prim)
}

/// Read 4 ASCII bytes as a chunk type string.
fn read_string_4(cursor: &mut Cursor<&[u8]>) -> Result<String, RvmError> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// Read a length-prefixed string (u32 length, then bytes, padded to 4-byte boundary).
fn read_counted_string(cursor: &mut Cursor<&[u8]>) -> Result<String, RvmError> {
    let len = cursor.read_u32::<BigEndian>()? as usize;
    if len == 0 { return Ok(String::new()); }
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf)?;
    // Pad to 4-byte boundary
    let pad = (4 - len % 4) % 4;
    if pad > 0 { cursor.seek(SeekFrom::Current(pad as i64))?; }
    Ok(String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string())
}

/// Read a 4×3 transformation matrix (12 f32 values).
fn read_transform(cursor: &mut Cursor<&[u8]>) -> Result<[[f32; 3]; 4], RvmError> {
    let mut m = [[0f32; 3]; 4];
    for row in &mut m {
        for col in row {
            *col = cursor.read_f32::<BigEndian>()?;
        }
    }
    Ok(m)
}

/// Read an axis-aligned bounding box (6 f32 values: xmin, xmax, ymin, ymax, zmin, zmax).
fn read_bbox(cursor: &mut Cursor<&[u8]>) -> Result<RvmBbox, RvmError> {
    Ok(RvmBbox {
        xmin: cursor.read_f32::<BigEndian>()?,
        xmax: cursor.read_f32::<BigEndian>()?,
        ymin: cursor.read_f32::<BigEndian>()?,
        ymax: cursor.read_f32::<BigEndian>()?,
        zmin: cursor.read_f32::<BigEndian>()?,
        zmax: cursor.read_f32::<BigEndian>()?,
    })
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rvm_bbox_volume() {
        let bbox = RvmBbox { xmin:0., xmax:100., ymin:0., ymax:200., zmin:0., zmax:50. };
        assert!((bbox.volume() - 1_000_000.0).abs() < 1.0);
    }

    #[test]
    fn test_rvm_bbox_centre() {
        let bbox = RvmBbox { xmin:0., xmax:100., ymin:0., ymax:200., zmin:0., zmax:50. };
        let (cx, cy, cz) = bbox.centre();
        assert!((cx - 50.).abs() < 0.01);
        assert!((cy - 100.).abs() < 0.01);
        assert!((cz - 25.).abs() < 0.01);
    }

    #[test]
    fn test_cyl_bounding_box() {
        let cyl = RvmPrimitive::Cyl { origin: [0.,0.,0.], radius: 109.55, height: 2500. };
        let bbox = cyl.bounding_box();
        assert!((bbox.xmin - (-109.55)).abs() < 0.01);
        assert!((bbox.xmax - 109.55).abs() < 0.01);
        assert!((bbox.zmax - 2500.).abs() < 0.01);
    }

    #[test]
    fn test_sphere_bounding_box() {
        let s = RvmPrimitive::Sphere { origin: [100.,200.,300.], radius: 50. };
        let bbox = s.bounding_box();
        assert!((bbox.xmin - 50.).abs() < 0.01);
        assert!((bbox.xmax - 150.).abs() < 0.01);
    }

    #[test]
    fn test_pmef_primitive_types() {
        assert_eq!(RvmPrimitive::Box { origin:[0.,0.,0.], extents:[1.,1.,1.] }.pmef_primitive_type(), "BOX");
        assert_eq!(RvmPrimitive::Cyl { origin:[0.,0.,0.], radius:1., height:1. }.pmef_primitive_type(), "CYLINDER");
        assert_eq!(RvmPrimitive::CTorus { origin:[0.,0.,0.], offset:1., radius:1., angle:90. }.pmef_primitive_type(), "CIRC_TORUS");
    }

    #[test]
    fn test_parse_rvm_invalid_magic() {
        let bad = b"XXXX\x00\x00\x00\x00";
        let result = parse_rvm_bytes(bad);
        assert!(matches!(result, Err(RvmError::InvalidMagic(_))));
    }

    #[test]
    fn test_rvm_group_local_bbox_empty() {
        let g = RvmGroup {
            name: "test".to_owned(),
            transform: [[0.;3];4],
            bbox: RvmBbox { xmin:0.,xmax:0.,ymin:0.,ymax:0.,zmin:0.,zmax:0. },
            primitives: vec![],
            children: vec![],
        };
        assert!(g.local_bbox().is_none());
    }

    #[test]
    fn test_find_group() {
        let child = RvmGroup {
            name: "/SITE01/ZONE-CW/PIPE-CW-201".to_owned(),
            transform: [[0.;3];4],
            bbox: RvmBbox { xmin:0.,xmax:100.,ymin:0.,ymax:100.,zmin:0.,zmax:100. },
            primitives: vec![RvmPrimitive::Cyl { origin:[0.,0.,0.], radius:50., height:100. }],
            children: vec![],
        };
        let root = RvmGroup {
            name: "MODEL".to_owned(),
            transform: [[0.;3];4],
            bbox: RvmBbox { xmin:0.,xmax:0.,ymin:0.,ymax:0.,zmin:0.,zmax:0. },
            primitives: vec![],
            children: vec![child],
        };
        let file = RvmFile { version: 2, date: "2026-03-31".to_owned(),
                             model: root, primitive_count: 1 };
        let found = file.find_group("PIPE-CW-201");
        assert!(found.is_some());
        assert_eq!(found.unwrap().primitives.len(), 1);
    }
}
