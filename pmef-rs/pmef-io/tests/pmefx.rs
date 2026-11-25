//! PMEFX container (spec 03 §3, RFC-0005): writer → reader round trip, schema conformance of
//! manifest / object index / octree, determinism and tamper detection of `verify`.

#![cfg(feature = "pmefx")]

use pmef_io::pmefx::*;
use serde_json::{json, Value};
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example(name: &str) -> Vec<u8> {
    std::fs::read(root().join("examples").join(name)).unwrap()
}

fn pack(name: &str) -> Vec<u8> {
    let mut w = PmefxWriter::new(Cursor::new(Vec::new()));
    w.add_ndjson(&format!("model/{name}"), &example(name)).unwrap();
    let c = w.extent().unwrap().center();
    w.set_mesh_origin(Point3 { x: (c.x / 1000.0).round() * 1000.0, y: (c.y / 1000.0).round() * 1000.0, z: 0.0 }).unwrap();
    w.generate_bbox_lods().unwrap();
    w.finish(FinishOptions::default()).unwrap().into_inner()
}

fn reader(bytes: &[u8]) -> PmefxReader<Cursor<Vec<u8>>> {
    PmefxReader::open(Cursor::new(bytes.to_vec())).unwrap()
}

/// Compiles `#/$defs/<def>` of the PMEFX schema with the `pmef-base` PmefId inlined.
fn validator(def: &str) -> jsonschema::JSONSchema {
    let base: Value = serde_json::from_slice(&std::fs::read(root().join("schemas/pmef-base.schema.json")).unwrap()).unwrap();
    let pmef_id = base["$defs"]["PmefId"].clone();
    let mut schema: Value =
        serde_json::from_slice(&std::fs::read(root().join("schemas/pmefx-manifest.schema.json")).unwrap()).unwrap();
    fn inline(v: &mut Value, id: &Value) {
        match v {
            Value::Object(m) => {
                if m.get("$ref").and_then(Value::as_str) == Some("pmef-base.schema.json#/$defs/PmefId") {
                    m.remove("$ref");
                    for (k, x) in id.as_object().unwrap() {
                        m.entry(k.clone()).or_insert(x.clone());
                    }
                }
                m.values_mut().for_each(|x| inline(x, id));
            }
            Value::Array(a) => a.iter_mut().for_each(|x| inline(x, id)),
            _ => {}
        }
    }
    inline(&mut schema, &pmef_id);
    schema["$ref"] = json!(format!("#/$defs/{def}"));
    schema.as_object_mut().unwrap().remove("$id");
    jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema)
        .unwrap()
}

fn assert_valid(v: &jsonschema::JSONSchema, instance: &Value, what: &str) {
    if let Err(errors) = v.validate(instance) {
        let msgs: Vec<String> = errors.map(|e| format!("{} at {}", e, e.instance_path)).collect();
        panic!("{what} is not schema-valid: {msgs:#?}");
    }
}

#[test]
fn pump_skid_round_trip_is_conformant() {
    let bytes = pack("pump-skid-complete.ndjson");
    let mut r = reader(&bytes);
    assert_eq!(r.verify(), Vec::<String>::new());
    let m = r.manifest().clone();
    assert_eq!(m.pmefx_version.as_deref(), Some(PMEFX_VERSION));
    assert_eq!(m.package_id, "urn:pmef:pkg:eaf-2026:pump-skid-ds01");
    assert_eq!(m.main_file, "model/pump-skid-complete.ndjson");

    let index = r.object_index().unwrap();
    assert_eq!(Some(index.len() as u64), m.object_count);
    // Every record resolves to its object, attributes included.
    for o in &index {
        let rec = r.read_record(&o.record).unwrap();
        assert_eq!(rec["@id"], json!(o.id));
        assert_eq!(rec["@type"], json!(o.type_str));
    }
    // Piping components get a box from their ports, grown by the bore.
    let pipe = index.iter().find(|o| o.id == "urn:pmef:obj:eaf-2026:CW-201-PIPE-001").unwrap();
    let b = pipe.bbox.unwrap();
    assert_eq!((b.x_min, b.x_max), (9000.0 - 100.0, 11195.2 + 100.0));
    assert_eq!(pipe.lods.len(), 1);
    assert_eq!(pipe.lods[0].lod, Lod::BboxOnly);
    assert_eq!(pipe.lods[0].triangles, Some(12));

    // Meshes are stored uncompressed and can be range-read straight from the archive.
    let (offset, len) = r.stored_range(&pipe.lods[0].path).unwrap().expect("glb is stored");
    let glb = r.read_file(&pipe.lods[0].path).unwrap();
    assert_eq!(&bytes[offset as usize..(offset + len) as usize], &glb[..]);
    let gltf = glb::parse_glb_json(&glb).unwrap();
    assert_eq!(gltf["nodes"][0]["extras"]["pmefId"], json!(pipe.id));
}

