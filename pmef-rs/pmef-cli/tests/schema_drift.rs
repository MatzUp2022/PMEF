//! Rust model vs. schema drift (docs/AUDIT.md P3.9).
//!
//! For every object in the reference instances (`examples/`, `tests/diff/`, `tests/merge/`, plus
//! the synthetic `tests/fixtures/drift-coverage.ndjson` for types no reference instance uses)
//! whose `@type` has a `pmef-core` type:
//!
//! 1. **accept** — the full, schema-valid object deserialises into the Rust type;
//! 2. **emit** — re-serialising the Rust value yields a schema-valid object;
//! 3. **minimal** — the object reduced to the properties the schema *requires* (recursively,
//!    through `allOf`/`$ref`) still deserialises. This catches Rust fields that are mandatory
//!    in the model but optional in the schema. Reductions that are no longer schema-valid
//!    (conditional `if/then` requirements) are skipped;
//! 4. **lossless** — no property of the input is dropped by the Rust type (catches renamed or
//!    unmodelled fields; `null` and `[]` count as absent).
//!
//! E&I types are frozen (CLAUDE.md scope) and listed in [`FROZEN`]: their drift is reported on
//! stderr but does not fail the test.

use pmef_core as core;
use pmef_validate::PmefValidator;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Frozen disciplines whose Rust types are not maintained (known drift, docs/AUDIT.md P3.9).
const FROZEN: &[&str] = &[
    "pmef:InstrumentObject", "pmef:InstrumentLoop", "pmef:PLCObject",
    "pmef:CableObject", "pmef:CableTrayRun", "pmef:MTPModule",
];

type RoundTrip = fn(&Value) -> Result<Value, String>;

fn rt<T: serde::de::DeserializeOwned + serde::Serialize>(v: &Value) -> Result<Value, String> {
    let t: T = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(&t).map_err(|e| e.to_string())
}

fn registry() -> BTreeMap<&'static str, RoundTrip> {
    let mut m: BTreeMap<&'static str, RoundTrip> = BTreeMap::new();
    m.insert("pmef:FileHeader", rt::<core::FileHeader>);
    m.insert("pmef:Plant", rt::<core::Plant>);
    m.insert("pmef:Unit", rt::<core::Unit>);
    m.insert("pmef:Area", rt::<core::Area>);
    m.insert("pmef:PipingNetworkSystem", rt::<core::PipingNetworkSystem>);
    m.insert("pmef:PipingSegment", rt::<core::PipingSegment>);
    m.insert("pmef:Pipe", rt::<core::Pipe>);
    m.insert("pmef:Elbow", rt::<core::Elbow>);
    m.insert("pmef:Tee", rt::<core::Tee>);
    m.insert("pmef:Reducer", rt::<core::Reducer>);
    m.insert("pmef:Flange", rt::<core::Flange>);
    m.insert("pmef:Gasket", rt::<core::Gasket>);
    m.insert("pmef:Valve", rt::<core::Valve>);
    m.insert("pmef:Olet", rt::<core::Olet>);
    m.insert("pmef:Weld", rt::<core::Weld>);
    m.insert("pmef:PipeSupport", rt::<core::PipeSupport>);
    m.insert("pmef:Spool", rt::<core::Spool>);
    m.insert("pmef:Pump", rt::<core::Pump>);
    m.insert("pmef:Vessel", rt::<core::Vessel>);
    m.insert("pmef:Tank", rt::<core::Tank>);
    m.insert("pmef:HeatExchanger", rt::<core::HeatExchanger>);
    m.insert("pmef:Compressor", rt::<core::Compressor>);
    m.insert("pmef:Reactor", rt::<core::Reactor>);
    m.insert("pmef:Filter", rt::<core::Filter>);
    m.insert("pmef:Turbine", rt::<core::Turbine>);
    m.insert("pmef:GenericEquipment", rt::<core::GenericEquipment>);
    m.insert("pmef:SteelMember", rt::<core::SteelMember>);
    m.insert("pmef:SteelNode", rt::<core::SteelNode>);
    m.insert("pmef:SteelConnection", rt::<core::SteelConnection>);
    m.insert("pmef:SteelSystem", rt::<core::SteelSystem>);
    m.insert("pmef:InstrumentObject", rt::<core::InstrumentObject>);
    m.insert("pmef:InstrumentLoop", rt::<core::InstrumentLoop>);
    m.insert("pmef:PLCObject", rt::<core::PlcObject>);
    m.insert("pmef:CableObject", rt::<core::CableObject>);
    m.insert("pmef:CableTrayRun", rt::<core::CableTrayRun>);
    m.insert("pmef:MTPModule", rt::<core::MtpModule>);
    m.insert("pmef:HasEquivalentIn", rt::<core::HasEquivalentIn>);
    m.insert("pmef:IsDerivedFrom", rt::<core::IsDerivedFrom>);
    m.insert("pmef:IsPartOf", rt::<core::IsPartOf>);
    m.insert("pmef:IsConnectedTo", rt::<core::IsConnectedTo>);
    m.insert("pmef:Supports", rt::<core::Supports>);
    m.insert("pmef:ControlledBy", rt::<core::ControlledBy>);
    m.insert("pmef:IsDocumentedBy", rt::<core::IsDocumentedBy>);
    m.insert("pmef:IsRevisionOf", rt::<core::IsRevisionOf>);
    m.insert("pmef:IsCollocatedWith", rt::<core::IsCollocatedWith>);
    m.insert("pmef:ReplacedBy", rt::<core::ReplacedBy>);
    m
}

