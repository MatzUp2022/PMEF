//! Finding 02: ports from E3D arrive/leave points, connectedTo along the branch, implicit
//! tubes from the gaps, branch ends to nozzles and to a tee of another branch.

use pmef_adapter_e3d::{E3DAdapter, E3DConfig};
use pmef_validate::PmefValidator;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[tokio::test]
async fn branch_ports_are_connected_and_tubes_fill_the_gaps() {
    let out = std::env::temp_dir().join(format!("pmef-e3d-branch-{}.ndjson", std::process::id()));
    let mut adapter = E3DAdapter::new(E3DConfig {
        project_code: "test-proj".to_owned(),
        pml_export_path: here().join("tests/fixtures/branch-ports.pml"),
        ..Default::default()
    });
    adapter.export_to_pmef(out.to_str().unwrap()).await.unwrap();
    assert!(adapter.warnings().is_empty(), "{:?}", adapter.warnings());

    let text = std::fs::read_to_string(&out).unwrap();
    let objs: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let validator = PmefValidator::from_directory(&here().join("../../schemas")).unwrap();
    for o in &objs {
        let errs = validator.validate_object(o["@type"].as_str().unwrap(), o);
        assert!(errs.is_empty(), "{} {:?}", o["@id"], errs.iter().map(|e| &e.message).collect::<Vec<_>>());
    }
    let by_id: HashMap<&str, &Value> = objs.iter().map(|o| (o["@id"].as_str().unwrap(), o)).collect();

    // Every connectedTo resolves and is reciprocal at the same point.
    let mut links = 0;
    for o in &objs {
        for p in o["ports"].as_array().into_iter().flatten() {
            let Some(t) = p["connectedTo"].as_str() else { continue };
            let back = by_id[t]["ports"].as_array().unwrap().iter()
                .find(|q| q["connectedTo"] == o["@id"]).unwrap_or_else(|| panic!("{t} does not link back"));
            assert_eq!(back["coordinate"], p["coordinate"], "{} -> {t}", o["@id"]);
            links += 1;
        }
    }
    // B1: FLAN-TUBI-ELBO-TUBI-TEE-TUBI-VALV-TUBI = 7 joints (PSUP sits on a tube);
    // B2: TUBI-VALV = 1 joint, plus tee P3 to the B2 head tube; B3 (no components, one
    // tube) starts at the B2 tail and both refer to each other: 10 joints, seen from both sides.
    assert_eq!(links, 20);

    let tubes: Vec<f64> = objs.iter()
        .filter(|o| o["customAttributes"]["e3dGenerated"] == "TUBI")
        .map(|o| o["pipeLength"].as_f64().unwrap())
        .collect();
    assert_eq!(tubes, [600.0, 850.0, 200.0, 700.0, 150.0, 300.0]);

    let tee = objs.iter().find(|o| o["@type"] == "pmef:Tee").unwrap();
    assert_eq!(tee["ports"][2]["portId"], "P3");
    assert!(tee["ports"][2]["connectedTo"].is_string(), "tee branch joined to B2");

    // Branch ends at nozzles: nozzle knows its line and port, the end port stays free.
    let pump = objs.iter().find(|o| o["equipmentBasic"]["tagNumber"] == "P-101").unwrap();
    assert_eq!(pump["nozzles"][0]["connectedLineId"], "urn:pmef:line:test-proj:4-PW-100-A1A2");
    assert_eq!(pump["nozzles"][0]["connectedPortId"], "P1");
    let tank = objs.iter().find(|o| o["equipmentBasic"]["tagNumber"] == "T-102").unwrap();
    assert_eq!(tank["nozzles"][0]["connectedPortId"], "P2");

    // The support has no p-points: fallback port P0, not connected.
    let sup = objs.iter().find(|o| o["@type"] == "pmef:PipeSupport").unwrap();
    assert_eq!(sup["ports"][0]["portId"], "P0");
    assert!(sup["ports"][0].get("connectedTo").is_none());

    if std::env::var_os("PMEF_KEEP_OUTPUT").is_none() {
        std::fs::remove_file(out).ok();
    } else {
        eprintln!("output: {}", out.display());
    }
}

/// A branch end at a component without arrive/leave points stays open (spec 05 fallback):
/// its `P0` at `POS` is not a connection point.
#[tokio::test]
async fn branch_end_at_fallback_component_stays_open() {
    let dir = std::env::temp_dir().join(format!("pmef-e3d-fallback-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pml = dir.join("fallback.pml");
    std::fs::write(&pml, "\
SITE /S
  ZONE /S/Z
    PIPE /S/Z/P1
      LINREF 'L-1'
      BORE 50
      BRAN /S/Z/P1/B1
        HPOS 0.0 0.0 0.0
        TPOS 0.0 0.0 500.0
        VALV /S/Z/P1/B1/V1
          BORE 50
          POS 0.0 0.0 250.0
      BRAN /S/Z/P1/B2
        HPOS 0.0 100.0 250.0
        HREF /S/Z/P1/B1/V1
        TPOS 0.0 400.0 250.0
").unwrap();
    let out = dir.join("out.ndjson");
    let mut adapter = E3DAdapter::new(E3DConfig {
        project_code: "test-proj".to_owned(),
        pml_export_path: pml,
        ..Default::default()
    });
    adapter.export_to_pmef(out.to_str().unwrap()).await.unwrap();
    assert!(adapter.warnings().iter().any(|w| w.contains("no arrive/leave points")), "{:?}", adapter.warnings());
    let text = std::fs::read_to_string(&out).unwrap();
    let valve = text.lines().map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|o| o["@type"] == "pmef:Valve").unwrap();
    assert_eq!(valve["ports"][0]["portId"], "P0");
    assert!(valve["ports"][0].get("connectedTo").is_none());
}
