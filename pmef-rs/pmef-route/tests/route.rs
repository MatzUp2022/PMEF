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
    rip_up_problem_at(rounds, 0.0)
}

/// L2 at height `z2`, L1 at z = 0: for z2 = 180 the lines are 180 mm apart (more than outer
/// radius plus clearance), but L1 still blocks the 100 mm grid cells of L2's level.
fn rip_up_problem_at(rounds: usize, z2: f64) -> RouteProblem {
    problem(json!({
        "bounds": {"min": [1000, 0, 0], "max": [9000, 6000, z2]},
        "settings": {"ripUpRounds": rounds},
        "obstacles": [{"id": "B", "aabb": {"min": [3000, 2000, -500], "max": [7000, 4000, 500]}}],
        "routes": [
            {"id": "L1", "pipeClass": "A1A2", "dn": 100,
             "from": {"point": [1000, 1000, 0], "direction": [1, 0, 0]},
             "to": {"point": [9000, 1000, 0], "direction": [-1, 0, 0]}},
            {"id": "L2", "pipeClass": "A1A2", "dn": 100,
             "from": {"point": [5000, 0, z2], "direction": [0, 1, 0]},
             "to": {"point": [5000, 2000, z2], "direction": [0, -1, 0], "owner": "B"}}
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

#[test]
fn rip_up_finds_blockers_by_grid_cells() {
    let seq = rip_up_problem_at(0, 180.0);
    let r = route_all(&seq, &scene(&seq));
    assert!(r[0].ok && !r[1].ok, "{:?} {:?}", r[0].points, r[1].points);

    let p = rip_up_problem_at(5, 180.0);
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok && r[1].ok, "{:?} {:?}", r[0].error, r[1].error);
}

fn header_problem() -> RouteProblem {
    problem(json!({
        "routes": [{
            "id": "H1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [10000, 0, 0], "direction": [-1, 0, 0]},
            "branches": [
                {"id": "B1", "to": {"point": [3000, 2000, 0], "direction": [0, -1, 0]}},
                {"id": "B2", "to": {"point": [6000, -1500, 1000], "direction": [0, 0, -1]}}
            ]
        }]
    }))
}

#[test]
fn branches_end_in_tees_on_the_header() {
    let p = header_problem();
    let r = &route_all(&p, &scene(&p))[0];
    assert!(r.ok, "{:?} {:?}", r.error, r.branches);
    assert_eq!(r.bends, 0);
    assert_eq!(r.tee_entry.as_deref(), Some("A1A2-TEE-100"));
    let b1 = &r.branches[0];
    assert_eq!(b1.points, vec![[3000.0, 0.0, 0.0], [3000.0, 2000.0, 0.0]]);
    let b2 = &r.branches[1];
    assert_eq!(b2.points[0], [6000.0, 0.0, 0.0]);
    assert_eq!(b2.bends, 1);
    assert_eq!(*b2.points.last().unwrap(), [6000.0, -1500.0, 1000.0]);
}

#[test]
fn branch_output_has_connected_tees() {
    let p = header_problem();
    let r = route_all(&p, &scene(&p));
    let objs = pmef_out::to_pmef(&p, &r);
    let by_id = |id: &serde_json::Value| objs.iter().find(|o| &o["@id"] == id).unwrap();
    let tees: Vec<_> = objs.iter().filter(|o| o["@type"] == "pmef:Tee").collect();
    assert_eq!(tees.len(), 2);
    let segs: Vec<_> = objs
        .iter()
        .filter(|o| o["@type"] == "pmef:PipingSegment")
        .collect();
    assert_eq!(segs.len(), 3);
    assert_eq!(objs[0]["segments"].as_array().unwrap().len(), 3);
    // A1A2-TEE-100: centre-to-end 114 mm (catalog).
    let t = tees[0];
    assert_eq!(t["catalogRef"]["catalogId"], "A1A2-TEE-100");
    assert_eq!(t["ports"][0]["coordinate"], json!([2886.0, 0.0, 0.0]));
    assert_eq!(t["ports"][1]["coordinate"], json!([3114.0, 0.0, 0.0]));
    assert_eq!(t["ports"][2]["coordinate"], json!([3000.0, 114.0, 0.0]));
    assert_eq!(t["ports"][2]["direction"], json!([0.0, 1.0, 0.0]));
    // Every connectedTo points back (both sides of each joint agree on the coordinate).
    for o in objs.iter().filter(|o| o.get("ports").is_some()) {
        for port in o["ports"].as_array().unwrap() {
            if let Some(c) = port.get("connectedTo") {
                let other = by_id(c);
                let back = other["ports"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|q| q["connectedTo"] == o["@id"])
                    .unwrap_or_else(|| panic!("{} -> {c} has no way back", o["@id"]));
                assert_eq!(back["coordinate"], port["coordinate"]);
            }
        }
    }
}

#[test]
fn tees_keep_their_distance_and_failures_fail_the_line() {
    let p = problem(json!({
        "routes": [{
            "id": "H1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [10000, 0, 0], "direction": [-1, 0, 0]},
            "branches": [
                {"id": "B1", "to": {"point": [3000, 2000, 0], "direction": [0, -1, 0]}},
                {"id": "B2", "to": {"point": [3100, -2000, 0], "direction": [0, 1, 0]}}
            ]
        }]
    }));
    let r = &route_all(&p, &scene(&p))[0];
    assert!(r.ok, "{:?}", r.error);
    let (t1, t2) = (r.branches[0].points[0], r.branches[1].points[0]);
    assert_eq!(t1, [3000.0, 0.0, 0.0]);
    // Two A1A2-TEE-100 (centre-to-end 114 mm) need 228 mm between their centres.
    assert!((t2[0] - t1[0]).abs() >= 228.0, "{t2:?}");
    assert_eq!(t2[1], 0.0);

    let p = problem(json!({
        "obstacles": [{"id": "BOX", "aabb": {"min": [2500, 1500, -500], "max": [3500, 2500, 500]}}],
        "routes": [{
            "id": "H1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [10000, 0, 0], "direction": [-1, 0, 0]},
            "branches": [{"id": "B1", "to": {"point": [3000, 2000, 0], "direction": [0, -1, 0]}}]
        }]
    }));
    let r = &route_all(&p, &scene(&p))[0];
    assert!(!r.ok);
    assert!(
        r.error.as_deref().unwrap().starts_with("branch B1: "),
        "{:?}",
        r.error
    );
    assert!(!r.branches[0].ok);
}

#[test]
fn branch_routed_out_of_order_keeps_its_tee() {
    // B2 is nearer to the header and is routed first; B1 tees into B2.
    let p = problem(json!({
        "routes": [{
            "id": "H1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [10000, 0, 0], "direction": [-1, 0, 0]},
            "branches": [
                {"id": "B1", "to": {"point": [3000, 1500, 1500], "direction": [0, 0, -1]}},
                {"id": "B2", "to": {"point": [3000, 2000, 0], "direction": [0, -1, 0]}}
            ]
        }]
    }));
    let r = route_all(&p, &scene(&p));
    assert!(r[0].ok, "{:?} {:?}", r[0].error, r[0].branches);
    let objs = pmef_out::to_pmef(&p, &r);
    let tees = objs.iter().filter(|o| o["@type"] == "pmef:Tee").count();
    assert_eq!(
        tees,
        2,
        "on: {:?}",
        r[0].branches.iter().map(|b| b.on).collect::<Vec<_>>()
    );
    // Every branch segment starts at a tee: its first component connects to an existing object.
    let ids: Vec<&serde_json::Value> = objs.iter().map(|o| &o["@id"]).collect();
    for o in &objs {
        if let Some(ports) = o["ports"].as_array() {
            for pt in ports {
                if let Some(c) = pt.get("connectedTo") {
                    assert!(ids.contains(&c), "dangling {c} on {}", o["@id"]);
                }
            }
        }
    }
}

#[test]
fn supports_sit_on_steel_and_keep_the_span() {
    // Straight DN 100 line at z = 0 over 20 m; two beams cross it (top 200 mm below the pipe
    // bottom, outside the clearance), at x = 4000 and x = 13000.
    let beam = |id: &str, x: f64| json!({"id": id, "structural": true, "aabb": {"min": [x - 100.0, -1000, -300], "max": [x + 100.0, 1000, -257.15]}});
    let p = problem(json!({
        "obstacles": [beam("urn:pmef:obj:t:BM-1", 4000.0), beam("urn:pmef:obj:t:BM-2", 13000.0)],
        "supports": {},
        "routes": [{
            "id": "L1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [20000, 0, 0], "direction": [-1, 0, 0]}
        }]
    }));
    let r = &route_all(&p, &scene(&p))[0];
    assert!(r.ok, "{:?}", r.error);
    assert_eq!(r.bends, 0);
    let xs: Vec<f64> = r.supports.iter().map(|s| s.point[0]).collect();
    assert!(xs.contains(&4000.0) && xs.contains(&13000.0), "{xs:?}");
    // B31.1 span for NPS 4: 4300 mm; ends within half a span.
    assert!(
        xs.windows(2).all(|w| w[1] - w[0] <= 4300.0 + 1e-6),
        "{xs:?}"
    );
    assert!(
        xs[0] <= 2150.0 && 20000.0 - xs.last().unwrap() <= 2150.0,
        "{xs:?}"
    );
    let on_steel: Vec<_> = r
        .supports
        .iter()
        .filter_map(|s| s.steel.as_deref())
        .collect();
    assert_eq!(on_steel, ["urn:pmef:obj:t:BM-1", "urn:pmef:obj:t:BM-2"]);
    assert_eq!(r.supports[0].bearing[2], -114.3 / 2.0);

    let objs = pmef_out::to_pmef(&p, std::slice::from_ref(r));
    let sups: Vec<_> = objs
        .iter()
        .filter(|o| o["@type"] == "pmef:PipeSupport")
        .collect();
    assert_eq!(sups.len(), xs.len());
    assert!(sups
        .iter()
        .any(|s| s["structuralAttachmentId"] == "urn:pmef:obj:t:BM-1"));
}

#[test]
fn terminal_straight_length_is_kept() {
    // Pump suction: 1000 mm straight at `to` before the last elbow.
    let p = problem(json!({
        "routes": [{
            "id": "L1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [5000, 500, 0], "direction": [0, -1, 0], "straight": 1000}
        }]
    }));
    let r = &route_all(&p, &scene(&p))[0];
    assert!(r.ok, "{:?}", r.error);
    let n = r.points.len();
    let last = (0..3)
        .map(|i| (r.points[n - 1][i] - r.points[n - 2][i]).abs())
        .sum::<f64>();
    assert!(last >= 152.0 + 1000.0, "{:?}", r.points);
}

#[test]
fn supports_rest_only_on_structural_obstacles() {
    // A hard box under the line that is not marked structural (e.g. equipment) is no support.
    let p = problem(json!({
        "obstacles": [{"id": "E-1", "aabb": {"min": [3900, -1000, -300], "max": [4100, 1000, -257.15]}}],
        "supports": {},
        "routes": [{
            "id": "L1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [10000, 0, 0], "direction": [-1, 0, 0]}
        }]
    }));
    let r = &route_all(&p, &scene(&p))[0];
    assert!(r.ok, "{:?}", r.error);
    assert!(!r.supports.is_empty());
    assert!(r.supports.iter().all(|s| s.steel.is_none()));
}

#[test]
fn supports_keep_the_span_around_tees() {
    // 17 200 mm header, span 4 300 mm: the fill puts a support at 6 450 mm, where the branch
    // tee sits; moving it off the tee must not open a gap above the span.
    let p = problem(json!({
        "supports": {"maxSpan": 4300},
        "routes": [{
            "id": "H1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [17200, 0, 0], "direction": [-1, 0, 0]},
            "branches": [{"id": "B1", "to": {"point": [6450, 2000, 0], "direction": [0, -1, 0]}}]
        }]
    }));
    let r = &route_all(&p, &scene(&p))[0];
    assert!(r.ok, "{:?} {:?}", r.error, r.branches);
    let tee = r.branches[0].points[0];
    assert_eq!(tee, [6450.0, 0.0, 0.0]);
    let run_c2e = r.tee.as_ref().unwrap().run_c2e;
    let xs: Vec<f64> = r
        .supports
        .iter()
        .filter(|s| s.on == 0)
        .map(|s| s.point[0])
        .collect();
    assert!(
        xs.iter().all(|x| (x - tee[0]).abs() >= run_c2e - 1e-6),
        "{xs:?}"
    );
    assert!(
        xs.windows(2).all(|w| w[1] - w[0] <= 4300.0 + 1e-6),
        "{xs:?}"
    );
    assert!(
        xs[0] <= 2150.0 + 1e-6 && 17200.0 - xs.last().unwrap() <= 2150.0 + 1e-6,
        "{xs:?}"
    );
}

fn cable_problem(extra: serde_json::Value) -> RouteProblem {
    // Two parallel trays 1 m apart (power at y = 0, signal at y = 1000), joined at both ends by
    // a cross tray carrying both classes; bend radius 300 mm.
    let mut base = json!({
        "project": "t", "revisionId": "r2026-11-25-001", "parent": "urn:pmef:unit:t:U1",
        "trays": [
            {"id": "urn:pmef:obj:t:CT-P", "points": [[0, 0, 3000], [20000, 0, 3000]],
             "width": 300, "height": 100, "classes": ["power"]},
            {"id": "urn:pmef:obj:t:CT-S", "points": [[0, 1000, 3000], [20000, 1000, 3000]],
             "width": 300, "height": 100, "classes": ["signal"]},
            {"id": "CT-X1", "points": [[0, 0, 3000], [0, 1000, 3000]], "width": 300, "height": 100},
            {"id": "CT-X2", "points": [[20000, 0, 3000], [20000, 1000, 3000]], "width": 300, "height": 100}
        ],
        "cables": [
            {"id": "C-1", "class": "signal", "cableType": "INSTRUMENTATION_PAIR", "diameter": 12,
             "minBendRadius": 120,
             "from": {"point": [2000, 500, 0], "id": "urn:pmef:obj:t:FT-1"},
             "to": {"point": [18000, 500, 0], "id": "urn:pmef:obj:t:PLC-1", "terminal": "CH0"}}
        ]
    });
    if let serde_json::Value::Object(m) = extra {
        for (k, v) in m {
            base[k] = v;
        }
    }
    serde_json::from_value(base).unwrap()
}

#[test]
fn cable_uses_its_segregated_tray() {
    let p = cable_problem(json!({}));
    let (c, fill) = pmef_route::cable::route_cables(&p);
    assert!(c[0].ok, "{:?}", c[0].error);
    // The signal cable drops onto the signal tray (equally far as the power tray).
    assert_eq!(c[0].trays, ["urn:pmef:obj:t:CT-S"]);
    assert_eq!(c[0].points[1], [2000.0, 1000.0, 3000.0]);
    assert_eq!(c[0].length_mm, 16000.0 + 2.0 * 3500.0);
    let f = fill.iter().find(|f| f.id == "urn:pmef:obj:t:CT-S").unwrap();
    assert_eq!(f.cables, ["C-1"]);
    assert!(f.fill > 0.0);
    let objs = pmef_out::cables_to_pmef(&p, &c);
    assert_eq!(objs[0]["@type"], "pmef:CableObject");
    assert_eq!(objs[0]["cableTrayId"], "urn:pmef:obj:t:CT-S");
    assert_eq!(objs[0]["routeLength"], 23.0);
    assert_eq!(objs[0]["toTerminal"], "CH0");
}

#[test]
fn cable_fill_and_bend_radius() {
    // The signal tray is full: the cable may not use it, and the power tray is segregated.
    let full = cable_problem(json!({"trays": [
        {"id": "CT-S", "points": [[0, 1000, 3000], [20000, 1000, 3000]], "width": 300,
         "height": 100, "classes": ["signal"], "fill": 39.9}
    ]}));
    let (c, _) = pmef_route::cable::route_cables(&full);
    assert!(!c[0].ok);

    // A stiff cable (bend radius 400 mm) cannot turn through the 300 mm tray bends; a long
    // L-shaped tray then has no admissible path.
    let l_tray = |r: f64| {
        cable_problem(json!({
            "trays": [{"id": "CT-L", "points": [[0, 0, 3000], [10000, 0, 3000], [10000, 10000, 3000]],
                       "width": 300, "height": 100, "bendRadius": r}],
            "cables": [{"id": "C-2", "class": "power", "diameter": 40, "minBendRadius": 400,
                        "from": {"point": [1000, 0, 0]}, "to": {"point": [10000, 9000, 0]}}]
        }))
    };
    let (c, _) = pmef_route::cable::route_cables(&l_tray(300.0));
    assert!(!c[0].ok);
    let (c, _) = pmef_route::cable::route_cables(&l_tray(600.0));
    assert!(c[0].ok, "{:?}", c[0].error);
    assert_eq!(c[0].length_mm, 9000.0 + 9000.0 + 2.0 * 3000.0);
}

#[test]
fn line_list_routes_pid_lines_in_the_3d_scene() {
    // Stage 6: P&ID lines (dexpi-pmef shape, no 3D placement) are routed between the nozzles
    // of the 3D scene. The files are read through an in-memory reader, as in the WASM binding.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/route");
    let mut files = std::collections::HashMap::new();
    for f in [
        "pump-skid-pid.ndjson",
        "../../examples/pump-skid-complete.ndjson",
    ] {
        files.insert(f.to_string(), std::fs::read_to_string(dir.join(f)).unwrap());
    }
    let read = |f: &str| files.get(f).cloned().ok_or(format!("{f}: not given"));
    let text = std::fs::read_to_string(dir.join("pump-skid-linelist-route.json")).unwrap();
    let p: RouteProblem = serde_json::from_str(&text).unwrap();
    let out = pmef_route::run(p.clone(), None, &read).unwrap();

    // CW-202: pump matched by isDerivedFrom, vessel by tag number; same route as the explicit
    // problem (pump-skid-route.json, CW-202R), attached to the P&ID line.
    assert_eq!(out.routes.len(), 1);
    let r = &out.routes[0];
    assert!(r.ok, "{:?}", r.error);
    assert_eq!((r.id.as_str(), r.dn, r.bends), ("CW-202", 150, 4));
    assert!(out
        .objects
        .iter()
        .filter(|o| o["@type"] == "pmef:PipingSegment")
        .all(|o| o["isPartOf"] == "urn:pmef:line:eaf-2026:CW-202"));
    assert!(!out
        .objects
        .iter()
        .any(|o| o["@type"] == "pmef:PipingNetworkSystem"));
    // CW-205: standby pump not in the 3D scene; CW-206: no class.
    let why: Vec<_> = out
        .skipped_lines
        .iter()
        .map(|s| (s.line_number.as_str(), s.reason.as_str()))
        .collect();
    assert_eq!(why.len(), 2);
    assert!(why[0].1.contains("PID-101.P-201B#N2"), "{why:?}");
    assert!(why[1].1.starts_with("no pipeClass"), "{why:?}");

    // With a default class CW-206 becomes a route as well; a line filter keeps only it.
    let mut q = p.clone();
    q.line_lists[0].pipe_class = Some("A1A2".into());
    q.line_lists[0].lines = vec!["CW-206".into()];
    let out = pmef_route::run(q, None, &read).unwrap();
    assert_eq!(out.routes.len(), 1);
    assert_eq!(out.routes[0].id, "CW-206");
    assert_eq!(out.routes[0].dn, 100);
    assert!(out.skipped_lines.is_empty());

    // A missing file is an error, not a panic.
    let mut q = p;
    q.line_lists[0].file = "missing.ndjson".into();
    assert!(pmef_route::run(q, None, &read).is_err());
}