#[test]
fn identical_input_gives_identical_archives() {
    assert_eq!(pack("compressor-train.ndjson"), pack("compressor-train.ndjson"));
}

#[test]
fn manifest_index_and_octree_match_the_schema() {
    for name in ["pump-skid-complete.ndjson", "compressor-train.ndjson", "stress-model-cw201.ndjson"] {
        let mut r = reader(&pack(name));
        let manifest: Value = serde_json::from_slice(&r.read_file(MANIFEST_PATH).unwrap()).unwrap();
        assert_valid(&validator("Manifest"), &manifest, "manifest.json");
        let entry_v = validator("ObjectEntry");
        let index = r.read_file(OBJECT_INDEX_PATH).unwrap();
        for line in String::from_utf8(index).unwrap().lines() {
            assert_valid(&entry_v, &serde_json::from_str(line).unwrap(), "object index line");
        }
        let octree: Value = serde_json::from_slice(&r.read_file(SPATIAL_INDEX_PATH).unwrap()).unwrap();
        assert_valid(&validator("Octree"), &octree, "octree.json");
    }
}

#[test]
fn committed_example_matches_writer_and_schema() {
    // examples/pmefx/ holds the manifest and index of `pmef pack examples/pump-skid-complete.ndjson
    // --bbox-lods --created-at 2026-11-25T00:00:00Z` (see examples/pmefx/README.md).
    let dir = root().join("examples/pmefx/pump-skid-complete");
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_valid(&validator("Manifest"), &manifest, "example manifest");
    let typed: Manifest = serde_json::from_value(manifest.clone()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), manifest, "Manifest serde round trip is lossless");

    let index = std::fs::read_to_string(dir.join("index/objects.ndjson")).unwrap();
    for line in index.lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        assert_valid(&validator("ObjectEntry"), &v, "example index line");
        let typed: ObjectEntry = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), v, "ObjectEntry serde round trip is lossless");
    }

    let mut w = PmefxWriter::new(Cursor::new(Vec::new()));
    w.add_ndjson("model/pump-skid-complete.ndjson", &example("pump-skid-complete.ndjson")).unwrap();
    let c = w.extent().unwrap().center();
    w.set_mesh_origin(Point3 { x: (c.x / 1000.0).round() * 1000.0, y: (c.y / 1000.0).round() * 1000.0, z: (c.z / 1000.0).round() * 1000.0 })
        .unwrap();
    w.generate_bbox_lods().unwrap();
    let opts = FinishOptions { created_at: "2026-11-25T00:00:00Z".into(), ..FinishOptions::default() };
    let mut r = reader(&w.finish(opts).unwrap().into_inner());
    let mut fresh: Value = serde_json::from_slice(&r.read_file(MANIFEST_PATH).unwrap()).unwrap();
    let mut committed = manifest;
    // The generator version follows the crate version; everything else must match.
    fresh.as_object_mut().unwrap().remove("generator");
    committed.as_object_mut().unwrap().remove("generator");
    assert_eq!(fresh, committed, "regenerate examples/pmefx (README) after writer changes");
    assert_eq!(String::from_utf8(r.read_file(OBJECT_INDEX_PATH).unwrap()).unwrap(), index);
}

