//! STRU → FRMW → SBFR, SCTN and PANE map to nested SteelSystems, SteelMembers and SteelPlates
//! (RFC-0013), and every object validates against the schemas.

use pmef_adapter_e3d::{E3DAdapter, E3DConfig};
use pmef_validate::PmefValidator;
use serde_json::Value;
use std::path::PathBuf;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[tokio::test]
async fn structure_frameworks_sections_and_panels() {
    let dir = std::env::temp_dir().join(format!("pmef-e3d-steel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.ndjson");
    let mut adapter = E3DAdapter::new(E3DConfig {
        project_code: "huldra".to_owned(),
        pml_export_path: here().join("tests/fixtures/steel.pml"),
        ..Default::default()
    });
    adapter.export_to_pmef(out.to_str().unwrap()).await.unwrap();
    let objs: Vec<Value> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let validator = PmefValidator::from_directory(&here().join("../../schemas")).unwrap();
    for o in &objs {
        let errs = validator.validate_object(o["@type"].as_str().unwrap(), o);
        assert!(errs.is_empty(), "{}: {:?}", o["@id"], errs.iter().map(|e| e.message.clone()).collect::<Vec<_>>());
    }
    let by_id = |id: &str| objs.iter().find(|o| o["@id"] == id).unwrap_or_else(|| panic!("{id} missing"));
    let id = |l: &str| format!("urn:pmef:obj:huldra:{l}");

    // hierarchy STRU > FRMW > SBFR
    assert_eq!(by_id(&id("STRU-P300"))["systemLevel"], "STRUCTURE");
    assert_eq!(by_id(&id("FRMW-DECK"))["isPartOf"], id("STRU-P300"));
    assert_eq!(by_id(&id("SBFR-HR"))["systemLevel"], "SUBFRAMEWORK");
    assert_eq!(by_id(&id("SBFR-HR"))["isPartOf"], id("FRMW-DECK"));

    // members: shared and project profiles, roll angle
    assert_eq!(by_id(&id("B-301"))["profileId"], "EN:HEA200");
    let st = by_id(&id("ST-306"));
    assert_eq!((st["profileId"].as_str(), st["rollAngle"].as_f64()), (Some("HULDRA:HP260x10"), Some(90.0)));
    assert_eq!(by_id(&id("HR-320"))["profileId"], "EN:RB20");
    assert_eq!(by_id(&id("FRMW-DECK"))["memberIds"].as_array().unwrap().len(), 2, "SCTN without POSS/POSE skipped");
    assert!(adapter.warnings().iter().any(|w| w.contains("BAD")));

    // panel
    let pl = by_id(&id("PL-310"));
    assert_eq!(pl["plateType"], "GRATING");
    assert_eq!(pl["thickness"], 30.0);
    assert_eq!(pl["normal"], serde_json::json!([0.0, 0.0, 1.0]));
    assert_eq!(pl["outline"].as_array().unwrap().len(), 4);
    assert_eq!(by_id(&id("FRMW-DECK"))["plateIds"], serde_json::json!([id("PL-310")]));
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn same_last_name_under_different_parents_keeps_distinct_ids() {
    let dir = std::env::temp_dir().join(format!("pmef-e3d-steel-dup-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.ndjson");
    let mut adapter = E3DAdapter::new(E3DConfig {
        project_code: "huldra".to_owned(),
        pml_export_path: here().join("tests/fixtures/steel-dup-names.pml"),
        ..Default::default()
    });
    adapter.export_to_pmef(out.to_str().unwrap()).await.unwrap();
    let objs: Vec<Value> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut ids: Vec<&str> = objs.iter().filter_map(|o| o["@id"].as_str()).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "duplicate @id: {ids:?}");
    let a = "urn:pmef:obj:huldra:FRAME";
    let b = "urn:pmef:obj:huldra:SITE01.STR-B.FRAME";
    let get = |id: &str| objs.iter().find(|o| o["@id"] == id).unwrap_or_else(|| panic!("{id} missing"));
    assert_eq!(get(a)["isPartOf"], "urn:pmef:obj:huldra:STR-A");
    assert_eq!(get(b)["isPartOf"], "urn:pmef:obj:huldra:STR-B");
    assert_eq!(get("urn:pmef:obj:huldra:B-2")["isPartOf"], b);
}
