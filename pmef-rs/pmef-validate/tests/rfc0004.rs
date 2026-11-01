//! RFC-0004: `polytropicEfficiency` (camelCase); `polytropic_efficiency` deprecated in 0.10.

use pmef_validate::PmefValidator;
use serde_json::{json, Value};
use std::path::PathBuf;

fn validator() -> PmefValidator {
    PmefValidator::from_directory(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas")).unwrap()
}

fn compressor(spec: Value) -> Value {
    json!({"@type": "pmef:Compressor", "@id": "urn:pmef:obj:t:K-1", "pmefVersion": "0.10.0",
           "isPartOf": "urn:pmef:unit:t:U-1",
           "equipmentBasic": {"tagNumber": "K-1", "equipmentClass": "CENTRIFUGAL_COMPRESSOR"},
           "compressorSpec": spec})
}

#[test]
fn new_spelling_is_valid() {
    assert!(validator().validate_object("pmef:Compressor", &compressor(json!({"polytropicEfficiency": 82.0}))).is_empty());
}

#[test]
fn deprecated_spelling_is_still_valid_in_0_10() {
    assert!(validator().validate_object("pmef:Compressor", &compressor(json!({"polytropic_efficiency": 82.0}))).is_empty());
}

#[test]
fn both_spellings_together_are_invalid() {
    let o = compressor(json!({"polytropicEfficiency": 82.0, "polytropic_efficiency": 82.0}));
    assert!(!validator().validate_object("pmef:Compressor", &o).is_empty());
}
