//! RFC-0004: the Rust model reads the deprecated `polytropic_efficiency` and writes
//! `polytropicEfficiency`; both spellings in one object are rejected.

use pmef_core::equipment::CompressorSpec;
use serde_json::json;

#[test]
fn reads_old_and_writes_new_spelling() {
    let spec: CompressorSpec = serde_json::from_value(json!({"polytropic_efficiency": 82.0})).unwrap();
    let out = serde_json::to_value(&spec).unwrap();
    assert_eq!(out["polytropicEfficiency"], json!(82.0));
    assert!(out.get("polytropic_efficiency").is_none());
}

#[test]
fn round_trip_of_new_spelling() {
    let spec: CompressorSpec = serde_json::from_value(json!({"polytropicEfficiency": 80.5})).unwrap();
    assert_eq!(serde_json::to_value(&spec).unwrap(), json!({"polytropicEfficiency": 80.5}));
}

#[test]
fn both_spellings_are_rejected() {
    let r: Result<CompressorSpec, _> =
        serde_json::from_value(json!({"polytropicEfficiency": 82.0, "polytropic_efficiency": 82.0}));
    assert!(r.is_err());
}
