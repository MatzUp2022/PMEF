use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn every_catalog_file_is_embedded() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs");
    let on_disk: BTreeSet<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();
    let embedded: BTreeSet<String> = pmef_catalogs::ids().map(str::to_string).collect();
    assert_eq!(
        on_disk, embedded,
        "add new catalogs to CATALOGS in src/lib.rs"
    );
}

#[test]
fn every_catalog_parses_and_carries_its_id() {
    for id in pmef_catalogs::ids() {
        let doc = pmef_catalogs::document(id).unwrap();
        assert_eq!(doc["catalogId"].as_str(), Some(id), "catalogId of {id}");
    }
}

#[test]
fn pipe_sizes_map_nps_and_dn() {
    assert_eq!(pmef_catalogs::nps_to_dn(0.5), Some(15));
    assert_eq!(pmef_catalogs::nps_to_dn(1.25), Some(32));
    assert_eq!(pmef_catalogs::nps_to_dn(48.0), Some(1200));
    assert_eq!(pmef_catalogs::dn_to_nps(50), Some(2.0));
    assert_eq!(pmef_catalogs::nps_to_dn(7.0), None);
    let sizes = pmef_catalogs::pipe_sizes();
    assert!(sizes.windows(2).all(|w| w[0].dn < w[1].dn), "ascending");
}

#[test]
fn parses_nps_designations() {
    assert_eq!(pmef_catalogs::parse_nps("1/2"), Some(0.5));
    assert_eq!(pmef_catalogs::parse_nps("1-1/4"), Some(1.25));
    assert_eq!(pmef_catalogs::parse_nps("2\""), Some(2.0));
    assert_eq!(pmef_catalogs::parse_nps("2.5"), Some(2.5));
    assert_eq!(pmef_catalogs::parse_nps("abc"), None);
}

#[test]
fn duct_sizes_round_and_rectangular() {
    use pmef_catalogs::{duct_sizes, next_duct_size, DuctSeries};
    let round = duct_sizes(DuctSeries::Round);
    assert!(round.windows(2).all(|w| w[0].size_mm < w[1].size_mm));
    for e in &round {
        let a = std::f64::consts::PI * (e.size_mm / 1000.0).powi(2) / 4.0;
        assert!((e.area_m2.unwrap() - a).abs() < 1e-6, "{}", e.id);
    }
    assert_eq!(next_duct_size(DuctSeries::Round, 380.0), Some(400.0));
    assert_eq!(next_duct_size(DuctSeries::Rectangular, 350.0), Some(400.0));
    assert_eq!(next_duct_size(DuctSeries::Round, 5000.0), None);
}

#[test]
fn pipe_supports_by_dn_and_restraint() {
    let guides = pmef_catalogs::supports_for(100, &["VERTICAL_DOWN", "LATERAL"]);
    assert!(guides.iter().any(|s| s.support_code == "GS-SHOE"));
    assert!(!guides.iter().any(|s| s.support_code == "RS-SHOE"));
    let anchors = pmef_catalogs::supports_for(200, &["AXIAL", "ROTATION"]);
    assert_eq!(anchors.len(), 1);
    assert_eq!(anchors[0].support_type, "ANCHOR");
    assert!(pmef_catalogs::supports_for(10, &[]).is_empty());
}

#[test]
fn cable_types_carry_bend_radius() {
    let c = pmef_catalogs::cable_type("NYY-J 5x2.5").unwrap();
    assert_eq!(c.number_of_cores, 5);
    assert_eq!(c.min_bend_radius_mm(), Some(14.0 * 12.0));
    assert!(pmef_catalogs::cable_type("NOPE").is_none());
    assert!(pmef_catalogs::cable_types().iter().all(|c| c.min_bend_radius_mm().is_some()));
}

#[test]
fn e3d_native_types_map_skey_variants_first() {
    let m = |kw, skey| pmef_catalogs::native_type("native-types-e3d", kw, skey).unwrap();
    assert_eq!(m("VALV", Some("bfwafl")).component_class.as_deref(), Some("VALVE_BUTTERFLY"));
    assert_eq!(m("VALV", None).component_class.as_deref(), Some("VALVE_UNSPECIFIED"));
    assert_eq!(m("INST", None).pmef_type, "pmef:GenericPipingComponent");
    assert_eq!(m("ATTA", None).pmef_type, "pmef:PipeSupport");
    assert_eq!(m("ATTA", None).component_class.as_deref(), Some("PIPE_SUPPORT"));
    for kw in ["REST", "HANG", "PCLA", "SHU"] {
        assert_eq!(m(kw, None).pmef_type, "pmef:PipeSupport", "{kw}");
    }
    assert!(pmef_catalogs::native_type("native-types-e3d", "NOPE", None).is_none());
}
