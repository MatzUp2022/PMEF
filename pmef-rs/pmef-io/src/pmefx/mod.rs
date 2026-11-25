//! # PMEFX package container (spec 03 §3, RFC-0005)
//!
//! A `.pmefx` file is a ZIP archive around PMEF NDJSON. Version 1.0 adds an **object index**
//! (`index/objects.ndjson`) that links each object's stable `@id` to
//!
//! * its NDJSON record (attributes),
//! * its bounding box,
//! * mesh levels of detail (glTF binaries, coarse → fine) and
//! * an exact B-rep (STEP AP242),
//!
//! plus an optional loose **octree** (`index/octree.json`) for visibility-driven streaming.
//! A viewer reads `manifest.json` and the index, then range-reads only the meshes it needs;
//! B-reps stay untouched until an object is edited.
//!
//! ```no_run
//! use pmef_io::pmefx::{PmefxWriter, PmefxReader, FinishOptions};
//! # fn main() -> Result<(), pmef_io::pmefx::PmefxError> {
//! let file = std::fs::File::create("plant.pmefx")?;
//! let mut w = PmefxWriter::new(file);
//! w.add_ndjson("model/main.ndjson", &std::fs::read("plant.ndjson")?)?;
//! w.generate_bbox_lods()?;
//! w.finish(FinishOptions::default())?;
//!
//! let mut r = PmefxReader::open(std::fs::File::open("plant.pmefx")?)?;
//! for entry in r.object_index()? { println!("{} {:?}", entry.id, entry.lods.len()); }
//! assert!(r.verify().is_empty());
//! # Ok(()) }
//! ```

pub mod glb;
pub mod model;
pub mod octree;

pub use model::*;

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{Read, Seek, Write};
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Errors from reading or writing PMEFX packages.
#[derive(Debug, Error)]
pub enum PmefxError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("JSON error in {path}: {source}")]
    Json { path: String, #[source] source: serde_json::Error },
    #[error("{0}")]
    Invalid(String),
}

fn invalid<T>(msg: impl Into<String>) -> Result<T, PmefxError> {
    Err(PmefxError::Invalid(msg.into()))
}

/// Checks an archive path against spec 03 §3.2 rule 2: relative, `/`-separated, no `.`/`..`
/// segments, no empty segments, not the reserved manifest path.
pub fn check_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return Err(format!("archive path '{path}' must be relative and use '/'"));
    }
    if path.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return Err(format!("archive path '{path}' has an empty, '.' or '..' segment"));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Turns a PMEF `@id` into a file-name-safe stem.
pub fn file_stem_for_id(id: &str) -> String {
    let local = id.strip_prefix("urn:pmef:").unwrap_or(id);
    local
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect()
}

/// Bounding box of a PMEF record (spec 03 §3.5): `geometry.boundingBox` if present, otherwise
/// the hull of its anchor points (`coordinate`, `center`, `startPoint`, `endPoint`, at any
/// depth) grown by half the largest `nominalDiameter`/`outsideDiameter`. `None` if neither
/// yields a box with volume or extent.
pub fn bbox_from_record(v: &Value) -> Option<BBox> {
    if let Some(b) = v.get("geometry").and_then(|g| g.get("boundingBox")) {
        if let Ok(bbox) = serde_json::from_value::<BBox>(b.clone()) {
            if bbox.is_valid() {
                return Some(bbox);
            }
        }
    }
    fn walk(v: &Value, pts: &mut Vec<[f64; 3]>, radius: &mut f64) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    match (k.as_str(), x) {
                        ("coordinate" | "center" | "startPoint" | "endPoint", Value::Array(a)) if a.len() == 3 => {
                            let p: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
                            if p.len() == 3 && p.iter().all(|c| c.is_finite()) {
                                pts.push([p[0], p[1], p[2]]);
                            }
                        }
                        ("nominalDiameter" | "outsideDiameter", Value::Number(n)) => {
                            *radius = radius.max(n.as_f64().unwrap_or(0.0) / 2.0);
                        }
                        _ => walk(x, pts, radius),
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, pts, radius)),
            _ => {}
        }
    }
    let mut pts = Vec::new();
    let mut r: f64 = 0.0;
    walk(v, &mut pts, &mut r);
    let first = *pts.first()?;
    let mut b = BBox { x_min: first[0], x_max: first[0], y_min: first[1], y_max: first[1], z_min: first[2], z_max: first[2] };
    for p in &pts[1..] {
        b = b.union(&BBox { x_min: p[0], x_max: p[0], y_min: p[1], y_max: p[1], z_min: p[2], z_max: p[2] });
    }
    let r = if r.is_finite() { r } else { 0.0 };
    let b = BBox { x_min: b.x_min - r, x_max: b.x_max + r, y_min: b.y_min - r, y_max: b.y_max + r, z_min: b.z_min - r, z_max: b.z_max + r };
    let has_extent = b.x_max > b.x_min || b.y_max > b.y_min || b.z_max > b.z_min;
    (b.is_valid() && has_extent).then_some(b)
}

