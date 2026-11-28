//! spec 02 §10.1: customAttributes on hierarchy, system and segment objects survive a serde round
//! trip through the pmef-core types (examples/tool-attributes-e3d.ndjson).

use pmef_core::{Area, DuctSegment, PipingSegment, SteelSystem, Unit};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::path::PathBuf;

fn objects() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/tool-attributes-e3d.ndjson");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn check<T: DeserializeOwned + Serialize>(objs: &[Value], ty: &str) {
    let v = objs.iter().find(|o| o["@type"] == ty).unwrap_or_else(|| panic!("{ty} missing"));
    let t: T = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{ty}: {e}"));
    let back = serde_json::to_value(&t).unwrap();
    assert!(v["customAttributes"].is_object(), "{ty} example has customAttributes");
    assert_eq!(back["customAttributes"], v["customAttributes"], "{ty}");
}

#[test]
fn custom_attributes_round_trip_on_hierarchy_system_and_segment_objects() {
    let objs = objects();
    check::<Unit>(&objs, "pmef:Unit");
    check::<Area>(&objs, "pmef:Area");
    check::<PipingSegment>(&objs, "pmef:PipingSegment");
    check::<SteelSystem>(&objs, "pmef:SteelSystem");
    check::<DuctSegment>(&objs, "pmef:DuctSegment");
}
