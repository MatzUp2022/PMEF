//! Minimal glTF 2.0 binary (`.glb`) writer and reader for PMEFX levels of detail: box meshes for
//! `BBOX_ONLY`, triangle meshes ([`mesh_glb`]) for the finer levels, and [`read_mesh_glb`] for viewers.
//!
//! Coordinate mapping (spec 03 §3.6): PMEF project coordinates are Z-up millimetres, glTF is
//! Y-up metres. A project point `p` becomes `((p.x-o.x)/1000, (p.z-o.z)/1000, -(p.y-o.y)/1000)`
//! with `o` = `manifest.meshOrigin`.

use super::model::{BBox, Point3};
use serde_json::json;

/// Maps a project point [mm, Z-up] to glTF space [m, Y-up] relative to `origin`.
pub fn to_gltf(p: [f64; 3], origin: Point3) -> [f32; 3] {
    [
        ((p[0] - origin.x) / 1000.0) as f32,
        ((p[2] - origin.z) / 1000.0) as f32,
        (-(p[1] - origin.y) / 1000.0) as f32,
    ]
}

/// A box mesh for `bbox` (24 vertices with flat normals, 12 triangles) as `.glb` bytes.
///
/// The single node is named after the PMEF `@id` and carries it in `extras.pmefId`, so viewers
/// can map picks back to the object.
#[allow(clippy::manual_is_multiple_of)] // MSRV 1.75
pub fn box_glb(id: &str, bbox: &BBox, origin: Point3) -> Vec<u8> {
    let (x0, x1, y0, y1, z0, z1) = (bbox.x_min, bbox.x_max, bbox.y_min, bbox.y_max, bbox.z_min, bbox.z_max);
    // Faces in PMEF coordinates: (normal, 4 corners counter-clockwise seen from outside).
    let faces: [([f64; 3], [[f64; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]]),
        ([-1.0, 0.0, 0.0], [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]]),
        ([0.0, 1.0, 0.0], [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]]),
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
        ([0.0, 0.0, -1.0], [[x0, y1, z0], [x1, y1, z0], [x1, y0, z0], [x0, y0, z0]]),
    ];
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(24);
    let mut indices: Vec<u16> = Vec::with_capacity(36);
    for (n, corners) in faces.iter() {
        // Normals are directions: map axes without translation or scaling.
        let gn = [n[0] as f32, n[2] as f32, -n[1] as f32];
        let base = positions.len() as u16;
        for c in corners {
            positions.push(to_gltf(*c, origin));
            normals.push(gn);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut bin: Vec<u8> = Vec::new();
    for p in &positions {
        for v in p {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    let normals_offset = bin.len();
    for p in &normals {
        for v in p {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    let indices_offset = bin.len();
    for i in &indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    let indices_len = bin.len() - indices_offset;
    while bin.len() % 4 != 0 {
        bin.push(0);
    }

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in &positions {
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }

    let doc = json!({
        "asset": {"version": "2.0", "generator": "pmef-io pmefx"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": id, "mesh": 0, "extras": {"pmefId": id}}],
        "meshes": [{"name": id, "primitives": [{
            "attributes": {"POSITION": 0, "NORMAL": 1},
            "indices": 2,
            "material": 0
        }]}],
        "materials": [{"pbrMetallicRoughness": {
            "baseColorFactor": [0.6, 0.6, 0.6, 1.0], "metallicFactor": 0.0, "roughnessFactor": 1.0
        }}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": normals_offset, "target": 34962},
            {"buffer": 0, "byteOffset": normals_offset, "byteLength": indices_offset - normals_offset, "target": 34962},
            {"buffer": 0, "byteOffset": indices_offset, "byteLength": indices_len, "target": 34963}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": positions.len(), "type": "VEC3", "min": min, "max": max},
            {"bufferView": 1, "componentType": 5126, "count": normals.len(), "type": "VEC3"},
            {"bufferView": 2, "componentType": 5123, "count": indices.len(), "type": "SCALAR"}
        ]
    });
    let mut json_bytes = serde_json::to_vec(&doc).expect("glTF JSON serialises");
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }

    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

/// Statistics of a box mesh written by [`box_glb`]: (triangles, vertices).
pub const BOX_STATS: (u64, u64) = (12, 24);

/// Checks the glb container header (magic, version 2, declared length, JSON first chunk) and
/// returns the parsed JSON chunk.
pub fn parse_glb_json(bytes: &[u8]) -> Result<serde_json::Value, String> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return Err("not a glTF binary (missing 'glTF' magic)".into());
    }
    let u32_at = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    if u32_at(4) != 2 {
        return Err(format!("glTF version {} (expected 2)", u32_at(4)));
    }
    if u32_at(8) as usize != bytes.len() {
        return Err(format!("glb length field {} != actual {}", u32_at(8), bytes.len()));
    }
    let json_len = u32_at(12) as usize;
    if &bytes[16..20] != b"JSON" || 20 + json_len > bytes.len() {
        return Err("first glb chunk is not a valid JSON chunk".into());
    }
    serde_json::from_slice(&bytes[20..20 + json_len]).map_err(|e| format!("glb JSON chunk: {e}"))
}

/// A triangle mesh in PMEF project coordinates (Z-up, mm) — what [`mesh_glb`] writes and
/// [`read_mesh_glb`] returns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlbMesh {
    /// Vertex positions [mm, Z-up, project coordinates].
    pub positions: Vec<[f64; 3]>,
    /// Unit vertex normals (Z-up); empty if the glb carries none.
    pub normals: Vec<[f32; 3]>,
    /// Triangle list.
    pub indices: Vec<u32>,
    /// `baseColorFactor` of the first material, if any (linear RGBA).
    pub color: Option<[f32; 4]>,
}

impl GlbMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Maps a glTF point [m, Y-up] relative to `origin` back to project coordinates [mm, Z-up].
pub fn from_gltf(g: [f32; 3], origin: Point3) -> [f64; 3] {
    [
        g[0] as f64 * 1000.0 + origin.x,
        -(g[2] as f64) * 1000.0 + origin.y,
        g[1] as f64 * 1000.0 + origin.z,
    ]
}

fn dir_to_gltf(n: [f32; 3]) -> [f32; 3] {
    [n[0], n[2], -n[1]]
}

fn dir_from_gltf(n: [f32; 3]) -> [f32; 3] {
    [n[0], -n[2], n[1]]
}

/// A mesh level of detail as `.glb` bytes (spec 03 §3.6 coordinate mapping): positions are
/// written relative to `origin` in metres, Y-up; 32-bit indices. Like [`box_glb`] the single
/// node is named after the PMEF `@id` and carries it in `extras.pmefId`.
///
/// Panics if a normal array of the wrong length is passed or an index is out of range.
pub fn mesh_glb(id: &str, mesh: &GlbMesh, origin: Point3) -> Vec<u8> {
    assert!(
        mesh.normals.is_empty() || mesh.normals.len() == mesh.positions.len(),
        "one normal per vertex"
    );
    assert!(
        mesh.indices
            .iter()
            .all(|&i| (i as usize) < mesh.positions.len()),
        "index out of range"
    );
    let mut bin: Vec<u8> = Vec::new();
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in &mesh.positions {
        let g = to_gltf(*p, origin);
        for k in 0..3 {
            min[k] = min[k].min(g[k]);
            max[k] = max[k].max(g[k]);
            bin.extend_from_slice(&g[k].to_le_bytes());
        }
    }
    let normals_offset = bin.len();
    for n in &mesh.normals {
        for v in dir_to_gltf(*n) {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    let indices_offset = bin.len();
    for i in &mesh.indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    let indices_len = bin.len() - indices_offset;

    let mut attributes = json!({"POSITION": 0});
    let mut views =
        vec![json!({"buffer": 0, "byteOffset": 0, "byteLength": normals_offset, "target": 34962})];
    let (min, max) = if mesh.positions.is_empty() {
        ([0.0f32; 3], [0.0f32; 3])
    } else {
        (min, max)
    };
    let mut accessors = vec![
        json!({"bufferView": 0, "componentType": 5126, "count": mesh.positions.len(), "type": "VEC3", "min": min, "max": max}),
    ];
    if !mesh.normals.is_empty() {
        attributes["NORMAL"] = json!(accessors.len());
        views.push(json!({"buffer": 0, "byteOffset": normals_offset, "byteLength": indices_offset - normals_offset, "target": 34962}));
        accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5126, "count": mesh.normals.len(), "type": "VEC3"}));
    }
    views.push(json!({"buffer": 0, "byteOffset": indices_offset, "byteLength": indices_len, "target": 34963}));
    accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5125, "count": mesh.indices.len(), "type": "SCALAR"}));
    let color = mesh.color.unwrap_or([0.6, 0.6, 0.6, 1.0]);
    let doc = json!({
        "asset": {"version": "2.0", "generator": "pmef-io pmefx"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": id, "mesh": 0, "extras": {"pmefId": id}}],
        "meshes": [{"name": id, "primitives": [{
            "attributes": attributes,
            "indices": accessors.len() - 1,
            "material": 0
        }]}],
        "materials": [{"pbrMetallicRoughness": {
            "baseColorFactor": color, "metallicFactor": 0.0, "roughnessFactor": 1.0
        }}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": views,
        "accessors": accessors
    });
    assemble(doc, bin)
}

/// Result of [`mesh_glb_quantized`].
#[derive(Debug, Clone)]
pub struct QuantizedGlb {
    /// The `.glb` bytes.
    pub bytes: Vec<u8>,
    /// Vertices after welding (equal quantised position and normal).
    pub vertex_count: usize,
    /// Largest position deviation from the input caused by quantisation [mm].
    pub max_error_mm: f64,
}

/// Like [`mesh_glb`], but compact (glTF `KHR_mesh_quantization`, required extension):
/// - positions as normalised 16-bit integers over the mesh bounds, dequantised by the node's
///   uniform scale and translation, as long as the rounding error stays within
///   `max_position_error_mm`; otherwise 32-bit floats;
/// - normals as normalised 8-bit integers;
/// - vertices with equal quantised position and normal welded, 16-bit indices where they fit.
///
/// [`read_mesh_glb`] reads the result back. Roughly halves the size of typical plant meshes.
///
/// Panics like [`mesh_glb`].
#[allow(clippy::manual_is_multiple_of)] // MSRV 1.75
pub fn mesh_glb_quantized(
    id: &str,
    mesh: &GlbMesh,
    origin: Point3,
    max_position_error_mm: f64,
) -> QuantizedGlb {
    assert!(
        mesh.normals.is_empty() || mesh.normals.len() == mesh.positions.len(),
        "one normal per vertex"
    );
    assert!(
        mesh.indices
            .iter()
            .all(|&i| (i as usize) < mesh.positions.len()),
        "index out of range"
    );
    // glTF space [m, Y-up] in f64, so quantisation is the only rounding.
    let g: Vec<[f64; 3]> = mesh
        .positions
        .iter()
        .map(|p| [(p[0] - origin.x) / 1000.0, (p[2] - origin.z) / 1000.0, -(p[1] - origin.y) / 1000.0])
        .collect();
    let mut lo = [0.0f64; 3];
    let mut hi = [0.0f64; 3];
    if let Some(first) = g.first() {
        lo = *first;
        hi = *first;
    }
    for p in &g {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    // One uniform scale keeps normals independent of the node transform.
    let extent = (0..3).map(|k| hi[k] - lo[k]).fold(0.0f64, f64::max).max(1e-9);
    // Worst case: a vertex half a step off on all three axes (Euclidean).
    let half_step_mm = extent / 65535.0 / 2.0 * 1000.0 * 3f64.sqrt();
    let ints = half_step_mm <= max_position_error_mm;

    let qn = |n: [f32; 3]| -> [i8; 3] {
        let n = dir_to_gltf(n);
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
        n.map(|v| ((v / l).clamp(-1.0, 1.0) * 127.0).round() as i8)
    };
    let qp = |p: [f64; 3]| -> [u16; 3] {
        [0, 1, 2].map(|k| (((p[k] - lo[k]) / extent) * 65535.0).round().clamp(0.0, 65535.0) as u16)
    };
    // Weld on the quantised (or, for float positions, the exact f32) values; first use wins.
    let mut slot: std::collections::HashMap<([u32; 3], [i8; 3]), u32> =
        std::collections::HashMap::new();
    let mut remap = vec![0u32; g.len()];
    let mut kept: Vec<usize> = Vec::new();
    for (i, p) in g.iter().enumerate() {
        let pk = if ints {
            qp(*p).map(u32::from)
        } else {
            p.map(|v| (v as f32).to_bits())
        };
        let nk = mesh.normals.get(i).map_or([0; 3], |n| qn(*n));
        remap[i] = *slot.entry((pk, nk)).or_insert_with(|| {
            kept.push(i);
            (kept.len() - 1) as u32
        });
    }
    let mut max_err = 0.0f64;
    let mut bin: Vec<u8> = Vec::new();
    let (pos_type, stride, pmin, pmax, node) = if ints {
        let mut qmin = [u16::MAX; 3];
        let mut qmax = [0u16; 3];
        for &i in &kept {
            let q = qp(g[i]);
            for k in 0..3 {
                qmin[k] = qmin[k].min(q[k]);
                qmax[k] = qmax[k].max(q[k]);
                bin.extend_from_slice(&q[k].to_le_bytes());
            }
            bin.extend_from_slice(&[0, 0]); // 4-byte aligned vertex stride
        }
        for p in &g {
            let q = qp(*p);
            let d2: f64 = (0..3)
                .map(|k| (lo[k] + q[k] as f64 / 65535.0 * extent - p[k]).powi(2))
                .sum();
            max_err = max_err.max(d2.sqrt() * 1000.0);
        }
        if kept.is_empty() {
            qmin = [0; 3];
        }
        (
            5123,
            8,
            json!(qmin),
            json!(qmax),
            json!({"name": id, "mesh": 0, "extras": {"pmefId": id},
                   "translation": lo, "scale": [extent, extent, extent]}),
        )
    } else {
        let mut fmin = [f32::INFINITY; 3];
        let mut fmax = [f32::NEG_INFINITY; 3];
        for &i in &kept {
            for k in 0..3 {
                let v = g[i][k] as f32;
                fmin[k] = fmin[k].min(v);
                fmax[k] = fmax[k].max(v);
                bin.extend_from_slice(&v.to_le_bytes());
            }
        }
        for p in &g {
            let d2: f64 = p.iter().map(|v| (*v as f32 as f64 - v).powi(2)).sum();
            max_err = max_err.max(d2.sqrt() * 1000.0);
        }
        if kept.is_empty() {
            fmin = [0.0; 3];
            fmax = [0.0; 3];
        }
        (5126, 12, json!(fmin), json!(fmax), json!({"name": id, "mesh": 0, "extras": {"pmefId": id}}))
    };
    let normals_offset = bin.len();
    if !mesh.normals.is_empty() {
        for &i in &kept {
            let n = qn(mesh.normals[i]);
            bin.extend_from_slice(&[n[0] as u8, n[1] as u8, n[2] as u8, 0]);
        }
    }
    let indices_offset = bin.len();
    // glTF forbids the maximum index value (primitive restart), so u16 only up to 65 535.
    let short = kept.len() <= u16::MAX as usize;
    for &i in &mesh.indices {
        let r = remap[i as usize];
        if short {
            bin.extend_from_slice(&(r as u16).to_le_bytes());
        } else {
            bin.extend_from_slice(&r.to_le_bytes());
        }
    }
    let indices_len = bin.len() - indices_offset;

    let mut attributes = json!({"POSITION": 0});
    let mut views = vec![json!({"buffer": 0, "byteOffset": 0, "byteLength": normals_offset,
                                "byteStride": stride, "target": 34962})];
    let mut accessors = vec![json!({"bufferView": 0, "componentType": pos_type,
        "count": kept.len(), "type": "VEC3", "min": pmin, "max": pmax})];
    if ints {
        accessors[0]["normalized"] = json!(true);
    }
    if !mesh.normals.is_empty() {
        attributes["NORMAL"] = json!(accessors.len());
        views.push(json!({"buffer": 0, "byteOffset": normals_offset,
            "byteLength": indices_offset - normals_offset, "byteStride": 4, "target": 34962}));
        accessors.push(json!({"bufferView": views.len() - 1, "componentType": 5120,
            "normalized": true, "count": kept.len(), "type": "VEC3"}));
    }
    views.push(json!({"buffer": 0, "byteOffset": indices_offset, "byteLength": indices_len, "target": 34963}));
    accessors.push(json!({"bufferView": views.len() - 1,
        "componentType": if short { 5123 } else { 5125 }, "count": mesh.indices.len(), "type": "SCALAR"}));
    let color = mesh.color.unwrap_or([0.6, 0.6, 0.6, 1.0]);
    let doc = json!({
        "asset": {"version": "2.0", "generator": "pmef-io pmefx"},
        "extensionsUsed": ["KHR_mesh_quantization"],
        "extensionsRequired": ["KHR_mesh_quantization"],
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [node],
        "meshes": [{"name": id, "primitives": [{
            "attributes": attributes,
            "indices": accessors.len() - 1,
            "material": 0
        }]}],
        "materials": [{"pbrMetallicRoughness": {
            "baseColorFactor": color, "metallicFactor": 0.0, "roughnessFactor": 1.0
        }}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": views,
        "accessors": accessors
    });
    QuantizedGlb {
        bytes: assemble(doc, bin),
        vertex_count: kept.len(),
        max_error_mm: max_err,
    }
}

#[allow(clippy::manual_is_multiple_of)] // MSRV 1.75
fn assemble(doc: serde_json::Value, mut bin: Vec<u8>) -> Vec<u8> {
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let mut json_bytes = serde_json::to_vec(&doc).expect("glTF JSON serialises");
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

/// Column-major 4×4 matrix as glTF stores it.
type Mat4 = [f64; 16];

const IDENTITY: Mat4 = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for row in 0..4 {
            r[c * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[c * 4 + k]).sum();
        }
    }
    r
}

fn node_matrix(node: &serde_json::Value) -> Mat4 {
    let nums = |key: &str, n: usize| -> Option<Vec<f64>> {
        let a = node.get(key)?.as_array()?;
        (a.len() == n).then(|| a.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect())
    };
    if let Some(m) = nums("matrix", 16) {
        let mut r = [0.0; 16];
        r.copy_from_slice(&m);
        return r;
    }
    let t = nums("translation", 3).unwrap_or(vec![0.0; 3]);
    let q = nums("rotation", 4).unwrap_or(vec![0.0, 0.0, 0.0, 1.0]);
    let s = nums("scale", 3).unwrap_or(vec![1.0; 3]);
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    // R · S, column-major
    [
        (1.0 - 2.0 * (y * y + z * z)) * s[0],
        (2.0 * (x * y + z * w)) * s[0],
        (2.0 * (x * z - y * w)) * s[0],
        0.0,
        (2.0 * (x * y - z * w)) * s[1],
        (1.0 - 2.0 * (x * x + z * z)) * s[1],
        (2.0 * (y * z + x * w)) * s[1],
        0.0,
        (2.0 * (x * z + y * w)) * s[2],
        (2.0 * (y * z - x * w)) * s[2],
        (1.0 - 2.0 * (x * x + y * y)) * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ]
}

/// Determinant of the linear part of a column-major 4×4 matrix.
fn det3(m: &Mat4) -> f64 {
    let c = cofactor3(m);
    m[0] * c[0][0] + m[4] * c[0][1] + m[8] * c[0][2]
}

/// Cofactor matrix (row-major `[row][col]`) of the linear part: `det · (M⁻¹)ᵀ`.
fn cofactor3(m: &Mat4) -> [[f64; 3]; 3] {
    let a = |r: usize, c: usize| m[c * 4 + r];
    let mut out = [[0.0; 3]; 3];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
            let (c1, c2) = ((c + 1) % 3, (c + 2) % 3);
            *v = a(r1, c1) * a(r2, c2) - a(r1, c2) * a(r2, c1);
        }
    }
    out
}

