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
    check_committed_example("pump-skid-complete", None);
}

#[test]
fn committed_bundled_example_matches_writer_and_schema() {
    // Same with `--bundle 16 --deflate-meshes` (RFC-0018).
    check_committed_example("pump-skid-bundled", Some(16));
}

/// `bundle` = `Some(n)`: bundled bbox levels of n objects, Deflate-compressed.
fn check_committed_example(name: &str, bundle: Option<usize>) {
    let dir = root().join("examples/pmefx").join(name);
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
    match bundle {
        Some(n) => {
            w.set_deflate_meshes(true);
            w.generate_bbox_lods_bundled(n).unwrap();
        }
        None => {
            w.generate_bbox_lods().unwrap();
        }
    }
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

/// RFC-0017: two objects share one mesh file, each placed with its own `offset`.
#[test]
fn shared_mesh_files_are_placed_by_offset() {
    let ids = ["urn:pmef:obj:eaf-2026:CW-201-PIPE-001", "urn:pmef:obj:eaf-2026:CW-201-PIPE-002"];
    let origin = Point3 { x: 1_000.0, y: 2_000.0, z: 0.0 };
    // One tetrahedron in shape-local coordinates (its corner at 0,0,0).
    let shape = glb::GlbMesh {
        positions: vec![[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 100.0, 0.0], [0.0, 0.0, 100.0]],
        normals: vec![[0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        indices: vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
        color: None,
    };
    let bytes = glb::mesh_glb(ids[0], &shape, Point3::default());
    let at = |x: f64| Point3 { x: x - origin.x, y: 500.0 - origin.y, z: 250.0 - origin.z };
    let stats = || LodStats { triangles: Some(4), vertex_count: Some(4), error_mm: None };

    let mut w = PmefxWriter::new(Cursor::new(Vec::new()));
    w.add_ndjson("model/main.ndjson", &example("pump-skid-complete.ndjson")).unwrap();
    w.set_mesh_origin(origin).unwrap();
    w.add_lod_at(ids[0], Lod::Lod3Fine, "meshes/shape.glb", &bytes, stats(), Some(at(3_000.0)))
        .unwrap();
    w.reuse_lod(ids[1], Lod::Lod3Fine, "meshes/shape.glb", stats(), Some(at(7_000.0)))
        .unwrap();
    // Only files of the package can be reused.
    assert!(w.reuse_lod(ids[1], Lod::Lod2Medium, "meshes/missing.glb", stats(), None).is_err());
    let archive = w.finish(FinishOptions::default()).unwrap().into_inner();

    let mut r = reader(&archive);
    assert_eq!(r.verify(), Vec::<String>::new());
    let gltf_files = r.manifest().files.iter().filter(|f| f.kind == FileKind::Gltf).count();
    assert_eq!(gltf_files, 1, "the shape is stored once");
    let package_origin = r.manifest().mesh_origin.unwrap_or_default();
    let index = r.object_index().unwrap();
    let entry_v = validator("ObjectEntry");
    for (id, x) in ids.iter().zip([3_000.0, 7_000.0]) {
        let o = index.iter().find(|o| o.id == *id).unwrap();
        let v = serde_json::to_value(o).unwrap();
        assert_valid(&entry_v, &v, "object index line");
        let back: ObjectEntry = serde_json::from_value(v).unwrap();
        let l = &o.lods[0];
        assert_eq!(back.lods[0].offset, l.offset, "serde round trip");
        assert!(l.offset.is_some());
        let m = glb::read_mesh_glb(&r.read_file(&l.path).unwrap(), l.mesh_origin(package_origin))
            .unwrap();
        let min = m.positions.iter().fold([f64::MAX; 3], |a, p| [0, 1, 2].map(|k| a[k].min(p[k])));
        let ok = (min[0] - x).abs() < 1e-3 && (min[1] - 500.0).abs() < 1e-3 && (min[2] - 250.0).abs() < 1e-3;
        assert!(ok, "{id}: {min:?}");
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
    for scale in [[-2.0, 1.0, 0.5], [3.0, 1.0, 1.0], [1.0, -1.0, 1.0], [1e-5, 1e-5, 1e-5], [-1e-7, 1e-7, 2e-7]] {
        let out = rewrap(&bytes, |doc| {
            doc["nodes"] = json!([{"mesh": 0}, {"children": [0], "scale": scale}]);
            doc["scenes"] = json!([{"nodes": [1]}]);
        });
        let m = glb::read_mesh_glb(&out, Point3::default()).unwrap();
        let f = face(&m.positions, &m.indices);
        assert_eq!(m.normals.len(), 3, "scale {scale:?}: normals dropped");
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

/// Cylinder-like strip with smooth normals and duplicated (unwelded) vertices.
fn tube(center: [f64; 3], radius: f64, length: f64, n: u32) -> glb::GlbMesh {
    let mut m = glb::GlbMesh { color: Some([0.2, 0.4, 0.6, 1.0]), ..Default::default() };
    for k in 0..n {
        let a = |k: u32| std::f64::consts::TAU * f64::from(k % n) / f64::from(n);
        for (a0, a1) in [(a(k), a(k + 1))] {
            let base = m.positions.len() as u32;
            for (ang, x) in [(a0, 0.0), (a1, 0.0), (a1, length), (a0, length)] {
                m.positions.push([center[0] + x, center[1] + radius * ang.cos(), center[2] + radius * ang.sin()]);
                m.normals.push([0.0, ang.cos() as f32, ang.sin() as f32]);
            }
            m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    m
}

#[test]
fn quantized_glb_round_trips_within_its_error_bound() {
    let origin = Point3 { x: 10_000.0, y: -5_000.0, z: 2_000.0 };
    let mesh = tube([12_000.0, -4_000.0, 2_500.0], 150.0, 6_000.0, 48);
    let plain = glb::mesh_glb("urn:pmef:obj:p:t", &mesh, origin);
    let q = glb::mesh_glb_quantized("urn:pmef:obj:p:t", &mesh, origin, 0.5);
    assert!(q.bytes.len() * 2 < plain.len(), "{} vs {}", q.bytes.len(), plain.len());
    assert_eq!(q.vertex_count, 96, "shared edges welded");
    assert!(q.max_error_mm > 0.0 && q.max_error_mm <= 6_000.0 / 65535.0 / 2.0 * 3f64.sqrt() + 1e-6);
    let doc = glb::parse_glb_json(&q.bytes).unwrap();
    assert_eq!(doc["extensionsRequired"], json!(["KHR_mesh_quantization"]));
    assert_eq!(doc["accessors"][0]["componentType"], json!(5123));

    let back = glb::read_mesh_glb(&q.bytes, origin).unwrap();
    assert_eq!(back.triangle_count(), mesh.triangle_count());
    assert_eq!(back.color, mesh.color);
    for (t, src) in back.indices.chunks(3).zip(mesh.indices.chunks(3)) {
        for (&a, &b) in t.iter().zip(src) {
            let (p, s) = (back.positions[a as usize], mesh.positions[b as usize]);
            let err = (0..3).map(|k| (p[k] - s[k]).powi(2)).sum::<f64>().sqrt();
            assert!(err <= q.max_error_mm + 1e-3, "{p:?} vs {s:?}"); // + f32 read-back
            let (n, sn) = (back.normals[a as usize], mesh.normals[b as usize]);
            let dot: f32 = (0..3).map(|k| n[k] * sn[k]).sum();
            assert!(dot > 0.999, "{n:?} vs {sn:?}");
        }
    }
    // Deterministic bytes.
    assert_eq!(q.bytes, glb::mesh_glb_quantized("urn:pmef:obj:p:t", &mesh, origin, 0.5).bytes);
}

#[test]
fn quantized_glb_keeps_float_positions_for_large_meshes() {
    // 200 m long: a 16-bit step would exceed 0.5 mm, so positions stay f32.
    let mesh = tube([0.0, 0.0, 0.0], 500.0, 200_000.0, 8);
    let q = glb::mesh_glb_quantized("urn:pmef:obj:p:t", &mesh, Point3::default(), 0.5);
    let doc = glb::parse_glb_json(&q.bytes).unwrap();
    assert_eq!(doc["accessors"][0]["componentType"], json!(5126));
    assert!(q.max_error_mm < 0.01);
    let back = glb::read_mesh_glb(&q.bytes, Point3::default()).unwrap();
    assert!((back.positions.iter().map(|p| p[0]).fold(0.0, f64::max) - 200_000.0).abs() < 0.01);
}

#[test]
fn quantized_glb_uses_32bit_indices_beyond_65536_vertices() {
    let mut mesh = glb::GlbMesh::default();
    for i in 0..70_000u32 {
        let x = f64::from(i);
        mesh.positions.extend([[x, 0.0, 0.0], [x, 1.0, 0.0], [x, 0.0, 1.0]]);
        mesh.indices.extend([3 * i, 3 * i + 1, 3 * i + 2]);
    }
    let q = glb::mesh_glb_quantized("urn:pmef:obj:p:t", &mesh, Point3::default(), 10.0);
    let doc = glb::parse_glb_json(&q.bytes).unwrap();
    assert_eq!(doc["accessors"][1]["componentType"], json!(5125));
    let back = glb::read_mesh_glb(&q.bytes, Point3::default()).unwrap();
    assert_eq!(back.triangle_count(), 70_000);
    assert_eq!(back.indices.iter().max(), Some(&(q.vertex_count as u32 - 1)));
}

#[test]
fn quantized_glb_never_emits_the_restart_index_65535() {
    // Exactly 65 536 distinct vertices: index 65 535 is used, which glTF forbids
    // as an unsigned-short index (primitive restart), so the indices are u32.
    let mut mesh = glb::GlbMesh::default();
    for i in 0..65_536u32 {
        mesh.positions.push([f64::from(i), f64::from(i % 2), 0.0]);
    }
    for i in 0..65_534u32 {
        mesh.indices.extend([i, i + 1, i + 2]);
    }
    let q = glb::mesh_glb_quantized("urn:pmef:obj:p:t", &mesh, Point3::default(), 10.0);
    assert_eq!(q.vertex_count, 65_536);
    let doc = glb::parse_glb_json(&q.bytes).unwrap();
    assert_eq!(doc["accessors"][0]["componentType"], json!(5123));
    assert_eq!(doc["accessors"][1]["componentType"], json!(5125));
}

#[test]
fn glb_reader_rejects_unknown_required_extensions() {
    let mesh = tube([0.0, 0.0, 0.0], 10.0, 100.0, 4);
    let bytes = glb::mesh_glb("urn:pmef:obj:p:t", &mesh, Point3::default());
    let out = rewrap(&bytes, |doc| doc["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]));
    assert!(glb::read_mesh_glb(&out, Point3::default()).is_err());
}

/// A bundle of three objects (two quantized tubes, one float tetrahedron), Deflate-compressed
/// (RFC-0018). Returns the archive and the per-object source meshes.
fn bundled_package() -> (Vec<u8>, Vec<(&'static str, glb::GlbMesh)>) {
    let origin = Point3 { x: 10_000.0, y: 0.0, z: 0.0 };
    let parts: Vec<(&str, glb::GlbMesh)> = vec![
        ("urn:pmef:obj:eaf-2026:CW-201-PIPE-001", tube([9_000.0, 0.0, 1_000.0], 100.0, 2_195.2, 24)),
        ("urn:pmef:obj:eaf-2026:CW-201-PIPE-002", tube([12_000.0, 0.0, 1_000.0], 100.0, 1_500.0, 24)),
        (
            "urn:pmef:obj:eaf-2026:P-201A",
            glb::GlbMesh {
                positions: vec![[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 100.0, 0.0], [0.0, 0.0, 100.0]],
                normals: vec![],
                indices: vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
                color: Some([0.8, 0.1, 0.1, 1.0]),
            },
        ),
    ];
    let mut bundle = glb::GlbBundle::new();
    let mut nodes = Vec::new();
    for (i, (id, m)) in parts.iter().enumerate() {
        let bytes = if i < 2 { glb::mesh_glb_quantized(id, m, origin, 0.5).bytes } else { glb::mesh_glb(id, m, origin) };
        nodes.push(bundle.push(&bytes).unwrap() as u32);
    }
    assert_eq!(bundle.len(), 3);
    let mut w = PmefxWriter::new(Cursor::new(Vec::new()));
    w.add_ndjson("model/main.ndjson", &example("pump-skid-complete.ndjson")).unwrap();
    w.set_mesh_origin(origin).unwrap();
    w.set_deflate_meshes(true);
    w.add_bundle("meshes/bundle-1.glb", &bundle.finish()).unwrap();
    for ((id, _), node) in parts.iter().zip(nodes) {
        let stats = LodStats { triangles: Some(4), vertex_count: None, error_mm: Some(0.5) };
        w.add_lod_ref(id, Lod::Lod3Fine, "meshes/bundle-1.glb", Some(node), stats, None).unwrap();
    }
    assert!(w.add_lod_ref(parts[0].0, Lod::Lod2Medium, "meshes/missing.glb", Some(0), LodStats::default(), None).is_err());
    // Switching Deflate off afterwards still declares the compressed bundle.
    w.set_deflate_meshes(false);
    (w.finish(FinishOptions::default()).unwrap().into_inner(), parts)
}

#[test]
fn bundled_and_deflated_meshes_round_trip() {
    let (archive, parts) = bundled_package();
    let mut r = reader(&archive);
    assert_eq!(r.verify(), Vec::<String>::new());
    let m = r.manifest().clone();
    assert_eq!(m.pmefx_version.as_deref(), Some(PMEFX_VERSION_FEATURES));
    assert_eq!(m.requires, vec![FEATURE_DEFLATE_MESHES.to_string(), FEATURE_LOD_NODE.to_string()]);
    let manifest: Value = serde_json::from_slice(&r.read_file(MANIFEST_PATH).unwrap()).unwrap();
    assert_valid(&validator("Manifest"), &manifest, "manifest.json");
    assert_eq!(m.files.iter().filter(|f| f.kind == FileKind::Gltf).count(), 1, "one bundle file");

    // The bundle is compressed: no stored range, a raw Deflate range instead.
    assert_eq!(r.stored_range("meshes/bundle-1.glb").unwrap(), None);
    let (_, clen, deflated) = r.raw_range("meshes/bundle-1.glb").unwrap();
    let bytes = r.read_file("meshes/bundle-1.glb").unwrap();
    assert!(deflated && clen < bytes.len() as u64);

    let file = glb::GlbFile::parse(&bytes).unwrap();
    assert_eq!(file.root_nodes().len(), 3);
    let origin = m.mesh_origin.unwrap_or_default();
    let index = r.object_index().unwrap();
    let entry_v = validator("ObjectEntry");
    for (id, src) in &parts {
        let o = index.iter().find(|o| o.id == *id).unwrap();
        let v = serde_json::to_value(o).unwrap();
        assert_valid(&entry_v, &v, "object index line");
        let back: ObjectEntry = serde_json::from_value(v).unwrap();
        assert_eq!(&back, o, "serde round trip");
        let l = &o.lods[0];
        assert_eq!(file.node_id(l.node.unwrap() as usize), Some(*id));
        let mesh = l.read_mesh_from(&file, origin).unwrap();
        assert_eq!(mesh, l.read_mesh(&bytes, origin).unwrap());
        assert_eq!(mesh.triangle_count(), src.triangle_count(), "{id}: only its own node");
        assert_eq!(mesh.color, src.color, "{id}: its own material");
        let lo = |m: &glb::GlbMesh| m.positions.iter().fold([f64::MAX; 3], |a, p| [0, 1, 2].map(|k| a[k].min(p[k])));
        let (a, b) = (lo(&mesh), lo(src));
        assert!((0..3).all(|k| (a[k] - b[k]).abs() < 0.5 + 1e-3), "{id}: {a:?} vs {b:?}");
    }
    // The whole scene still reads as all three meshes.
    let all = file.mesh(None, origin).unwrap();
    assert_eq!(all.triangle_count(), parts.iter().map(|(_, m)| m.triangle_count()).sum::<usize>());
    assert!(file.mesh(Some(99), origin).is_err());
}

#[test]
fn bundles_reject_textured_parts() {
    let id = "urn:pmef:obj:p:t";
    let plain = glb::mesh_glb(id, &tube([0.0; 3], 10.0, 100.0, 8), Point3::default());
    let textured = rewrap(&plain, |d| {
        d["images"] = json!([{"uri": "tex.png"}]);
        d["textures"] = json!([{"source": 0}]);
    });
    let mut b = glb::GlbBundle::new();
    assert!(b.push(&textured).unwrap_err().contains("textures"));
    assert_eq!(b.push(&plain).unwrap(), 0);
}

#[test]
fn verify_enforces_required_features() {
    let (good, _) = bundled_package();
    let with_manifest = |edit: &dyn Fn(&mut Manifest)| {
        let mut m = reader(&good).manifest().clone();
        edit(&mut m);
        let bytes = serde_json::to_vec(&m).unwrap();
        reader(&rewrite(&good, |name, _| (name == MANIFEST_PATH).then(|| (bytes.clone(), zip::CompressionMethod::Deflated))))
            .verify()
    };
    let has = |issues: &[String], s: &str| issues.iter().any(|i| i.contains(s));

    let issues = with_manifest(&|m| m.requires.push("meshoptCompression".into()));
    assert!(has(&issues, "unsupported required feature meshoptCompression"), "{issues:?}");
    let issues = with_manifest(&|m| m.pmefx_version = Some("1.0".into()));
    assert!(has(&issues, "requires needs pmefxVersion 1.1"), "{issues:?}");
    let issues = with_manifest(&|m| m.requires.retain(|f| f != FEATURE_DEFLATE_MESHES));
    assert!(has(&issues, "must be stored uncompressed"), "{issues:?}");
    let issues = with_manifest(&|m| m.requires.retain(|f| f != FEATURE_LOD_NODE));
    assert!(has(&issues, "lods[].node needs"), "{issues:?}");

    // A node that is not a root of the bundle.
    let bad = rewrite(&good, |name, data| {
        (name == OBJECT_INDEX_PATH).then(|| {
            let text = String::from_utf8(data).unwrap().replacen("\"node\":0", "\"node\":7", 1);
            (text.into_bytes(), zip::CompressionMethod::Deflated)
        })
    });
    let issues = reader(&bad).verify();
    assert!(has(&issues, "node 7 is not a root node of meshes/bundle-1.glb"), "{issues:?}");
}

#[test]
fn bundled_bbox_lods_need_fewer_files() {
    let mut w = PmefxWriter::new(Cursor::new(Vec::new()));
    w.add_ndjson("model/main.ndjson", &example("pump-skid-complete.ndjson")).unwrap();
    let boxes = w.generate_bbox_lods_bundled(16).unwrap();
    let mut r = reader(&w.finish(FinishOptions::default()).unwrap().into_inner());
    assert_eq!(r.verify(), Vec::<String>::new());
    let gltf = r.manifest().files.iter().filter(|f| f.kind == FileKind::Gltf).count();
    assert_eq!(gltf, boxes.div_ceil(16));
    let index = r.object_index().unwrap();
    let origin = r.manifest().mesh_origin.unwrap_or_default();
    for o in index.iter().filter(|o| !o.lods.is_empty()) {
        let l = &o.lods[0];
        let m = l.read_mesh(&r.read_file(&l.path).unwrap(), origin).unwrap();
        assert_eq!(m.triangle_count(), 12, "{}", o.id);
        let b = o.bbox.unwrap();
        let lo = m.positions.iter().fold([f64::MAX; 3], |a, p| [0, 1, 2].map(|k| a[k].min(p[k])));
        assert!((lo[0] - b.x_min).abs() < 1e-2 && (lo[2] - b.z_min).abs() < 1e-2, "{}", o.id);
    }
}