/// Rewrites an archive, letting `edit` change, drop or recompress entries.
fn rewrite(bytes: &[u8], edit: impl Fn(&str, Vec<u8>) -> Option<(Vec<u8>, zip::CompressionMethod)>) -> Vec<u8> {
    let mut zin = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
    let mut zout = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).unwrap();
        let name = f.name().to_string();
        let method = f.compression();
        let mut data = Vec::new();
        f.read_to_end(&mut data).unwrap();
        drop(f);
        let (data, method) = match edit(&name, data.clone()) {
            Some(x) => x,
            None if name.starts_with("__drop__") => continue,
            None => (data, method),
        };
        zout.start_file(name, zip::write::SimpleFileOptions::default().compression_method(method)).unwrap();
        zout.write_all(&data).unwrap();
    }
    zout.finish().unwrap().into_inner()
}

#[test]
fn verify_detects_tampering() {
    let good = pack("pump-skid-complete.ndjson");

    // Dropped index line → count mismatch, hash mismatch, octree entry without index entry.
    let bad = rewrite(&good, |name, data| {
        (name == OBJECT_INDEX_PATH).then(|| {
            let text = String::from_utf8(data).unwrap();
            let kept: Vec<&str> = text.lines().filter(|l| !l.contains("CW-201-PIPE-001")).collect();
            ((kept.join("\n") + "\n").into_bytes(), zip::CompressionMethod::Deflated)
        })
    });
    let issues = reader(&bad).verify();
    assert!(issues.iter().any(|i| i.contains("objectCount")), "{issues:?}");
    assert!(issues.iter().any(|i| i.contains("sha256 mismatch")), "{issues:?}");
    assert!(issues.iter().any(|i| i.contains("no bbox in the object index")), "{issues:?}");

    // Compressed mesh → must be stored.
    let bad = rewrite(&good, |name, data| name.ends_with(".glb").then_some((data, zip::CompressionMethod::Deflated)));
    let issues = reader(&bad).verify();
    assert!(issues.iter().any(|i| i.contains("must be stored uncompressed")), "{issues:?}");

    // Record pointing to the wrong line.
    let bad = rewrite(&good, |name, data| {
        (name == OBJECT_INDEX_PATH).then(|| {
            let text = String::from_utf8(data).unwrap().replace("\"line\":5}", "\"line\":6}");
            (text.into_bytes(), zip::CompressionMethod::Deflated)
        })
    });
    let issues = reader(&bad).verify();
    assert!(issues.iter().any(|i| i.contains("does not hold this @id")), "{issues:?}");
}

#[test]
fn writer_rejects_invalid_input() {
    let mut w = PmefxWriter::new(Cursor::new(Vec::new()));
    assert!(w.add_ndjson("../escape.ndjson", &example("pump-skid-complete.ndjson")).is_err());
    assert!(w.add_ndjson("model/no-header.ndjson", b"{\"@type\":\"pmef:Plant\",\"@id\":\"urn:pmef:plant:p:x\"}\n").is_err());
    w.add_ndjson("model/main.ndjson", &example("pump-skid-complete.ndjson")).unwrap();
    // Same objects again → duplicate @id.
    assert!(w.add_ndjson("model/again.ndjson", &example("pump-skid-complete.ndjson")).is_err());
    assert!(w.add_lod("urn:pmef:obj:eaf-2026:nope", Lod::Lod1Coarse, "meshes/x.glb", b"glTF", LodStats::default()).is_err());
    let id = "urn:pmef:obj:eaf-2026:CW-201-PIPE-001";
    assert!(w.add_lod(id, Lod::Lod1Coarse, "meshes/x.glb", b"not a glb", LodStats::default()).is_err());
    assert!(w.set_brep(id, "brep/x.igs", "iges", b"...").is_err());
    w.set_brep(id, "brep/x.stp", "step", b"ISO-10303-21;").unwrap();
    assert!(w.set_brep(id, "brep/y.stp", "step", b"ISO-10303-21;").is_err());
}

