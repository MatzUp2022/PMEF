//! RFC-0001: provenance / review / asBuilt blocks.
//! Positive: examples/asbuilt-recognition.ndjson is valid.
//! Negative: each normative rule rejects a violating object.

use pmef_validate::PmefValidator;
use serde_json::Value;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn validator() -> PmefValidator {
    PmefValidator::from_directory(&root().join("schemas")).unwrap()
}

fn example() -> Vec<Value> {
    std::fs::read_to_string(root().join("examples/asbuilt-recognition.ndjson"))
        .unwrap()
        .lines()
        .filter(|l| l.starts_with('{'))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn find(objs: &[Value], id_suffix: &str) -> Value {
    objs.iter().find(|o| o["@id"].as_str().unwrap().ends_with(id_suffix)).unwrap().clone()
}

fn errors(v: &PmefValidator, o: &Value) -> usize {
    v.validate_object(o["@type"].as_str().unwrap(), o).len()
}

#[test]
fn example_is_valid() {
    let v = validator();
    for o in example() {
        assert_eq!(errors(&v, &o), 0, "{} should be valid", o["@id"]);
    }
}

#[test]
fn normative_rules_reject_violations() {
    let v = validator();
    let objs = example();
    let pipe = find(&objs, "CW-301-PIPE-001");
    let beam = find(&objs, ":B-101");

    let cases: Vec<(&str, Value)> = vec![
        ("confidence > 1", { let mut o = pipe.clone(); o["provenance"]["confidence"] = 1.2.into(); o }),
        ("recognizer without confidence", { let mut o = pipe.clone(); o["provenance"].as_object_mut().unwrap().remove("confidence"); o }),
        ("scan without sourceRef", { let mut o = pipe.clone(); o["provenance"].as_object_mut().unwrap().remove("sourceRef"); o }),
        ("unknown sourceType", { let mut o = pipe.clone(); o["provenance"]["sourceType"] = "lidar".into(); o }),
        ("approved without reviewedBy", { let mut o = pipe.clone(); o["review"].as_object_mut().unwrap().remove("reviewedBy"); o }),
        ("detected but SHARED", { let mut o = pipe.clone(); o["review"] = serde_json::json!({"status": "detected"}); o }),
        ("rejected but PUBLISHED", { let mut o = beam.clone(); o["review"]["status"] = "rejected".into(); o }),
        ("negative deviation", { let mut o = pipe.clone(); o["asBuilt"]["deviationRms_mm"] = (-1.0).into(); o }),
        ("snap without catalogVersion", { let mut o = beam.clone(); o["asBuilt"]["catalogSnap"]["catalogRef"].as_object_mut().unwrap().remove("catalogVersion"); o }),
        ("unknown field in provenance", { let mut o = pipe.clone(); o["provenance"]["foo"] = 1.into(); o }),
    ];
    for (name, o) in cases {
        assert!(errors(&v, &o) > 0, "case '{name}' must be rejected");
    }

    // `manual` objects need no sourceRef.
    let mut manual = pipe.clone();
    manual["provenance"] = serde_json::json!({"sourceType": "manual"});
    assert_eq!(errors(&v, &manual), 0);
}
