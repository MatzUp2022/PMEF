//! `schema_for_type` is a hand-maintained map. This test derives the same map
//! from the schemas (`"@type": {"const": ...}` / `enum`) and fails on drift in
//! either direction, so no object type is silently left unvalidated.

use pmef_validate::schema_for_type;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn collect(node: &Value, file: &str, out: &mut BTreeMap<String, String>) {
    match node {
        Value::Object(map) => {
            if let Some(t) = map.get("properties").and_then(|p| p.get("@type")) {
                let mut values: Vec<&str> = t.get("const").and_then(Value::as_str).into_iter().collect();
                if let Some(e) = t.get("enum").and_then(Value::as_array) {
                    values.extend(e.iter().filter_map(Value::as_str));
                }
                for v in values {
                    out.insert(v.to_owned(), file.to_owned());
                }
            }
            map.values().for_each(|v| collect(v, file, out));
        }
        Value::Array(items) => items.iter().for_each(|v| collect(v, file, out)),
        _ => {}
    }
}

#[test]
fn schema_for_type_covers_every_schema_type() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let mut derived = BTreeMap::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if !name.ends_with(".schema.json") {
            continue;
        }
        let schema: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        collect(&schema, &name, &mut derived);
    }
    assert!(derived.len() > 40, "expected the full type list, got {}", derived.len());

    let mismatches: Vec<String> = derived
        .iter()
        .filter(|(t, f)| schema_for_type(t) != Some(f.as_str()))
        .map(|(t, f)| format!("{t}: schemas say {f}, schema_for_type says {:?}", schema_for_type(t)))
        .collect();
    assert!(mismatches.is_empty(), "type map drift:\n{}", mismatches.join("\n"));
}