struct Glb<'a> {
    doc: serde_json::Value,
    bin: &'a [u8],
}

impl Glb<'_> {
    /// Accessor `a` as rows of `n` components (float, or unsigned integer types as f64).
    fn accessor(&self, a: usize, n: usize) -> Result<Vec<Vec<f64>>, String> {
        let acc = &self.doc["accessors"][a];
        let count = acc["count"]
            .as_u64()
            .ok_or(format!("accessor {a}: count"))? as usize;
        let ctype = acc["componentType"]
            .as_u64()
            .ok_or(format!("accessor {a}: componentType"))?;
        let normalized = acc["normalized"].as_bool().unwrap_or(false);
        let size = match ctype {
            5126 | 5125 => 4,
            5122 | 5123 => 2,
            5120 | 5121 => 1,
            other => return Err(format!("accessor {a}: unsupported componentType {other}")),
        };
        let Some(view_idx) = acc["bufferView"].as_u64() else {
            return Ok(vec![vec![0.0; n]; count]); // sparse/zero-initialised: not used by PMEFX writers
        };
        let view = &self.doc["bufferViews"][view_idx as usize];
        if view["buffer"].as_u64().unwrap_or(0) != 0 {
            return Err(format!(
                "accessor {a}: external buffers are not supported in .glb LODs"
            ));
        }
        let overflow = || format!("accessor {a}: offsets overflow");
        let off = |v: &serde_json::Value| {
            usize::try_from(v.as_u64().unwrap_or(0)).map_err(|_| overflow())
        };
        let base = off(&view["byteOffset"])?
            .checked_add(off(&acc["byteOffset"])?)
            .ok_or_else(overflow)?;
        let stride = match view["byteStride"].as_u64() {
            Some(s) => usize::try_from(s).map_err(|_| overflow())?,
            None => size * n,
        };
        let need = if count == 0 {
            Some(0)
        } else {
            stride
                .checked_mul(count - 1)
                .and_then(|v| v.checked_add(base))
                .and_then(|v| v.checked_add(size * n))
        };
        if !matches!(need, Some(need) if need <= self.bin.len()) {
            return Err(format!("accessor {a}: reads past the BIN chunk"));
        }
        let b = self.bin;
        // Normalised integers (KHR_mesh_quantization) map to [0, 1] or [-1, 1].
        let read = |o: usize| -> f64 {
            let (v, max) = match ctype {
                5126 => return f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f64,
                5125 => (u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f64, u32::MAX as f64),
                5123 => (u16::from_le_bytes([b[o], b[o + 1]]) as f64, 65535.0),
                5122 => (i16::from_le_bytes([b[o], b[o + 1]]) as f64, 32767.0),
                5121 => (b[o] as f64, 255.0),
                _ => (b[o] as i8 as f64, 127.0),
            };
            if normalized {
                (v / max).max(-1.0)
            } else {
                v
            }
        };
        Ok((0..count)
            .map(|i| (0..n).map(|k| read(base + i * stride + k * size)).collect())
            .collect())
    }
}

