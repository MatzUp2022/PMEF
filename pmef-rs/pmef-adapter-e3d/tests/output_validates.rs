//! Every object written by the E3D adapter must validate against the normative schemas
//! (docs/AUDIT.md P1.7). Covers equipment (with nozzles and design primitives), a piping line
//! with its components, hierarchy and HasEquivalentIn relationships.

use pmef_adapter_e3d::{E3DAdapter, E3DConfig};
use pmef_validate::PmefValidator;
use std::path::PathBuf;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[tokio::test]
async fn all_exported_objects_are_schema_valid() {
    let dir = std::env::temp_dir().join(format!("pmef-e3d-all-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.ndjson");
    let mut adapter = E3DAdapter::new(E3DConfig {
        project_code: "test-proj".to_owned(),
        pml_export_path: here().join("tests/fixtures/plant.pml"),
        ..Default::default()
    });
    adapter.export_to_pmef(out.to_str().unwrap()).await.unwrap();

    let validator = PmefValidator::from_directory(&here().join("../../schemas")).unwrap();
    let text = std::fs::read_to_string(&out).unwrap();
    let mut failures = Vec::new();
    let mut types = std::collections::BTreeSet::new();
    for line in text.lines() {
        let o: serde_json::Value = serde_json::from_str(line).unwrap();
        let t = o["@type"].as_str().unwrap().to_owned();
        types.insert(t.clone());
        for e in validator.validate_object(&t, &o) {
            failures.push(format!("{} {}: {} — {}", t, o["@id"], e.instance_path, e.message));
        }
    }
    assert!(failures.is_empty(), "{} schema error(s):\n{}", failures.len(), failures.join("\n"));
    for t in ["pmef:FileHeader", "pmef:Plant", "pmef:Unit", "pmef:Vessel", "pmef:Pump", "pmef:Tank",
              "pmef:PipingNetworkSystem", "pmef:PipingSegment", "pmef:Pipe", "pmef:Elbow", "pmef:Flange",
              "pmef:Valve", "pmef:GenericPipingComponent", "pmef:PipeSupport", "pmef:ParametricGeometry", "pmef:HasEquivalentIn"] {
        assert!(types.contains(t), "fixture should produce {t}; got {types:?}");
    }
    // Finding 01: element types without a specific PMEF type are kept, with their E3D keyword.
    let generic: Vec<(String, String)> = text
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .filter(|o| o["@type"] == "pmef:GenericPipingComponent")
        .map(|o| (o["nativeType"].as_str().unwrap().to_owned(),
                  o["componentSpec"]["componentClass"].as_str().unwrap().to_owned()))
        .collect();
    assert_eq!(generic, [("E3D:COUP".into(), "COUPLING".into()),
                         ("E3D:INST".into(), "INLINE_INSTRUMENT".into()),
                         ("E3D:CLOS".into(), "CLOSURE".into()),
                         ("E3D:XYZZ".into(), "SPECIAL".into())]);
    let blind = text.lines().map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|o| o["componentSpec"]["componentClass"] == "BLIND_FLANGE").expect("FBLI exported");
    assert_eq!(blind["flangeType"], "BLIND");
    std::fs::remove_dir_all(dir).ok();
}
