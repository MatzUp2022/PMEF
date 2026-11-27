//! PMEFX manifest, object index and spatial index types (spec 03 §3, RFC-0005).
//!
//! These types mirror `schemas/pmefx-manifest.schema.json` one to one; the round-trip and schema
//! tests in `tests/pmefx.rs` keep them in sync.

use serde::{Deserialize, Serialize};

/// Container version written by this implementation.
pub const PMEFX_VERSION: &str = "1.0";

/// Archive path of the manifest (spec 03 §3.3).
pub const MANIFEST_PATH: &str = "manifest.json";
/// Default archive path of the object index (spec 03 §3.5).
pub const OBJECT_INDEX_PATH: &str = "index/objects.ndjson";
/// Default archive path of the spatial index (spec 03 §3.7).
pub const SPATIAL_INDEX_PATH: &str = "index/octree.json";

/// `manifest.json` — the package table of contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Container version (`"1.0"`). Absent in 0.9 manifests, which carry no object index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pmefx_version: Option<String>,
    /// PMEF data version of the NDJSON files, e.g. `"0.9.0"`.
    pub pmef_version: String,
    /// `@id` of the main file's `pmef:FileHeader`.
    pub package_id: String,
    /// Archive path of the primary NDJSON file.
    pub main_file: String,
    /// `"Z-up"` (PMEF default) or `"Y-up"`.
    pub coordinate_system: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// Every archive entry except `manifest.json`.
    pub files: Vec<FileEntry>,
    /// Origin of all mesh assets in project coordinates [mm] (spec 03 §3.6). Default: 0,0,0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_origin: Option<Point3>,
    /// Archive path of the object index (NDJSON, one [`ObjectEntry`] per line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_index: Option<String>,
    /// Archive path of the spatial index ([`Octree`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_index: Option<String>,
    /// Number of lines in the object index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_count: Option<u64>,
    /// Software that wrote the package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<Generator>,
}

/// One archive entry listed in the manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: FileKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Lower-case hex SHA-256 of the uncompressed entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Number of PMEF objects (NDJSON files only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<u64>,
}

/// Archive entry type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Ndjson,
    Gltf,
    Step,
    Usd,
    Catalog,
    Document,
    Index,
    Other,
}

impl FileKind {
    /// Binary geometry assets are stored uncompressed so that readers can range-read or
    /// memory-map them (spec 03 §3.2 rule 4).
    pub fn must_be_stored(self) -> bool {
        matches!(self, FileKind::Gltf | FileKind::Step | FileKind::Usd)
    }
}

/// Writing software.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Generator {
    pub name: String,
    pub version: String,
}

/// A point in project coordinates [mm].
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Axis-aligned bounding box in project coordinates [mm] — same shape as
/// `GeometryReference.boundingBox` in `pmef-base`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BBox {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub z_min: f64,
    pub z_max: f64,
}

impl BBox {
    pub fn union(&self, o: &BBox) -> BBox {
        BBox {
            x_min: self.x_min.min(o.x_min),
            x_max: self.x_max.max(o.x_max),
            y_min: self.y_min.min(o.y_min),
            y_max: self.y_max.max(o.y_max),
            z_min: self.z_min.min(o.z_min),
            z_max: self.z_max.max(o.z_max),
        }
    }

    pub fn contains(&self, o: &BBox) -> bool {
        o.x_min >= self.x_min
            && o.x_max <= self.x_max
            && o.y_min >= self.y_min
            && o.y_max <= self.y_max
            && o.z_min >= self.z_min
            && o.z_max <= self.z_max
    }

    pub fn center(&self) -> Point3 {
        Point3 {
            x: (self.x_min + self.x_max) / 2.0,
            y: (self.y_min + self.y_max) / 2.0,
            z: (self.z_min + self.z_max) / 2.0,
        }
    }

    /// True if all values are finite and min <= max on every axis.
    pub fn is_valid(&self) -> bool {
        [self.x_min, self.x_max, self.y_min, self.y_max, self.z_min, self.z_max]
            .iter()
            .all(|v| v.is_finite())
            && self.x_min <= self.x_max
            && self.y_min <= self.y_max
            && self.z_min <= self.z_max
    }
}

/// One line of the object index: everything a viewer needs before loading any geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectEntry {
    /// PMEF `@id` — the stable key that links all representations of one object.
    pub id: String,
    /// PMEF `@type`.
    #[serde(rename = "type")]
    pub type_str: String,
    /// Where the object's attributes (the NDJSON record) live.
    pub record: RecordRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<BBox>,
    /// Mesh levels of detail, ordered coarse → fine.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lods: Vec<LodRef>,
    /// Exact boundary representation, loaded on demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brep: Option<BrepRef>,
}

/// Location of a PMEF object in an NDJSON file of the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRef {
    pub path: String,
    /// 1-based physical line number.
    pub line: u64,
}

/// Mesh level of detail (same enum as `GeometryReference.lod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Lod {
    #[serde(rename = "BBOX_ONLY")]
    BboxOnly,
    #[serde(rename = "LOD1_COARSE")]
    Lod1Coarse,
    #[serde(rename = "LOD2_MEDIUM")]
    Lod2Medium,
    #[serde(rename = "LOD3_FINE")]
    Lod3Fine,
    #[serde(rename = "LOD4_FABRICATION")]
    Lod4Fabrication,
}

impl Lod {
    pub fn as_str(self) -> &'static str {
        match self {
            Lod::BboxOnly => "BBOX_ONLY",
            Lod::Lod1Coarse => "LOD1_COARSE",
            Lod::Lod2Medium => "LOD2_MEDIUM",
            Lod::Lod3Fine => "LOD3_FINE",
            Lod::Lod4Fabrication => "LOD4_FABRICATION",
        }
    }
}

/// One mesh level of detail of an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LodRef {
    pub lod: Lod,
    /// Archive path of a glTF 2.0 binary (`.glb`).
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triangles: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_count: Option<u64>,
    /// Largest geometric deviation from the finest representation [mm] — drives
    /// screen-space-error LOD selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Shift of this mesh against `manifest.meshOrigin` [mm] (RFC-0017): the mesh's vertices
    /// are relative to `meshOrigin + offset`. Lets several objects share one mesh file
    /// (the same fitting at many places). Absent = no shift.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<Point3>,
}

impl LodRef {
    /// Origin to read this level's mesh with ([`super::glb::read_mesh_glb`]):
    /// the package's `meshOrigin` plus the level's `offset`.
    pub fn mesh_origin(&self, package_origin: Point3) -> Point3 {
        let o = self.offset.unwrap_or_default();
        Point3 {
            x: package_origin.x + o.x,
            y: package_origin.y + o.y,
            z: package_origin.z + o.z,
        }
    }
}

/// Exact geometry of an object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrepRef {
    pub path: String,
    /// `"step"` (ISO 10303 AP242) or a vendor extension `"x-…"`.
    pub format: String,
}

/// `index/octree.json` — loose octree over object bounding boxes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Octree {
    pub max_depth: u32,
    pub max_objects_per_node: u32,
    pub root: OctreeNode,
}

/// Octree node. `bbox` is loose: the union of all object boxes in the subtree, so it encloses
/// objects that overhang the node's cell. Every object appears in exactly one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OctreeNode {
    pub bbox: BBox,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<OctreeNode>,
}
