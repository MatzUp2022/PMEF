//! pmef-snap/1 must reproduce the shared, language-neutral test vectors
//! (tests/snapping/vectors.json) that the Python reference implementation also passes.

use pmef_core::snapping::{snap, SnapKind, SnapOutcome, SnapRequest};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn matches(expected: &Value, got: &Value) -> bool {
    match (expected, got) {
        (Value::Object(e), Value::Object(g)) => {
            e.len() == g.len() && e.iter().all(|(k, v)| g.get(k).is_some_and(|gv| matches(v, gv)))
        }
        (Value::Array(e), Value::Array(g)) => e.len() == g.len() && e.iter().zip(g).all(|(a, b)| matches(a, b)),
        (Value::Number(e), Value::Number(g)) => (e.as_f64().unwrap() - g.as_f64().unwrap()).abs() <= 1e-6,
        _ => expected == got,
    }
}

fn to_map(v: &Value) -> BTreeMap<String, f64> {
    v.as_object().unwrap().iter().map(|(k, x)| (k.clone(), x.as_f64().unwrap())).collect()
}

#[test]
fn rust_implementation_reproduces_all_vectors() {
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/snapping/vectors.json")).unwrap()).unwrap();
    let vectors = doc["vectors"].as_array().unwrap();
    assert!(vectors.len() >= 16);
    let mut failures = Vec::new();
    for v in vectors {
        let catalog: Value =
            serde_json::from_str(&std::fs::read_to_string(root().join(v["catalog"].as_str().unwrap())).unwrap())
                .unwrap();
        let measured = to_map(&v["measured"]);
        let tolerances = v.get("tolerances").map(to_map);
        let req = SnapRequest {
            kind: SnapKind::parse(v["kind"].as_str().unwrap()).unwrap(),
            measured: &measured,
            family: v["family"].as_str(),
            tolerances: tolerances.as_ref(),
        };
        let ok = match (snap(&catalog, &req), v.get("expected_error")) {
            (Err(e), Some(want)) => e.to_string().contains(
                want.as_str().unwrap().split(" for kind").next().unwrap(),
            ),
            (Ok(outcome), None) => {
                let got = match outcome {
                    SnapOutcome::Snapped(s) => json!({"snapped": true, "catalogSnap": s}),
                    SnapOutcome::NoSnap { nearest } => json!({"snapped": false, "nearest": nearest}),
                };
                matches(&v["expected"], &got) || {
                    eprintln!("{}: got {got}", v["id"]);
                    false
                }
            }
            (other, _) => {
                eprintln!("{}: got {other:?}", v["id"]);
                false
            }
        };
        if !ok {
            failures.push(v["id"].as_str().unwrap().to_owned());
        }
    }
    assert!(failures.is_empty(), "vectors failed: {failures:?}");
}
