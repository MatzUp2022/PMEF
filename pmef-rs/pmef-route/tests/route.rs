use pmef_route::{pmef_out, route_all, RouteProblem, Scene};
use serde_json::json;

fn problem(extra: serde_json::Value) -> RouteProblem {
    let mut base = json!({
        "project": "t", "revisionId": "r2026-11-25-001", "parent": "urn:pmef:unit:t:U1",
        "grid": {"step": 100, "margin": 1500},
        "routes": [{
            "id": "L1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [5000, 3000, 0], "direction": [0, -1, 0]}
        }]
    });
    if let serde_json::Value::Object(m) = extra {
        for (k, v) in m {
            base[k] = v;
        }
    }
    serde_json::from_value(base).unwrap()
}

fn scene(p: &RouteProblem) -> Scene {
    Scene::load(p, std::path::Path::new(".")).unwrap()
}

#[test]
fn l_shape_one_bend() {
    let p = problem(json!({}));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok, "{:?}", r[0].error);
    assert_eq!(r[0].bends, 1);
    assert_eq!(
        r[0].points,
        vec![[0.0, 0.0, 0.0], [5000.0, 0.0, 0.0], [5000.0, 3000.0, 0.0]]
    );
    assert_eq!(r[0].length_mm, 8000.0);
}

#[test]
fn avoids_box_with_clearance() {
    let p = problem(
        json!({"obstacles": [{"id": "E1", "aabb": {"min": [2000, -1000, -1000], "max": [3000, 1000, 1000]}}]}),
    );
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok, "{:?}", r[0].error);
    let reach = 114.3 / 2.0 + 50.0;
    for w in r[0].points.windows(2) {
        // Every run keeps the outer radius + clearance away from the box.
        let lo = [0, 1, 2].map(|i| w[0][i].min(w[1][i]));
        let hi = [0, 1, 2].map(|i| w[0][i].max(w[1][i]));
        let min = [2000.0 - reach, -1000.0 - reach, -1000.0 - reach];
        let max = [3000.0 + reach, 1000.0 + reach, 1000.0 + reach];
        assert!(
            (0..3).any(|i| hi[i] < min[i] || lo[i] > max[i]),
            "run {w:?} hits the box"
        );
    }
}

#[test]
fn elbows_fit_between_bends() {
    // Offset of 200 mm sideways: a Z-bend needs >= 2 x 152 mm between the two elbows.
    let p = problem(json!({"routes": [{
        "id": "L1", "pipeClass": "A1A2", "dn": 100,
        "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
        "to": {"point": [3000, 200, 0], "direction": [1, 0, 0]}
    }]}));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok, "{:?}", r[0].error);
    let pts = &r[0].points;
    for w in pts.windows(2).skip(1).take(pts.len().saturating_sub(3)) {
        let len: f64 = (0..3).map(|i| (w[1][i] - w[0][i]).abs()).sum();
        assert!(len >= 304.0 - 1e-6, "run between bends {len} < 2 x c2f");
    }
    assert!(r[0].bends >= 2);
}

#[test]
fn wall_is_crossed_straight_with_sleeve() {
    let p = problem(json!({
        "obstacles": [{"id": "W1", "kind": "soft", "aabb": {"min": [2500, -5000, -5000], "max": [2700, 8000, 5000]}}]
    }));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok, "{:?}", r[0].error);
    assert_eq!(r[0].penetrations, 1);
}

#[test]
fn closed_box_has_no_route() {
    let p = problem(json!({
        "bounds": {"min": [-500, -500, -500], "max": [5500, 3500, 500]},
        "obstacles": [{"id": "W", "aabb": {"min": [3000, -2000, -2000], "max": [3200, 5000, 2000]}}]
    }));
    let r = route_all(&p, &scene(&p));
    assert!(!r[0].ok);
}

