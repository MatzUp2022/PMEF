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