#[test]
fn lods_are_ordered_coarse_to_fine_and_brep_is_indexed() {
    let id = "urn:pmef:obj:eaf-2026:CW-201-PIPE-001";
    let bbox = BBox { x_min: 0.0, x_max: 1.0, y_min: 0.0, y_max: 1.0, z_min: 0.0, z_max: 1.0 };
    let mesh = glb::box_glb(id, &bbox, Point3::default());
    let mut w = PmefxWriter::new(Cursor::new(Vec::new()));
    w.add_ndjson("model/main.ndjson", &example("pump-skid-complete.ndjson")).unwrap();
    let fine = LodStats { triangles: Some(4800), vertex_count: Some(2500), error_mm: Some(0.0) };
    let coarse = LodStats { triangles: Some(96), vertex_count: Some(60), error_mm: Some(12.5) };
    w.add_lod(id, Lod::Lod3Fine, "meshes/pipe.lod3.glb", &mesh, fine).unwrap();
    w.add_lod(id, Lod::Lod1Coarse, "meshes/pipe.lod1.glb", &mesh, coarse).unwrap();
    w.set_brep(id, "brep/pipe.stp", "step", b"ISO-10303-21;\nEND-ISO-10303-21;\n").unwrap();
    w.generate_bbox_lods().unwrap();
    let mut r = reader(&w.finish(FinishOptions::default()).unwrap().into_inner());
    assert_eq!(r.verify(), Vec::<String>::new());
    let o = r.object_index().unwrap().into_iter().find(|o| o.id == id).unwrap();
    let levels: Vec<Lod> = o.lods.iter().map(|l| l.lod).collect();
    assert_eq!(levels, vec![Lod::BboxOnly, Lod::Lod1Coarse, Lod::Lod3Fine]);
    assert_eq!(o.lods[1].error_mm, Some(12.5));
    assert_eq!(o.lods[1].vertex_count, Some(60));
    assert_eq!(o.brep, Some(BrepRef { path: "brep/pipe.stp".into(), format: "step".into() }));
    // B-rep is stored uncompressed as well.
    assert!(r.stored_range("brep/pipe.stp").unwrap().is_some());
}

#[test]
fn octree_holds_each_object_once_in_a_loose_node() {
    // 500 pseudo-random boxes, many straddling octant boundaries.
    let mut s: u64 = 42;
    let mut rnd = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let items: Vec<(String, BBox)> = (0..500)
        .map(|i| {
            let (x, y, z) = (rnd() * 100_000.0, rnd() * 50_000.0, rnd() * 20_000.0);
            let size = rnd() * 3000.0;
            (format!("urn:pmef:obj:p:o-{i:03}"), BBox { x_min: x, x_max: x + size, y_min: y, y_max: y + size, z_min: z, z_max: z + size })
        })
        .collect();
    let tree = octree::build(&items, 6, 8).unwrap();
    let mut seen = std::collections::BTreeMap::new();
    let mut depth_ok = true;
    fn check(n: &OctreeNode, items: &[(String, BBox)], seen: &mut std::collections::BTreeMap<String, usize>, depth: u32, ok: &mut bool) {
        *ok &= depth <= 6 && n.children.len() <= 8;
        for id in &n.objects {
            *seen.entry(id.clone()).or_insert(0) += 1;
            let b = items.iter().find(|(i, _)| i == id).unwrap().1;
            assert!(n.bbox.contains(&b), "loose node box must enclose {id}");
        }
        for c in &n.children {
            assert!(n.bbox.contains(&c.bbox));
            check(c, items, seen, depth + 1, ok);
        }
    }
    check(&tree.root, &items, &mut seen, 0, &mut depth_ok);
    assert!(depth_ok);
    assert_eq!(seen.len(), 500);
    assert!(seen.values().all(|&c| c == 1));
    assert!(!tree.root.children.is_empty(), "500 objects with limit 8 must subdivide");
}

