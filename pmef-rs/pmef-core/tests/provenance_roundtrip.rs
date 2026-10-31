//! RFC-0001 serde round trip: every object of examples/asbuilt-recognition.ndjson is
//! deserialised into its pmef-core struct and serialised back without loss.

use pmef_core::{
    Elbow, Pipe, PipeSupport, PipingNetworkSystem, PipingSegment, Plant, ReviewState, SteelMember, Unit,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// Numbers compare as f64 (Rust `f64` fields re-serialise `150` as `150.0`).
fn normalise(v: &Value) -> Value {
    match v {
        Value::Number(n) => serde_json::json!(n.as_f64().unwrap()),
        Value::Array(a) => Value::Array(a.iter().map(normalise).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), normalise(v))).collect()),
        other => other.clone(),
    }
}

fn round_trip<T: DeserializeOwned + Serialize>(v: &Value) -> (T, Value) {
    let t: T = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{}: {e}", v["@id"]));
    let back = serde_json::to_value(&t).unwrap();
    (t, back)
}

#[test]
fn asbuilt_example_round_trips_through_core_types() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/asbuilt-recognition.ndjson");
    let text = std::fs::read_to_string(path).unwrap();
    let mut checked = 0;
    for line in text.lines().filter(|l| l.starts_with('{')) {
        let v: Value = serde_json::from_str(line).unwrap();
        let back = match v["@type"].as_str().unwrap() {
            "pmef:Plant" => round_trip::<Plant>(&v).1,
            "pmef:Unit" => round_trip::<Unit>(&v).1,
            "pmef:PipingNetworkSystem" => round_trip::<PipingNetworkSystem>(&v).1,
            "pmef:PipingSegment" => round_trip::<PipingSegment>(&v).1,
            "pmef:Pipe" => round_trip::<Pipe>(&v).1,
            "pmef:Elbow" => round_trip::<Elbow>(&v).1,
            "pmef:PipeSupport" => round_trip::<PipeSupport>(&v).1,
            "pmef:SteelMember" => {
                let (m, back) = round_trip::<SteelMember>(&v);
                let snap = m.recognition.as_built.as_ref().unwrap().catalog_snap.as_ref().unwrap();
                assert_eq!(snap.catalog_ref.entry_id, "EN:HEB300");
                assert_eq!(m.recognition.review.as_ref().unwrap().status, ReviewState::Approved);
                back
            }
            "pmef:FileHeader" => continue,
            other => panic!("unexpected type {other}"),
        };
        for block in ["provenance", "review", "asBuilt"] {
            assert_eq!(back.get(block).map(normalise), v.get(block).map(normalise), "{} {block} changed", v["@id"]);
        }
        assert_eq!(normalise(&back), normalise(&v), "{} not lossless", v["@id"]);
        checked += 1;
    }
    assert_eq!(checked, 9);
}

#[test]
fn objects_without_blocks_serialise_unchanged() {
    let v = serde_json::json!({"@type": "pmef:Plant", "@id": "urn:pmef:plant:demo:P", "pmefVersion": "0.9.0",
                               "name": "P"});
    let (p, back) = round_trip::<Plant>(&v);
    assert!(p.recognition.is_empty());
    assert_eq!(back, v);
}