/// Options for [`PmefxWriter::finish`].
#[derive(Debug, Clone)]
pub struct FinishOptions {
    /// ISO 8601 timestamp for `manifest.createdAt`. Fixed default keeps output reproducible.
    pub created_at: String,
    /// `"Z-up"` or `"Y-up"`.
    pub coordinate_system: String,
    /// Write `index/octree.json` with these limits (`None` = no spatial index).
    pub octree: Option<(u32, u32)>,
    pub generator: Option<Generator>,
}

impl Default for FinishOptions {
    fn default() -> Self {
        Self {
            created_at: "1980-01-01T00:00:00Z".into(),
            coordinate_system: "Z-up".into(),
            octree: Some((octree::DEFAULT_MAX_DEPTH, octree::DEFAULT_MAX_OBJECTS_PER_NODE)),
            generator: Some(Generator { name: "pmef-io".into(), version: env!("CARGO_PKG_VERSION").into() }),
        }
    }
}

/// Mesh statistics supplied with a level of detail.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LodStats {
    pub triangles: Option<u64>,
    pub vertex_count: Option<u64>,
    pub error_mm: Option<f64>,
}

/// Streaming PMEFX writer. Entries are written to the ZIP as they are added; the object
/// index, octree and manifest are written by [`finish`](Self::finish). Entry timestamps are
/// fixed, so identical input yields byte-identical archives.
pub struct PmefxWriter<W: Write + Seek> {
    zip: ZipWriter<W>,
    files: Vec<FileEntry>,
    paths: BTreeSet<String>,
    objects: BTreeMap<String, ObjectEntry>,
    main_file: Option<String>,
    package_id: Option<String>,
    pmef_version: Option<String>,
    mesh_origin: Option<Point3>,
}