#[test]
fn box_mesh_faces_point_outwards() {
    let bbox = BBox { x_min: 1000.0, x_max: 3000.0, y_min: -500.0, y_max: 500.0, z_min: 0.0, z_max: 750.0 };
    let glb = glb::box_glb("urn:pmef:obj:p:x", &bbox, Point3 { x: 1000.0, y: 0.0, z: 0.0 });
    let doc = glb::parse_glb_json(&glb).unwrap();
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let bin = &glb[20 + json_len + 8..];
    let f = |i: usize| f32::from_le_bytes(bin[i * 4..i * 4 + 4].try_into().unwrap());
    let pos = |v: usize| [f(v * 3), f(v * 3 + 1), f(v * 3 + 2)];
    let nrm = |v: usize| [f(72 + v * 3), f(72 + v * 3 + 1), f(72 + v * 3 + 2)];
    let idx_off = doc["bufferViews"][2]["byteOffset"].as_u64().unwrap() as usize;
    let idx = |k: usize| u16::from_le_bytes(bin[idx_off + k * 2..idx_off + k * 2 + 2].try_into().unwrap()) as usize;
    for t in 0..12 {
        let (a, b, c) = (pos(idx(t * 3)), pos(idx(t * 3 + 1)), pos(idx(t * 3 + 2)));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cross = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let n = nrm(idx(t * 3));
        assert!(cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2] > 0.0, "triangle {t} winds inwards");
    }
    // glTF is Y-up metres relative to the mesh origin: PMEF z 0..750 mm → glTF y 0..0.75 m.
    assert_eq!(doc["accessors"][0]["min"], json!([0.0, 0.0, -0.5]));
    assert_eq!(doc["accessors"][0]["max"], json!([2.0, 0.75, 0.5]));
}

#[test]
fn verify_detects_objects_missing_from_the_index() {
    // Drop a hierarchy object (no bbox, so the octree cannot notice) and fix up count and hash.
    let good = pack("pump-skid-complete.ndjson");
    let dropped = "urn:pmef:unit:eaf-2026:U-100";
    let mut new_index = Vec::new();
    let bad = rewrite(&good, |name, data| {
        (name == OBJECT_INDEX_PATH).then(|| {
            let text = String::from_utf8(data).unwrap();
            let kept: Vec<&str> = text.lines().filter(|l| !l.contains(&format!("\"id\":\"{dropped}\""))).collect();
            ((kept.join("\n") + "\n").into_bytes(), zip::CompressionMethod::Deflated)
        })
    });
    let mut r = reader(&bad);
    new_index.extend(r.read_file(OBJECT_INDEX_PATH).unwrap());
    let mut m = r.manifest().clone();
    m.object_count = Some(m.object_count.unwrap() - 1);
    use sha2::Digest;
    let digest: String = sha2::Sha256::digest(&new_index).iter().map(|b| format!("{b:02x}")).collect();
    for f in &mut m.files {
        if f.path == OBJECT_INDEX_PATH {
            f.size_bytes = Some(new_index.len() as u64);
            f.sha256 = Some(digest.clone());
        }
    }
    let manifest = serde_json::to_vec(&m).unwrap();
    let bad = rewrite(&bad, |name, _| (name == MANIFEST_PATH).then(|| (manifest.clone(), zip::CompressionMethod::Deflated)));
    let issues = reader(&bad).verify();
    assert_eq!(issues, vec![format!("{dropped} in model/pump-skid-complete.ndjson is missing from the object index")]);
}

