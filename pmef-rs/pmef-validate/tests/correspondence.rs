//! RFC-0003: `correspondence` on `pmef:IsRevisionOf` — schema rules.

use pmef_validate::PmefValidator;
use serde_json::{json, Value};
use std::path::PathBuf;

fn validator() -> PmefValidator {
    PmefValidator::from_directory(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas")).unwrap()
}

fn rel() -> Value {
    json!({"@type": "pmef:IsRevisionOf", "@id": "urn:pmef:rel:scan:R-1-asbuilt", "relationType": "IS_REVISION_OF",
           "sourceId": "urn:pmef:obj:scan:R-1", "targetId": "urn:pmef:obj:dsn:B-101", "changeType": "AS_BUILT",
           "correspondence": {"method": "geometry", "distance_mm": 12.5, "ambiguous": false},
           "review": {"status": "detected"},
           "revision": {"revisionId": "r2026-10-31-001", "changeState": "WIP"}})
}

#[test]
fn correspondence_rules() {
    let v = validator();
    let errs = |o: &Value| v.validate_object("pmef:IsRevisionOf", o).len();
    assert_eq!(errs(&rel()), 0);

    let mut wrong_type = rel();
    wrong_type["changeType"] = "MINOR".into();
    assert!(errs(&wrong_type) > 0, "correspondence requires changeType AS_BUILT");

    let mut no_type = rel();
    no_type.as_object_mut().unwrap().remove("changeType");
    assert!(errs(&no_type) > 0, "correspondence requires changeType");

    let mut bad_method = rel();
    bad_method["correspondence"]["method"] = "guess".into();
    assert!(errs(&bad_method) > 0);

    let mut negative = rel();
    negative["correspondence"]["distance_mm"] = (-1.0).into();
    assert!(errs(&negative) > 0);

    let mut plain = rel();
    plain.as_object_mut().unwrap().remove("correspondence");
    plain["changeType"] = "MINOR".into();
    assert_eq!(errs(&plain), 0, "plain revisions are unaffected");
}