impl<W: Write + Seek> PmefxWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            zip: ZipWriter::new(inner),
            files: Vec::new(),
            paths: BTreeSet::new(),
            objects: BTreeMap::new(),
            main_file: None,
            package_id: None,
            pmef_version: None,
            mesh_origin: None,
        }
    }

    /// Sets `manifest.meshOrigin` [mm]. Must be called before any mesh is added.
    pub fn set_mesh_origin(&mut self, origin: Point3) -> Result<(), PmefxError> {
        if self.files.iter().any(|f| f.kind == FileKind::Gltf) {
            return invalid("mesh origin must be set before meshes are added");
        }
        self.mesh_origin = Some(origin);
        Ok(())
    }

    pub fn mesh_origin(&self) -> Point3 {
        self.mesh_origin.unwrap_or_default()
    }

    /// Union of all object bounding boxes indexed so far.
    pub fn extent(&self) -> Option<BBox> {
        self.objects.values().filter_map(|o| o.bbox).reduce(|a, b| a.union(&b))
    }

    /// Read access to the object index built so far.
    pub fn objects(&self) -> impl Iterator<Item = &ObjectEntry> {
        self.objects.values()
    }

    fn write_entry(&mut self, path: &str, kind: FileKind, bytes: &[u8], objects: Option<u64>) -> Result<(), PmefxError> {
        check_path(path).map_err(PmefxError::Invalid)?;
        if path == MANIFEST_PATH || !self.paths.insert(path.to_string()) {
            return invalid(format!("duplicate or reserved archive path '{path}'"));
        }
        let method = if kind.must_be_stored() { CompressionMethod::Stored } else { CompressionMethod::Deflated };
        let opts = SimpleFileOptions::default()
            .compression_method(method)
            .large_file(bytes.len() as u64 >= u32::MAX as u64);
        self.zip.start_file(path, opts)?;
        self.zip.write_all(bytes)?;
        self.files.push(FileEntry {
            path: path.to_string(),
            kind,
            size_bytes: Some(bytes.len() as u64),
            sha256: Some(sha256_hex(bytes)),
            objects,
        });
        Ok(())
    }

    /// Adds a PMEF NDJSON file and indexes every object in it. The first NDJSON file added is
    /// the package's `mainFile`. Each file must start with a `pmef:FileHeader` (spec 03 §3.4).
    pub fn add_ndjson(&mut self, path: &str, bytes: &[u8]) -> Result<u64, PmefxError> {
        let text = std::str::from_utf8(bytes).map_err(|_| PmefxError::Invalid(format!("{path}: not UTF-8")))?;
        let mut count = 0u64;
        let mut new_objects = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let t = line.trim();
            if t.is_empty() || t.starts_with("//") {
                continue;
            }
            let v: Value = serde_json::from_str(t)
                .map_err(|e| PmefxError::Json { path: format!("{path}:{}", i + 1), source: e })?;
            let type_str = v.get("@type").and_then(Value::as_str).unwrap_or_default().to_string();
            let id = v.get("@id").and_then(Value::as_str).unwrap_or_default().to_string();
            if type_str.is_empty() || id.is_empty() {
                return invalid(format!("{path}:{}: object without '@type' or '@id'", i + 1));
            }
            if count == 0 {
                if type_str != "pmef:FileHeader" {
                    return invalid(format!("{path}: first object must be a pmef:FileHeader"));
                }
                if self.main_file.is_none() {
                    self.package_id = Some(id.clone());
                    self.pmef_version = v.get("pmefVersion").and_then(Value::as_str).map(str::to_string);
                }
            }
            count += 1;
            if self.objects.contains_key(&id) || new_objects.iter().any(|o: &ObjectEntry| o.id == id) {
                return invalid(format!("{path}:{}: duplicate @id {id}", i + 1));
            }
            new_objects.push(ObjectEntry {
                bbox: bbox_from_record(&v),
                id,
                type_str,
                record: RecordRef { path: path.to_string(), line: (i + 1) as u64 },
                lods: Vec::new(),
                brep: None,
            });
        }
        if count == 0 {
            return invalid(format!("{path}: no PMEF objects"));
        }
        self.write_entry(path, FileKind::Ndjson, bytes, Some(count))?;
        if self.main_file.is_none() {
            self.main_file = Some(path.to_string());
        }
        for o in new_objects {
            self.objects.insert(o.id.clone(), o);
        }
        Ok(count)
    }

    /// Adds a file that is not linked to an object (catalog, document, whole-model glb, …).
    pub fn add_file(&mut self, path: &str, kind: FileKind, bytes: &[u8]) -> Result<(), PmefxError> {
        if kind == FileKind::Ndjson {
            return self.add_ndjson(path, bytes).map(|_| ());
        }
        self.write_entry(path, kind, bytes, None)
    }

    fn object_mut(&mut self, id: &str) -> Result<&mut ObjectEntry, PmefxError> {
        self.objects
            .get_mut(id)
            .ok_or_else(|| PmefxError::Invalid(format!("unknown object {id} (add its NDJSON first)")))
    }

    /// Adds a mesh level of detail (glTF binary) for object `id`. Levels are kept ordered
    /// coarse → fine; each level may occur once per object.
    pub fn add_lod(&mut self, id: &str, lod: Lod, path: &str, glb_bytes: &[u8], stats: LodStats) -> Result<(), PmefxError> {
        if self.object_mut(id)?.lods.iter().any(|l| l.lod == lod) {
            return invalid(format!("{id}: duplicate level {}", lod.as_str()));
        }
        glb::parse_glb_json(glb_bytes).map_err(|e| PmefxError::Invalid(format!("{path}: {e}")))?;
        self.write_entry(path, FileKind::Gltf, glb_bytes, None)?;
        let o = self.object_mut(id)?;
        o.lods.push(LodRef {
            lod,
            path: path.to_string(),
            triangles: stats.triangles,
            vertex_count: stats.vertex_count,
            error_mm: stats.error_mm,
            size_bytes: Some(glb_bytes.len() as u64),
        });
        o.lods.sort_by_key(|l| l.lod);
        Ok(())
    }

    /// Adds the exact B-rep of object `id` (`format` = `"step"` or `"x-…"`).
    pub fn set_brep(&mut self, id: &str, path: &str, format: &str, bytes: &[u8]) -> Result<(), PmefxError> {
        if format != "step" && !format.starts_with("x-") {
            return invalid(format!("unknown B-rep format '{format}' (step or x-…)"));
        }
        if self.object_mut(id)?.brep.is_some() {
            return invalid(format!("{id}: B-rep already set"));
        }
        let kind = if format == "step" { FileKind::Step } else { FileKind::Other };
        self.write_entry(path, kind, bytes, None)?;
        self.object_mut(id)?.brep = Some(BrepRef { path: path.to_string(), format: format.to_string() });
        Ok(())
    }

    /// Writes a `BBOX_ONLY` box mesh for every object that has a bounding box and no such
    /// level yet. Returns the number of meshes written.
    pub fn generate_bbox_lods(&mut self) -> Result<usize, PmefxError> {
        let origin = self.mesh_origin();
        let todo: Vec<(String, BBox)> = self
            .objects
            .values()
            .filter(|o| !o.lods.iter().any(|l| l.lod == Lod::BboxOnly))
            .filter_map(|o| o.bbox.map(|b| (o.id.clone(), b)))
            .collect();
        for (id, bbox) in &todo {
            let stem = file_stem_for_id(id);
            let mut path = format!("meshes/{stem}.bbox.glb");
            let mut n = 1;
            while self.paths.contains(&path) {
                n += 1;
                path = format!("meshes/{stem}-{n}.bbox.glb");
            }
            let bytes = glb::box_glb(id, bbox, origin);
            let stats = LodStats {
                triangles: Some(glb::BOX_STATS.0),
                vertex_count: Some(glb::BOX_STATS.1),
                error_mm: None,
            };
            self.add_lod(id, Lod::BboxOnly, &path, &bytes, stats)?;
        }
        Ok(todo.len())
    }

    /// Writes the object index, spatial index and manifest, and closes the archive.
    pub fn finish(mut self, opts: FinishOptions) -> Result<W, PmefxError> {
        let (Some(main_file), Some(package_id)) = (self.main_file.clone(), self.package_id.clone()) else {
            return invalid("a PMEFX package needs at least one NDJSON file");
        };
        let mut index = Vec::new();
        for o in self.objects.values() {
            serde_json::to_writer(&mut index, o).map_err(|e| PmefxError::Json { path: OBJECT_INDEX_PATH.into(), source: e })?;
            index.push(b'\n');
        }
        let object_count = self.objects.len() as u64;
        self.write_entry(OBJECT_INDEX_PATH, FileKind::Index, &index, None)?;

        let mut spatial_index = None;
        if let Some((depth, per_node)) = opts.octree {
            let items: Vec<(String, BBox)> = self.objects.values().filter_map(|o| o.bbox.map(|b| (o.id.clone(), b))).collect();
            if let Some(tree) = octree::build(&items, depth, per_node) {
                let bytes = serde_json::to_vec(&tree).map_err(|e| PmefxError::Json { path: SPATIAL_INDEX_PATH.into(), source: e })?;
                self.write_entry(SPATIAL_INDEX_PATH, FileKind::Index, &bytes, None)?;
                spatial_index = Some(SPATIAL_INDEX_PATH.to_string());
            }
        }

        let manifest = Manifest {
            pmefx_version: Some(PMEFX_VERSION.into()),
            pmef_version: self.pmef_version.clone().unwrap_or_else(|| "0.9.0".into()),
            package_id,
            main_file,
            coordinate_system: opts.coordinate_system,
            created_at: opts.created_at,
            files: self.files.clone(),
            mesh_origin: self.mesh_origin,
            object_index: Some(OBJECT_INDEX_PATH.into()),
            spatial_index,
            object_count: Some(object_count),
            generator: opts.generator,
        };
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| PmefxError::Json { path: MANIFEST_PATH.into(), source: e })?;
        self.zip.start_file(MANIFEST_PATH, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))?;
        self.zip.write_all(&bytes)?;
        Ok(self.zip.finish()?)
    }
}