#[cfg(unix)]
#[test]
fn extraction_does_not_follow_symlinks() {
    let base = std::env::temp_dir().join(format!("pmefx-symlink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (out, elsewhere) = (base.join("out"), base.join("elsewhere"));
    std::fs::create_dir_all(&out).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, out.join("model")).unwrap();
    let err = reader(&pack("stress-model-cw201.ndjson")).extract_to(&out).unwrap_err();
    assert!(err.to_string().contains("symbolic link"), "{err}");
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0, "nothing written through the link");
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn mesh_lods_round_trip_through_glb_and_package() {
    let id = "urn:pmef:obj:eaf-2026:CW-201-PIPE-001";
    let origin = Point3 {
        x: 12_000.0,
        y: -3_000.0,
        z: 500.0,
    };
    // Tetrahedron in project coordinates (mm, Z-up), coloured.
    let mesh = glb::GlbMesh {
        positions: vec![
            [12_000.0, -3_000.0, 500.0],
            [13_000.0, -3_000.0, 500.0],
            [12_000.0, -2_000.0, 500.0],
            [12_000.0, -3_000.0, 1_500.0],
        ],
        normals: vec![
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
        indices: vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
        color: Some([0.8, 0.1, 0.1, 1.0]),
    };
    let bytes = glb::mesh_glb(id, &mesh, origin);
    let doc = glb::parse_glb_json(&bytes).unwrap();
    assert_eq!(doc["nodes"][0]["extras"]["pmefId"], json!(id));
    // glTF space: Y-up metres relative to the origin → project z 1500 mm is glTF y 1.0 m.
    assert_eq!(doc["accessors"][0]["max"][1], json!(1.0));

    let back = glb::read_mesh_glb(&bytes, origin).unwrap();
    assert_eq!(back.indices, mesh.indices);
    assert_eq!(back.color, mesh.color);
    for (a, b) in back.positions.iter().zip(&mesh.positions) {
        assert!((0..3).all(|k| (a[k] - b[k]).abs() < 1e-3), "{a:?} vs {b:?}");
    }
    for (a, b) in back.normals.iter().zip(&mesh.normals) {
        assert!((0..3).all(|k| (a[k] - b[k]).abs() < 1e-6), "{a:?} vs {b:?}");
    }

    // Through a package: the box writer's output is readable too, LODs come back coarse → fine.
    let mut w = PmefxWriter::new(Cursor::new(Vec::new()));
    w.add_ndjson("model/main.ndjson", &example("pump-skid-complete.ndjson"))
        .unwrap();
    w.set_mesh_origin(origin).unwrap();
    let stats = LodStats {
        triangles: Some(4),
        vertex_count: Some(4),
        error_mm: Some(0.0),
    };
    w.add_lod(id, Lod::Lod3Fine, "meshes/pipe.lod3.glb", &bytes, stats)
        .unwrap();
    w.generate_bbox_lods().unwrap();
    let mut r = reader(&w.finish(FinishOptions::default()).unwrap().into_inner());
    assert_eq!(r.verify(), Vec::<String>::new());
    let o = r
        .object_index()
        .unwrap()
        .into_iter()
        .find(|o| o.id == id)
        .unwrap();
    let origin = r.manifest().mesh_origin.unwrap_or_default();
    let coarse = glb::read_mesh_glb(&r.read_file(&o.lods[0].path).unwrap(), origin).unwrap();
    let fine = glb::read_mesh_glb(&r.read_file(&o.lods[1].path).unwrap(), origin).unwrap();
    assert_eq!((coarse.triangle_count(), fine.triangle_count()), (12, 4));
    let bb = o.bbox.unwrap();
    for p in &coarse.positions {
        assert!(
            p[0] >= bb.x_min - 1e-3
                && p[0] <= bb.x_max + 1e-3
                && p[2] >= bb.z_min - 1e-3
                && p[2] <= bb.z_max + 1e-3
        );
    }
}

#[test]
#[allow(clippy::manual_is_multiple_of)] // MSRV 1.75
fn glb_reader_applies_node_transforms_and_rejects_garbage() {
    let mesh = glb::GlbMesh {
        positions: vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [0.0, 1000.0, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let bytes = glb::mesh_glb("urn:pmef:obj:p:t", &mesh, Point3::default());
    // Re-wrap the JSON chunk with a parent node translating by 2 m along glTF x (= project x).
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let mut doc = glb::parse_glb_json(&bytes).unwrap();
    doc["nodes"] = json!([{"mesh": 0}, {"children": [0], "translation": [2.0, 0.0, 0.0]}]);
    doc["scenes"] = json!([{"nodes": [1]}]);
    let mut js = serde_json::to_vec(&doc).unwrap();
    while js.len() % 4 != 0 {
        js.push(b' ');
    }
    let bin = &bytes[20 + json_len..];
    let mut out = b"glTF".to_vec();
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((20 + js.len() + bin.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(bin);
    let m = glb::read_mesh_glb(&out, Point3::default()).unwrap();
    assert!(
        (m.positions[1][0] - 3000.0).abs() < 1e-3,
        "{:?}",
        m.positions
    );
    assert!(m.normals.is_empty());

    assert!(glb::read_mesh_glb(b"not a glb at all, sorry", Point3::default()).is_err());
    let mut truncated = bytes.clone();
    truncated.truncate(bytes.len() - 8);
    let n = truncated.len() as u32;
    truncated[8..12].copy_from_slice(&n.to_le_bytes());
    assert!(glb::read_mesh_glb(&truncated, Point3::default()).is_err());
}

/// Re-wrap a glb with an edited JSON chunk (same BIN chunk).
fn rewrap(bytes: &[u8], edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let mut doc = glb::parse_glb_json(bytes).unwrap();
    edit(&mut doc);
    let mut js = serde_json::to_vec(&doc).unwrap();
    while js.len() & 3 != 0 {
        js.push(b' ');
    }
    let bin = &bytes[20 + json_len..];
    let mut out = b"glTF".to_vec();
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((20 + js.len() + bin.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(bin);
    out
}

#[test]
fn glb_reader_keeps_normals_and_winding_under_mirrored_nonuniform_scale() {
    // Tilted triangle with its face normal as vertex normals.
    let p = [[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [0.0, 1000.0, 1000.0]];
    let face = |p: &[[f64; 3]], i: &[u32]| {
        let v = |k: usize| p[i[k] as usize];
        let (a, b, c) = (v(0), v(1), v(2));
        let (e, f) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        n.map(|x| x / l)
    };
    let n = face(&p, &[0, 1, 2]).map(|x| x as f32);
    let mesh = glb::GlbMesh { positions: p.to_vec(), normals: vec![n; 3], indices: vec![0, 1, 2], color: None };
    let bytes = glb::mesh_glb("urn:pmef:obj:p:t", &mesh, Point3::default());
    for scale in [[-2.0, 1.0, 0.5], [3.0, 1.0, 1.0], [1.0, -1.0, 1.0]] {
        let out = rewrap(&bytes, |doc| {
            doc["nodes"] = json!([{"mesh": 0}, {"children": [0], "scale": scale}]);
            doc["scenes"] = json!([{"nodes": [1]}]);
        });
        let m = glb::read_mesh_glb(&out, Point3::default()).unwrap();
        let f = face(&m.positions, &m.indices);
        for vn in &m.normals {
            let d = f[0] * vn[0] as f64 + f[1] * vn[1] as f64 + f[2] * vn[2] as f64;
            assert!(d > 0.999, "scale {scale:?}: face {f:?} vs normal {vn:?}");
        }
    }
}

#[test]
fn glb_reader_rejects_overflowing_offsets() {
    let mesh = glb::GlbMesh {
        positions: vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [0.0, 1000.0, 0.0]],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let bytes = glb::mesh_glb("urn:pmef:obj:p:t", &mesh, Point3::default());
    for (key, value) in [("byteOffset", json!(u64::MAX)), ("byteStride", json!(u64::MAX / 2))] {
        let out = rewrap(&bytes, |doc| {
            for v in doc["bufferViews"].as_array_mut().unwrap() {
                v[key] = value.clone();
            }
        });
        assert!(glb::read_mesh_glb(&out, Point3::default()).is_err(), "{key}");
    }
}
