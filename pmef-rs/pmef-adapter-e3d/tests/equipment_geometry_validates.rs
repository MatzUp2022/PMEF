//! End-to-end: synthetic PML export → PMEF NDJSON → validated against the
//! normative schemas in `schemas/`. Covers the PML design-primitive geometry
//! path that replaced the former RVM parser.
//!
//! Only the geometry objects are checked here; `output_validates.rs` validates
//! every object the adapter writes.

use pmef_adapter_e3d::{E3DAdapter, E3DConfig};
use pmef_validate::PmefValidator;
use std::path::PathBuf;

/// Synthetic vessel (shell + two heads) and a pump with a box skid. CC0, no real project data.
const PML: &str = "\
EQUI /SITE01/ZONE-U100/EQUI-V-201
  TAG 'V-201'
  DTYP 'VESSEL'
  DESC 'Synthetic test vessel'
  CYLI
    NAME shell
    POS 11500.0 5400.0 2200.0
    AXIS 0.0 0.0 1.0
    DIAM 1224
    HEIG 2200
  DISH
    NAME top_head
    POS 11500.0 5400.0 3300.0
    AXIS 0.0 0.0 1.0
    DIAM 1224
    HEIG 306
  DISH
    NAME bottom_head
    POS 11500.0 5400.0 1100.0
    AXIS 0.0 0.0 -1.0
    DIAM 1224
    HEIG 306
EQUI /SITE01/ZONE-U100/EQUI-P-201A
  TAG 'P-201A'
  DTYP 'CENTRIFUGALPUMP'
  BOX
    NAME skid
    POS 10200.0 5400.0 300.0
    XLEN 1800
    YLEN 800
    ZLEN 600
EQUI /SITE01/ZONE-U100/EQUI-T-1
  TAG 'T-1'
  DTYP 'TANK'
";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[tokio::test]
async fn equipment_geometry_export_is_schema_valid() {
    let dir = std::env::temp_dir().join(format!("pmef-e3d-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pml_path = dir.join("plant.pml");
    let out_path = dir.join("out.ndjson");
    std::fs::write(&pml_path, PML).unwrap();

    let mut adapter = E3DAdapter::new(E3DConfig {
        project_code: "test-proj".to_owned(),
        pml_export_path: pml_path,
        ..Default::default()
    });
    adapter.export_to_pmef(out_path.to_str().unwrap()).await.unwrap();
    assert!(adapter.warnings().is_empty(), "unexpected warnings: {:?}", adapter.warnings());

    let validator = PmefValidator::from_directory(&repo_root().join("schemas")).unwrap();
    let text = std::fs::read_to_string(&out_path).unwrap();
    let objs: Vec<serde_json::Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();

    let mut geom = 0;
    for obj in &objs {
        let t = obj["@type"].as_str().unwrap();
        if t == "pmef:ParametricGeometry" {
            let errors = validator.validate_object(t, obj);
            assert!(errors.is_empty(), "{} invalid: {:?}", obj["@id"], errors);
            geom += 1;
            let owner = obj["ownerRef"].as_str().unwrap();
            let eq = objs.iter().find(|o| o["@id"] == owner).expect("owner exists");
            assert_eq!(eq["geometry"]["type"], "parametric");
            assert_eq!(eq["geometry"]["ref"], obj["@id"]);
        }
    }
    assert_eq!(geom, 2, "vessel + pump have geometry, tank has none");
    let tank = objs.iter().find(|o| o["equipmentBasic"]["tagNumber"] == "T-1").unwrap();
    assert_eq!(tank["geometry"]["type"], "none");

    std::fs::remove_dir_all(&dir).ok();
}