/// Reads all triangle primitives of a `.glb` (default scene, node transforms applied) back
/// into project coordinates (spec 03 §3.6), relative to `origin` = `manifest.meshOrigin`.
/// Only what PMEFX writers emit is supported: embedded BIN chunk, `TRIANGLES` mode, float or
/// `KHR_mesh_quantization` integer positions/normals, unsigned integer indices. Other required
/// extensions are rejected.
pub fn read_mesh_glb(bytes: &[u8], origin: Point3) -> Result<GlbMesh, String> {
    GlbFile::parse(bytes)?.mesh(None, origin)
}

/// A parsed `.glb`: parse once, then read the whole scene or single root nodes. Used for mesh
/// bundles (RFC-0018), where many objects reference nodes of one file.
pub struct GlbFile<'a> {
    g: Glb<'a>,
}

impl<'a> GlbFile<'a> {
    /// Checks the container and the required extensions.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let doc = parse_glb_json(bytes)?;
        let json_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let bin_start = 20 + json_len;
        let bin: &[u8] = if bin_start + 8 <= bytes.len() && &bytes[bin_start + 4..bin_start + 8] == b"BIN\0" {
            let len = u32::from_le_bytes([bytes[bin_start], bytes[bin_start + 1], bytes[bin_start + 2], bytes[bin_start + 3]])
                as usize;
            bytes.get(bin_start + 8..bin_start + 8 + len).ok_or("BIN chunk truncated")?
        } else {
            &[]
        };
        if let Some(req) = doc["extensionsRequired"].as_array() {
            if let Some(e) = req.iter().find(|e| e.as_str() != Some("KHR_mesh_quantization")) {
                return Err(format!("unsupported required glTF extension {e}"));
            }
        }
        Ok(Self { g: Glb { doc, bin } })
    }

    /// Root nodes of the default scene.
    pub fn root_nodes(&self) -> Vec<usize> {
        let scene = self.g.doc["scene"].as_u64().unwrap_or(0) as usize;
        self.g.doc["scenes"][scene]["nodes"]
            .as_array()
            .map(|a| a.iter().filter_map(|n| n.as_u64()).map(|n| n as usize).collect())
            .unwrap_or_default()
    }

    /// `extras.pmefId` (else `name`) of node `n`.
    pub fn node_id(&self, n: usize) -> Option<&str> {
        let node = &self.g.doc["nodes"][n];
        node["extras"]["pmefId"].as_str().or_else(|| node["name"].as_str())
    }

    /// Triangles in project coordinates: of the whole default scene (`node` = `None`), or of
    /// the subtree of one root node of the default scene (`lods[].node`, RFC-0018).
    pub fn mesh(&self, node: Option<usize>, origin: Point3) -> Result<GlbMesh, String> {
        let roots = self.root_nodes();
        let start = match node {
            None => roots,
            Some(n) if roots.contains(&n) => vec![n],
            Some(n) => return Err(format!("node {n} is not a root node of the default scene")),
        };
        read_nodes(&self.g, start, origin)
    }
}

