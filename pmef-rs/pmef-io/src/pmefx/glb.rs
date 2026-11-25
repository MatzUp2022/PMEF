//! Minimal glTF 2.0 binary (`.glb`) writer for `BBOX_ONLY` levels of detail.
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