#[test]
fn class_rules_reject_dn_and_pressure() {
    let p = problem(json!({"routes": [
        {"id": "A", "pipeClass": "C5D1", "dn": 600, "from": {"point": [0,0,0], "direction": [1,0,0]}, "to": {"point": [1000,0,0], "direction": [-1,0,0]}},
        {"id": "B", "pipeClass": "A1A2", "dn": 100, "designPressure": 4.0e6, "from": {"point": [0,0,0], "direction": [1,0,0]}, "to": {"point": [1000,0,0], "direction": [-1,0,0]}}
    ]}));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].error.as_deref().unwrap().contains("DN 600"));
    assert!(r[1].error.as_deref().unwrap().contains("pressure"));
}

#[test]
fn second_line_avoids_first() {
    let p = problem(json!({"routes": [
        {"id": "A", "pipeClass": "A1A2", "dn": 100, "from": {"point": [0,0,0], "direction": [1,0,0]}, "to": {"point": [4000,0,0], "direction": [-1,0,0]}},
        {"id": "B", "pipeClass": "A1A2", "dn": 50, "from": {"point": [2000,-1500,0], "direction": [0,1,0]}, "to": {"point": [2000,1500,0], "direction": [0,-1,0]}}
    ]}));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok && r[1].ok);
    assert_eq!(r[0].bends, 0);
    assert!(r[1].bends >= 2, "B must leave the plane of A to cross it");
}

#[test]
fn pmef_output_chains_ports() {
    let p = problem(json!({}));
    let r = route_all(&p, &scene(&p));
    let objs = pmef_out::to_pmef(&p, &r);
    let types: Vec<&str> = objs.iter().map(|o| o["@type"].as_str().unwrap()).collect();
    assert_eq!(
        types,
        [
            "pmef:PipingNetworkSystem",
            "pmef:PipingSegment",
            "pmef:Pipe",
            "pmef:Elbow",
            "pmef:Pipe"
        ]
    );
    let elbow = &objs[3];
    assert_eq!(elbow["catalogRef"]["catalogId"], "A1A2-ELB90LR-100");
    assert_eq!(elbow["ports"][0]["coordinate"], json!([4848.0, 0.0, 0.0]));
    assert_eq!(elbow["ports"][1]["coordinate"], json!([5000.0, 152.0, 0.0]));
    assert_eq!(objs[2]["pipeLength"], 4848.0);
    assert_eq!(objs[4]["pipeLength"], 2848.0);
    assert_eq!(objs[2]["ports"][1]["connectedTo"], elbow["@id"]);
    assert_eq!(objs[4]["provenance"]["sourceType"], "generated");
}

/// `examples/pump-skid-auto-routed.ndjson` is exactly what the router produces for
/// `tests/route/pump-skid-route.json` (regenerate with `pmef route … --plant …` after changes).
#[test]
fn pump_skid_example_is_current() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("tests/route");
    let p: RouteProblem =
        serde_json::from_str(&std::fs::read_to_string(dir.join("pump-skid-route.json")).unwrap())
            .unwrap();
    let s = Scene::load(&p, &dir).unwrap();
    let r = route_all(&p, &s);
    assert!(r.iter().all(|x| x.ok), "{r:?}");
    let got: Vec<serde_json::Value> = pmef_out::to_pmef(&p, &r);
    let text = std::fs::read_to_string(root.join("examples/pump-skid-auto-routed.ndjson")).unwrap();
    let want: Vec<serde_json::Value> = text
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(got, want);
}

#[test]
fn avoidance_zone_is_bypassed() {
    // Factor 10 on the direct corridor: the route takes a detour instead of crossing it.
    let p = problem(json!({"zones": [
        {"aabb": {"min": [1000, -300, -300], "max": [4000, 300, 300]}, "factor": 10.0}
    ]}));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok, "{:?}", r[0].error);
    for w in r[0].points.windows(2) {
        let on_axis = w[0][1].abs() < 300.0 && w[1][1].abs() < 300.0 && w[0][2].abs() < 300.0;
        let spans = w[0][0].min(w[1][0]) < 2000.0 && w[0][0].max(w[1][0]) > 3000.0;
        assert!(!(on_axis && spans), "run {w:?} crosses the avoidance zone");
    }
    assert!(r[0].bends > 1);
}