fn read_nodes(g: &Glb<'_>, start: Vec<usize>, origin: Point3) -> Result<GlbMesh, String> {
    let mut out = GlbMesh::default();
    let mut stack: Vec<(usize, Mat4)> = start.into_iter().map(|n| (n, IDENTITY)).collect();
    let mut depth_guard = 0usize;
    while let Some((n, parent)) = stack.pop() {
        depth_guard += 1;
        if depth_guard > 1_000_000 {
            return Err("node graph too large or cyclic".into());
        }
        let node = &g.doc["nodes"][n];
        let m = mat_mul(&parent, &node_matrix(node));
        if let Some(children) = node["children"].as_array() {
            stack.extend(
                children
                    .iter()
                    .filter_map(|c| c.as_u64())
                    .map(|c| (c as usize, m)),
            );
        }
        let Some(mesh) = node["mesh"].as_u64() else {
            continue;
        };
        for prim in g.doc["meshes"][mesh as usize]["primitives"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if prim["mode"].as_u64().unwrap_or(4) != 4 {
                continue;
            }
            let Some(pa) = prim["attributes"]["POSITION"].as_u64() else {
                continue;
            };
            let pos = g.accessor(pa as usize, 3)?;
            let base = out.positions.len() as u32;
            // Normals transform with the inverse transpose of the linear part; its direction is the
            // cofactor matrix times sign(det). A reflection (det < 0) also reverses the winding.
            let det = det3(&m);
            // Scale-independent singularity test: |det| over the product of the column lengths
            // is 1 for any rotation/uniform scale and 0 for a degenerate transform (Hadamard).
            let col = |c: usize| (m[c] * m[c] + m[c + 1] * m[c + 1] + m[c + 2] * m[c + 2]).sqrt();
            let regular = det.abs() > 1e-9 * col(0) * col(4) * col(8);
            // The cofactor scales with s²; normalise it so tiny scales keep usable normals.
            let mut cof = cofactor3(&m);
            let peak = cof.iter().flatten().fold(0.0f64, |a, v| a.max(v.abs()));
            if peak > 0.0 {
                cof.iter_mut().flatten().for_each(|v| *v /= peak);
            }
            for p in &pos {
                let t = [
                    m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
                    m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
                    m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
                ];
                out.positions
                    .push(from_gltf([t[0] as f32, t[1] as f32, t[2] as f32], origin));
            }
            match prim["attributes"]["NORMAL"].as_u64() {
                Some(na) if out.normals.len() == base as usize && regular => {
                    let sign = det.signum();
                    for v in g.accessor(na as usize, 3)? {
                        let t = [0, 1, 2].map(|r| {
                            sign * (cof[r][0] * v[0] + cof[r][1] * v[1] + cof[r][2] * v[2])
                        });
                        let l = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt().max(1e-12);
                        out.normals.push(dir_from_gltf([
                            (t[0] / l) as f32,
                            (t[1] / l) as f32,
                            (t[2] / l) as f32,
                        ]));
                    }
                }
                _ => out.normals.clear(), // mixed: drop normals, consumers recompute
            }
            let count = pos.len() as u32;
            let idx: Vec<u32> = match prim["indices"].as_u64() {
                Some(ia) => g
                    .accessor(ia as usize, 1)?
                    .into_iter()
                    .map(|v| v[0] as u32)
                    .collect(),
                None => (0..count).collect(),
            };
            if idx.iter().any(|&i| i >= count) {
                return Err("index out of range".into());
            }
            let whole = idx.len() - idx.len() % 3; // drop a trailing partial triangle
            if det < 0.0 {
                for k in (0..whole).step_by(3) {
                    out.indices
                        .extend([idx[k] + base, idx[k + 2] + base, idx[k + 1] + base]);
                }
            } else {
                out.indices.extend(idx[..whole].iter().map(|i| i + base));
            }
            if out.color.is_none() {
                if let Some(mat) = prim["material"].as_u64() {
                    let f = &g.doc["materials"][mat as usize]["pbrMetallicRoughness"]
                        ["baseColorFactor"];
                    if let Some(a) = f.as_array().filter(|a| a.len() == 4) {
                        out.color = Some([0, 1, 2, 3].map(|k| a[k].as_f64().unwrap_or(1.0) as f32));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Builds one mesh bundle (RFC-0018) from self-contained `.glb` files, such as those written
/// by [`mesh_glb`], [`mesh_glb_quantized`] or [`box_glb`]. Each part becomes one root node of
/// the bundle's default scene; [`push`](Self::push) returns that node's index, the value of
/// `lods[].node`. Node transforms (used by `KHR_mesh_quantization`) are kept, identical
/// materials are stored once.
#[derive(Debug, Default)]
pub struct GlbBundle {
    nodes: Vec<serde_json::Value>,
    meshes: Vec<serde_json::Value>,
    accessors: Vec<serde_json::Value>,
    views: Vec<serde_json::Value>,
    materials: Vec<serde_json::Value>,
    material_index: std::collections::HashMap<String, usize>,
    roots: Vec<usize>,
    bin: Vec<u8>,
    quantized: bool,
}

impl GlbBundle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of parts pushed so far.
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Bytes the bundle would have now (approximately; for size limits).
    pub fn approx_size(&self) -> usize {
        self.bin.len() + 400 * self.roots.len() + 200
    }

    #[allow(clippy::manual_is_multiple_of)] // MSRV 1.75
    /// Adds a part whose default scene has exactly one root node. Returns the root node's
    /// index in the bundle.
    pub fn push(&mut self, glb: &[u8]) -> Result<usize, String> {
        let f = GlbFile::parse(glb)?;
        let d = &f.g.doc;
        let roots = f.root_nodes();
        let [root] = roots[..] else {
            return Err(format!("a bundle part needs exactly one root node, found {}", roots.len()));
        };
        if d["buffers"].as_array().is_some_and(|b| b.len() > 1) {
            return Err("a bundle part may only use the BIN chunk".into());
        }
        // Materials are copied as they are; texture, image and sampler indices would dangle.
        for k in ["textures", "images", "samplers", "skins", "animations", "cameras"] {
            if d[k].as_array().is_some_and(|a| !a.is_empty()) {
                return Err(format!("bundle parts with {k} are not supported"));
            }
        }
        if d["extensionsRequired"].as_array().is_some_and(|a| !a.is_empty()) {
            self.quantized = true;
        }
        let arr = |k: &str| d[k].as_array().cloned().unwrap_or_default();
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        let bin_base = self.bin.len() as u64;
        self.bin.extend_from_slice(f.g.bin);
        let view_base = self.views.len() as u64;
        for mut v in arr("bufferViews") {
            v["buffer"] = 0.into();
            v["byteOffset"] = (v["byteOffset"].as_u64().unwrap_or(0) + bin_base).into();
            self.views.push(v);
        }
        let acc_base = self.accessors.len() as u64;
        for mut a in arr("accessors") {
            if let Some(bv) = a["bufferView"].as_u64() {
                a["bufferView"] = (bv + view_base).into();
            }
            if a.get("sparse").is_some() {
                return Err("sparse accessors are not supported in bundles".into());
            }
            self.accessors.push(a);
        }
        let mut mat_map = Vec::new();
        for m in arr("materials") {
            let key = m.to_string();
            let i = *self.material_index.entry(key).or_insert_with(|| {
                self.materials.push(m.clone());
                self.materials.len() - 1
            });
            mat_map.push(i as u64);
        }
        let mesh_base = self.meshes.len() as u64;
        for mut m in arr("meshes") {
            for p in m["primitives"].as_array_mut().into_iter().flatten() {
                if let Some(attrs) = p["attributes"].as_object_mut() {
                    for v in attrs.values_mut() {
                        if let Some(a) = v.as_u64() {
                            *v = (a + acc_base).into();
                        }
                    }
                }
                if let Some(i) = p["indices"].as_u64() {
                    p["indices"] = (i + acc_base).into();
                }
                if let Some(mi) = p["material"].as_u64() {
                    p["material"] = (*mat_map.get(mi as usize).ok_or("material index out of range")?).into();
                }
            }
            self.meshes.push(m);
        }
        let node_base = self.nodes.len() as u64;
        for mut n in arr("nodes") {
            if let Some(m) = n["mesh"].as_u64() {
                n["mesh"] = (m + mesh_base).into();
            }
            if let Some(c) = n["children"].as_array_mut() {
                for v in c.iter_mut() {
                    if let Some(i) = v.as_u64() {
                        *v = (i + node_base).into();
                    }
                }
            }
            self.nodes.push(n);
        }
        let idx = node_base as usize + root;
        self.roots.push(idx);
        Ok(idx)
    }

    /// The bundle as `.glb` bytes.
    pub fn finish(self) -> Vec<u8> {
        let mut doc = json!({
            "asset": {"version": "2.0", "generator": "pmef-io pmefx bundle"},
            "scene": 0,
            "scenes": [{"nodes": self.roots}],
            "nodes": self.nodes,
            "meshes": self.meshes,
            "accessors": self.accessors,
            "bufferViews": self.views,
            "buffers": [{"byteLength": self.bin.len()}],
        });
        if !self.materials.is_empty() {
            doc["materials"] = self.materials.into();
        }
        if self.quantized {
            doc["extensionsUsed"] = json!(["KHR_mesh_quantization"]);
            doc["extensionsRequired"] = json!(["KHR_mesh_quantization"]);
        }
        container(&doc, &self.bin)
    }
}

/// glb container around a JSON document and a BIN chunk.
#[allow(clippy::manual_is_multiple_of)] // MSRV 1.75
fn container(doc: &serde_json::Value, bin: &[u8]) -> Vec<u8> {
    let mut json_bytes = serde_json::to_vec(doc).expect("glTF JSON serialises");
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let mut bin = bin.to_vec();
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}