/// All schema documents keyed by file name.
struct Schemas(BTreeMap<String, Value>);

impl Schemas {
    fn load(dir: &Path) -> Self {
        let mut m = BTreeMap::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "json") {
                let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
                m.insert(p.file_name().unwrap().to_string_lossy().into_owned(), v);
            }
        }
        Schemas(m)
    }

    /// Resolve a `$ref` relative to `file`; returns (file, schema).
    fn resolve<'a>(&'a self, file: &'a str, r: &'a str) -> (&'a str, &'a Value) {
        let (f, ptr) = r.split_once('#').unwrap_or((r, ""));
        let f = if f.is_empty() { file } else { f };
        let doc = self.0.get_key_value(f).unwrap_or_else(|| panic!("unknown schema file {f}"));
        (doc.0.as_str(), doc.1.pointer(ptr).unwrap_or_else(|| panic!("bad ref {r}")))
    }

    /// The `$defs` entry whose `@type` const equals `t`.
    fn type_def(&self, t: &str) -> Option<(&str, &Value)> {
        for (f, doc) in &self.0 {
            for d in doc.get("$defs").and_then(Value::as_object).into_iter().flat_map(|m| m.values()) {
                let mut parts = vec![d];
                parts.extend(d.get("allOf").and_then(Value::as_array).into_iter().flatten());
                if parts.iter().any(|p| p.pointer("/properties/@type/const") == Some(&Value::from(t))) {
                    return Some((f.as_str(), d));
                }
            }
        }
        None
    }

    /// Flatten a schema through `$ref`/`allOf`: (required names, property sub-schemas).
    fn shape<'a>(&'a self, file: &'a str, s: &'a Value, req: &mut BTreeSet<String>,
                 props: &mut BTreeMap<String, (&'a str, &'a Value)>) {
        if let Some(r) = s.get("$ref").and_then(Value::as_str) {
            let (f, t) = self.resolve(file, r);
            self.shape(f, t, req, props);
        }
        for p in s.get("allOf").and_then(Value::as_array).into_iter().flatten() {
            self.shape(file, p, req, props);
        }
        for r in s.get("required").and_then(Value::as_array).into_iter().flatten() {
            req.insert(r.as_str().unwrap().to_owned());
        }
        for (k, v) in s.get("properties").and_then(Value::as_object).into_iter().flatten() {
            props.entry(k.clone()).or_insert((file, v));
        }
    }

    /// Reduce `v` to what schema `s` requires, recursively.
    fn minimal(&self, file: &str, s: &Value, v: &Value) -> Value {
        match v {
            Value::Object(o) => {
                let (mut req, mut props) = (BTreeSet::new(), BTreeMap::new());
                self.shape(file, s, &mut req, &mut props);
                if req.is_empty() && props.is_empty() {
                    return v.clone(); // free-form object (e.g. customAttributes)
                }
                let mut out = Map::new();
                for k in req {
                    if let Some(x) = o.get(&k) {
                        let x = match props.get(&k) {
                            Some((f, ps)) => self.minimal(f, ps, x),
                            None => x.clone(),
                        };
                        out.insert(k, x);
                    }
                }
                Value::Object(out)
            }
            Value::Array(a) => {
                let (f, items) = match s.get("$ref").and_then(Value::as_str) {
                    Some(r) => {
                        let (f, t) = self.resolve(file, r);
                        (f, t.get("items"))
                    }
                    None => (file, s.get("items")),
                };
                match items {
                    Some(i) => Value::Array(a.iter().map(|x| self.minimal(f, i, x)).collect()),
                    None => v.clone(),
                }
            }
            _ => v.clone(),
        }
    }
}