#[test]
fn routes_stay_inside_bounds() {
    let p = problem(json!({"bounds": {"min": [-550, -550, -550], "max": [5050, 3050, 550]}}));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok, "{:?}", r[0].error);
    for q in &r[0].points {
        assert!(
            q[0] <= 5050.0 && q[1] <= 3050.0 && q[2] <= 550.0,
            "{q:?} outside bounds"
        );
    }
}

#[test]
fn terminal_stub_only_ignores_its_owner() {
    // A thin hard wall right in front of a point terminal blocks it ...
    let wall =
        json!([{"id": "W1", "aabb": {"min": [100, -1000, -1000], "max": [110, 1000, 1000]}}]);
    let p = problem(json!({"obstacles": wall}));
    let r = route_all(&p, &scene(&p));
    assert!(
        !r[0].ok,
        "route passed through a foreign obstacle: {:?}",
        r[0].points
    );
    // ... unless the terminal belongs to it (fixture, equipment).
    let p = problem(json!({"obstacles": wall, "routes": [{
        "id": "L1", "pipeClass": "A1A2", "dn": 100,
        "from": {"point": [0, 0, 0], "direction": [1, 0, 0], "owner": "W1"},
        "to": {"point": [5000, 3000, 0], "direction": [0, -1, 0]}
    }]}));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok, "{:?}", r[0].error);
}

#[test]
fn schema_primitives_are_obstacles() {
    use pmef_route::scene::primitive_box;
    let b = primitive_box(
        &json!({"type": "BOX", "center": [0, 0, 0], "xLen": 400, "yLen": 800, "zLen": 600}),
    )
    .unwrap();
    assert_eq!(b.min, [-200.0, -400.0, -300.0]);
    assert_eq!(b.max, [200.0, 400.0, 300.0]);
    let c = primitive_box(&json!({"type": "CONE", "center": [0, 0, 0], "axis": [0, 0, 1], "r1": 300, "r2": 100, "length": 500})).unwrap();
    assert_eq!(c.max, [300.0, 300.0, 500.0]);
    assert!(primitive_box(&json!({"type": "SNOUT", "center": [0, 0, 0], "axis": [1, 0, 0], "r1": 100, "r2": 50, "length": 200})).is_some());
    assert!(primitive_box(&json!({"type": "CIRC_TORUS", "center": [0, 0, 0], "axis": [0, 1, 0], "torusRadius": 300, "tubeRadius": 100, "angleDeg": 90})).is_some());
}

fn rack_problem(racks: serde_json::Value) -> RouteProblem {
    problem(json!({
        "bounds": {"min": [-1500, -1500, -500], "max": [21500, 1500, 5000]},
        "routes": [{
            "id": "L1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [0, 0, 1]},
            "to": {"point": [20000, 0, 0], "direction": [0, 0, 1]}
        }],
        // Rack deck (top of steel 4000) between the two terminals.
        "obstacles": [{"id": "DECK", "aabb": {"min": [2000, -1000, 3800], "max": [18000, 1000, 4000]}}],
        "racks": racks
    }))
}

#[test]
fn rack_level_is_preferred() {
    let without = rack_problem(json!([]));
    let r0 = &route_all(&without, &scene(&without))[0];
    assert!(r0.ok, "{:?}", r0.error);
    assert!(r0.points.iter().all(|p| p[2] < 3700.0), "{:?}", r0.points);

    let with = rack_problem(json!([{
        "id": "PR-1", "axis": "x", "levels": [4000],
        "footprint": {"min": [0, -1000, 0], "max": [20000, 1000, 0]}
    }]));
    let r = &route_all(&with, &scene(&with))[0];
    assert!(r.ok, "{:?}", r.error);
    // Centreline = top of steel + shoe 100 + outer radius.
    let z = 4000.0 + 100.0 + 114.3 / 2.0;
    let on_rack: f64 = r
        .points
        .windows(2)
        .filter(|w| w[0][2] == z && w[1][2] == z)
        .map(|w| (w[1][0] - w[0][0]).abs())
        .sum();
    assert!(on_rack >= 19000.0, "{:?}", r.points);
    assert_eq!(r.bends, 2);
}