/// PMEFX reader: random access to manifest, index and individual entries.
pub struct PmefxReader<R: Read + Seek> {
    zip: ZipArchive<R>,
    manifest: Manifest,
}

impl<R: Read + Seek> PmefxReader<R> {
    /// Opens an archive and parses `manifest.json`. Nothing else is read.
    pub fn open(inner: R) -> Result<Self, PmefxError> {
        let mut zip = ZipArchive::new(inner)?;
        let bytes = read_entry(&mut zip, MANIFEST_PATH)?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|e| PmefxError::Json { path: MANIFEST_PATH.into(), source: e })?;
        Ok(Self { zip, manifest })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Reads one archive entry (uncompressed bytes).
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, PmefxError> {
        read_entry(&mut self.zip, path)
    }

    /// Byte range of an uncompressed (STORED) entry inside the archive, for HTTP range reads or
    /// memory mapping: `(offset, length)`. `None` if the entry is compressed.
    pub fn stored_range(&mut self, path: &str) -> Result<Option<(u64, u64)>, PmefxError> {
        let f = self.zip.by_name(path)?;
        Ok((f.compression() == CompressionMethod::Stored).then(|| (f.data_start(), f.size())))
    }

    /// The object index (empty for 0.9 packages without one).
    pub fn object_index(&mut self) -> Result<Vec<ObjectEntry>, PmefxError> {
        let Some(path) = self.manifest.object_index.clone() else { return Ok(Vec::new()) };
        let bytes = self.read_file(&path)?;
        let text = String::from_utf8_lossy(&bytes);
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| serde_json::from_str(l).map_err(|e| PmefxError::Json { path: format!("{path}:{}", i + 1), source: e }))
            .collect()
    }

    /// The spatial index, if the package has one.
    pub fn octree(&mut self) -> Result<Option<Octree>, PmefxError> {
        let Some(path) = self.manifest.spatial_index.clone() else { return Ok(None) };
        let bytes = self.read_file(&path)?;
        serde_json::from_slice(&bytes).map(Some).map_err(|e| PmefxError::Json { path, source: e })
    }

    /// Loads the NDJSON record (all attributes) an index entry points to.
    pub fn read_record(&mut self, record: &RecordRef) -> Result<Value, PmefxError> {
        let bytes = self.read_file(&record.path)?;
        let text = String::from_utf8_lossy(&bytes);
        let line = text
            .lines()
            .nth(record.line.saturating_sub(1) as usize)
            .ok_or_else(|| PmefxError::Invalid(format!("{}: no line {}", record.path, record.line)))?;
        serde_json::from_str(line).map_err(|e| PmefxError::Json { path: format!("{}:{}", record.path, record.line), source: e })
    }

    /// Checks the package against spec 03 §3 (structure, integrity, index consistency).
    /// Returns one message per problem; empty = conformant.
    pub fn verify(&mut self) -> Vec<String> {
        let mut issues = Vec::new();
        let m = self.manifest.clone();

        if let Some(v) = &m.pmefx_version {
            if v.split('.').next() != Some("1") {
                issues.push(format!("unsupported pmefxVersion {v}"));
            }
        }
        if m.coordinate_system != "Z-up" && m.coordinate_system != "Y-up" {
            issues.push(format!("coordinateSystem must be Z-up or Y-up, got {}", m.coordinate_system));
        }

        // Files listed ↔ archive entries.
        let mut listed: HashMap<&str, &FileEntry> = HashMap::new();
        for f in &m.files {
            if let Err(e) = check_path(&f.path) {
                issues.push(e);
            }
            if listed.insert(f.path.as_str(), f).is_some() {
                issues.push(format!("{} is listed twice in files", f.path));
            }
        }
        let names: Vec<String> = self.zip.file_names().map(str::to_string).collect();
        for n in &names {
            if n != MANIFEST_PATH && !n.ends_with('/') && !listed.contains_key(n.as_str()) {
                issues.push(format!("archive entry {n} is not listed in manifest.files"));
            }
        }
        for f in &m.files {
            let entry = match self.zip.by_name(&f.path) {
                Ok(e) => e,
                Err(_) => {
                    issues.push(format!("{} is listed but missing from the archive", f.path));
                    continue;
                }
            };
            if f.kind.must_be_stored() && entry.compression() != CompressionMethod::Stored {
                issues.push(format!("{} ({:?}) must be stored uncompressed", f.path, f.kind));
            }
            drop(entry);
            match self.read_file(&f.path) {
                Ok(bytes) => {
                    if f.size_bytes.is_some_and(|s| s != bytes.len() as u64) {
                        issues.push(format!("{}: sizeBytes {} != actual {}", f.path, f.size_bytes.unwrap_or(0), bytes.len()));
                    }
                    if let Some(h) = &f.sha256 {
                        if *h != sha256_hex(&bytes) {
                            issues.push(format!("{}: sha256 mismatch", f.path));
                        }
                    }
                    if f.kind == FileKind::Gltf {
                        if let Err(e) = glb::parse_glb_json(&bytes) {
                            issues.push(format!("{}: {e}", f.path));
                        }
                    }
                    if f.kind == FileKind::Ndjson {
                        let first = String::from_utf8_lossy(&bytes)
                            .lines()
                            .find(|l| !l.trim().is_empty() && !l.trim().starts_with("//"))
                            .and_then(|l| serde_json::from_str::<Value>(l).ok());
                        if first.as_ref().and_then(|v| v.get("@type")).and_then(Value::as_str) != Some("pmef:FileHeader") {
                            issues.push(format!("{}: first object is not a pmef:FileHeader", f.path));
                        }
                    }
                }
                Err(e) => issues.push(format!("{}: {e}", f.path)),
            }
        }
        match listed.get(m.main_file.as_str()) {
            Some(f) if f.kind == FileKind::Ndjson => {}
            _ => issues.push(format!("mainFile {} is not a listed ndjson file", m.main_file)),
        }

        // Object index.
        let index = match self.object_index() {
            Ok(i) => i,
            Err(e) => {
                issues.push(e.to_string());
                return issues;
            }
        };
        if m.object_count.is_some_and(|c| c != index.len() as u64) {
            issues.push(format!("objectCount {} != index lines {}", m.object_count.unwrap_or(0), index.len()));
        }
        let mut seen = BTreeSet::new();
        let mut ndjson_cache: HashMap<String, Vec<String>> = HashMap::new();
        for o in &index {
            if !seen.insert(o.id.as_str()) {
                issues.push(format!("{} appears twice in the object index", o.id));
            }
            match listed.get(o.record.path.as_str()) {
                Some(f) if f.kind == FileKind::Ndjson => {
                    if !ndjson_cache.contains_key(&o.record.path) {
                        let lines = self
                            .read_file(&o.record.path)
                            .map(|b| String::from_utf8_lossy(&b).lines().map(str::to_string).collect())
                            .unwrap_or_default();
                        ndjson_cache.insert(o.record.path.clone(), lines);
                    }
                    let rec = ndjson_cache[&o.record.path]
                        .get(o.record.line.saturating_sub(1) as usize)
                        .and_then(|l| serde_json::from_str::<Value>(l).ok());
                    if rec.as_ref().and_then(|v| v.get("@id")).and_then(Value::as_str) != Some(o.id.as_str()) {
                        issues.push(format!("{}: record {}:{} does not hold this @id", o.id, o.record.path, o.record.line));
                    }
                }
                _ => issues.push(format!("{}: record path {} is not a listed ndjson file", o.id, o.record.path)),
            }
            if let Some(b) = &o.bbox {
                if !b.is_valid() {
                    issues.push(format!("{}: invalid bbox", o.id));
                }
            }
            let mut prev: Option<Lod> = None;
            for l in &o.lods {
                if prev.is_some_and(|p| p >= l.lod) {
                    issues.push(format!("{}: lods must be ordered coarse to fine without duplicates", o.id));
                }
                prev = Some(l.lod);
                match listed.get(l.path.as_str()) {
                    Some(f) if f.kind == FileKind::Gltf => {}
                    _ => issues.push(format!("{}: lod path {} is not a listed gltf file", o.id, l.path)),
                }
            }
            if let Some(b) = &o.brep {
                if b.format != "step" && !b.format.starts_with("x-") {
                    issues.push(format!("{}: unknown brep format {}", o.id, b.format));
                }
                if !listed.contains_key(b.path.as_str()) {
                    issues.push(format!("{}: brep path {} is not listed", o.id, b.path));
                }
            }
        }

        // Spatial index: every object with a bbox exactly once, nothing unknown.
        match self.octree() {
            Ok(Some(tree)) => {
                let with_bbox: BTreeSet<&str> = index.iter().filter(|o| o.bbox.is_some()).map(|o| o.id.as_str()).collect();
                let mut count: BTreeMap<String, usize> = BTreeMap::new();
                let mut loose_ok = true;
                octree::walk(&tree.root, &mut |n| {
                    for id in &n.objects {
                        *count.entry(id.clone()).or_default() += 1;
                        if let Some(b) = index.iter().find(|o| &o.id == id).and_then(|o| o.bbox) {
                            loose_ok &= n.bbox.contains(&b);
                        }
                    }
                    for c in &n.children {
                        loose_ok &= n.bbox.contains(&c.bbox);
                    }
                });
                for (id, c) in &count {
                    if !with_bbox.contains(id.as_str()) {
                        issues.push(format!("octree references {id}, which has no bbox in the object index"));
                    } else if *c != 1 {
                        issues.push(format!("{id} appears {c} times in the octree (must be exactly once)"));
                    }
                }
                for id in with_bbox {
                    if !count.contains_key(id) {
                        issues.push(format!("{id} has a bbox but is missing from the octree"));
                    }
                }
                if !loose_ok {
                    issues.push("octree node bbox does not enclose its objects and children".into());
                }
            }
            Ok(None) => {}
            Err(e) => issues.push(e.to_string()),
        }
        issues
    }

    /// Extracts every entry (including `manifest.json`) below `dir`.
    pub fn extract_to(&mut self, dir: &std::path::Path) -> Result<usize, PmefxError> {
        let names: Vec<String> = self.zip.file_names().map(str::to_string).collect();
        let mut n = 0;
        for name in names {
            if name.ends_with('/') {
                continue;
            }
            if name != MANIFEST_PATH {
                check_path(&name).map_err(PmefxError::Invalid)?;
            }
            let target = dir.join(&name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, self.read_file(&name)?)?;
            n += 1;
        }
        Ok(n)
    }
}

fn read_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, path: &str) -> Result<Vec<u8>, PmefxError> {
    let mut f = zip.by_name(path)?;
    let mut out = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut out)?;
    Ok(out)
}
