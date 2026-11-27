//! `Gasket.bondingJumper`: read from the reference example and written back unchanged.

use pmef_core::piping::Gasket;

#[test]
fn bonding_jumper_round_trips() {
    let text = include_str!("../../../examples/pump-skid-complete.ndjson");
    let gaskets: Vec<serde_json::Value> = text
        .lines()
        .filter(|l| l.contains("\"pmef:Gasket\""))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(gaskets.len(), 2);
    for v in gaskets {
        let g: Gasket = serde_json::from_value(v.clone()).unwrap();
        let bonded = v["@id"].as_str().unwrap().ends_with("CW-202-GK-001");
        assert_eq!(g.bonding_jumper, bonded.then_some(true));
        let out = serde_json::to_value(&g).unwrap();
        assert_eq!(out.get("bondingJumper"), v.get("bondingJumper"));
    }
}