/// Paths present in `a` but absent in `b` (object keys only, arrays index-aligned).
fn dropped(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                let p = format!("{path}/{k}");
                match y.get(k) {
                    Some(w) => dropped(v, w, &p, out),
                    // absent, null and [] are equivalent (writers omit empty arrays)
                    None if v.is_null() || v.as_array().is_some_and(Vec::is_empty) => {}
                    None => out.push(p),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                dropped(v, w, &format!("{path}/{i}"), out);
            }
        }
        _ => {}
    }
}

fn instances() -> Vec<(String, Value)> {
    let mut out = Vec::new();
    // last entry: synthetic objects (CC0) for types no reference instance uses
    for dir in ["examples", "tests/diff", "tests/merge", "pmef-rs/pmef-cli/tests/fixtures"] {
        let mut files: Vec<_> = std::fs::read_dir(root().join(dir)).unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "ndjson"))
            .collect();
        files.sort();
        for f in files {
            let name = format!("{dir}/{}", f.file_name().unwrap().to_string_lossy());
            for l in std::fs::read_to_string(&f).unwrap().lines().filter(|l| l.starts_with('{')) {
                out.push((name.clone(), serde_json::from_str(l).unwrap()));
            }
        }
    }
    out
}

#[test]
fn rust_types_match_schemas() {
    let validator = PmefValidator::from_directory(&root().join("schemas")).unwrap();
    let schemas = Schemas::load(&root().join("schemas"));
    let reg = registry();
    let mut problems = Vec::new();
    let mut covered = BTreeSet::new();
    for (file, o) in instances() {
        let t = o["@type"].as_str().unwrap();
        let Some(f) = reg.get(t) else { continue };
        covered.insert(t.to_owned());
        let id = o["@id"].as_str().unwrap_or("?");
        if file.starts_with("pmef-rs/") {
            for e in validator.validate_object(t, &o) {
                problems.push(format!("fixture {t} {file} {id}: {} {}", e.instance_path, e.message));
            }
        }
        // 1. accept
        let back = match f(&o) {
            Ok(b) => b,
            Err(e) => { problems.push(format!("accept  {t} {file} {id}: {e}")); continue; }
        };
        // 4. lossless
        let mut lost = Vec::new();
        dropped(&o, &back, "", &mut lost);
        for l in lost {
            problems.push(format!("lossy   {t} {file} {id}: {l}"));
        }
        // 2. emit
        for e in validator.validate_object(t, &back) {
            problems.push(format!("emit    {t} {file} {id}: {} {}", e.instance_path, e.message));
        }
        // 3. minimal
        let (sf, def) = schemas.type_def(t).unwrap_or_else(|| panic!("no schema for {t}"));
        let min = schemas.minimal(sf, def, &o);
        if validator.validate_object(t, &min).is_empty() {
            if let Err(e) = f(&min) {
                problems.push(format!("minimal {t} {file} {id}: {e}"));
            }
        }
    }
    let uncovered: Vec<_> = reg.keys().filter(|k| !covered.contains(**k)).collect();
    eprintln!("covered {} types; no instance for {uncovered:?}", covered.len());
    for t in &uncovered {
        assert!(FROZEN.contains(t) || t.starts_with("pmef:Is") || t.starts_with("pmef:Replaced"),
            "active type {t} has no instance; add one to tests/fixtures/drift-coverage.ndjson");
    }
    let (frozen, active): (Vec<_>, Vec<_>) = problems.into_iter()
        .partition(|p| FROZEN.iter().any(|t| p.split_whitespace().nth(1) == Some(*t)));
    eprintln!("{} known drift problem(s) in frozen E&I types (not failing)", frozen.len());
    assert!(active.is_empty(), "{} drift problem(s):\n{}", active.len(), active.join("\n"));
}