#[test]
fn coarse_to_fine_matches_fine() {
    let obstacles =
        json!([{"id": "E1", "aabb": {"min": [2000, -1000, -1000], "max": [3000, 1000, 1000]}}]);
    let fine = problem(json!({"obstacles": obstacles}));
    let f = &route_all(&fine, &scene(&fine))[0];
    let coarse = problem(json!({
        "obstacles": obstacles,
        "grid": {"step": 100, "margin": 1500, "coarseStep": 500}
    }));
    let c = &route_all(&coarse, &scene(&coarse))[0];
    assert!(f.ok && c.ok, "{:?} {:?}", f.error, c.error);
    assert!(
        c.notes
            .iter()
            .any(|n| n.starts_with("coarse-to-fine: 500 mm graph")),
        "{:?}",
        c.notes
    );
    assert!(c.grid_nodes < f.grid_nodes);
    // The corridor contains an optimal route of the full graph.
    assert!((c.cost - f.cost).abs() < 1e-6, "{} vs {}", c.cost, f.cost);
}

#[test]
fn coarse_to_fine_falls_back_for_narrow_gaps() {
    // A wall across the whole region with a 400 mm gap: too narrow for 500 mm cells.
    let wall = |id: &str, y0: f64, y1: f64| json!({"id": id, "aabb": {"min": [2400, y0, -2000], "max": [2500, y1, 2000]}});
    let p = problem(json!({
        "bounds": {"min": [-1500, -1500, -500], "max": [6500, 4500, 500]},
        "obstacles": [wall("W1", -2000.0, 1000.0), wall("W2", 1400.0, 5000.0)],
        "grid": {"step": 50, "margin": 1500, "coarseStep": 500}
    }));
    let r = &route_all(&p, &scene(&p))[0];
    assert!(r.ok, "{:?} {:?}", r.error, r.notes);
    assert!(
        r.notes.iter().any(|n| n.contains("full 50 mm graph")),
        "{:?}",
        r.notes
    );
}

/// Plan-only problem (one z level): L1 runs straight along the lower corridor and cuts L2
/// (a short vertical line into the block's nozzle) off, unless L1 takes the upper corridor.
fn rip_up_problem(rounds: usize) -> RouteProblem {
    problem(json!({
        "bounds": {"min": [1000, 0, 0], "max": [9000, 6000, 0]},
        "settings": {"ripUpRounds": rounds},
        "obstacles": [{"id": "B", "aabb": {"min": [3000, 2000, -500], "max": [7000, 4000, 500]}}],
        "routes": [
            {"id": "L1", "pipeClass": "A1A2", "dn": 100,
             "from": {"point": [1000, 1000, 0], "direction": [1, 0, 0]},
             "to": {"point": [9000, 1000, 0], "direction": [-1, 0, 0]}},
            {"id": "L2", "pipeClass": "A1A2", "dn": 100,
             "from": {"point": [5000, 0, 0], "direction": [0, 1, 0]},
             "to": {"point": [5000, 2000, 0], "direction": [0, -1, 0], "owner": "B"}}
        ]
    }))
}

#[test]
fn rip_up_reroutes_blocking_line() {
    let seq = rip_up_problem(0);
    let r = route_all(&seq, &scene(&seq));
    assert!(r[0].ok && !r[1].ok, "{:?} {:?}", r[0].points, r[1].points);
    assert_eq!(r[0].bends, 0);

    let p = rip_up_problem(5);
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok && r[1].ok, "{:?} {:?}", r[0].error, r[1].error);
    assert_eq!(r[1].bends, 0);
    assert!(r[0].bends >= 4);
    assert!(
        r[0].points.iter().any(|q| q[1] > 4000.0),
        "{:?}",
        r[0].points
    );
    assert!(r[1]
        .notes
        .iter()
        .any(|n| n.starts_with("rip-up: routed before L1")));
    assert!(r[0].notes.iter().any(|n| n == "rip-up: re-routed after L2"));
}
