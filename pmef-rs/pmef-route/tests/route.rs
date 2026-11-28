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
    // The 90° corner is taken at the centreline radius R = 600 + 300/2 = 750 mm:
    // 2 R − R π/2 shorter than the sharp corner.
    let r = 750.0;
    let corner = 2.0 * r - r * std::f64::consts::FRAC_PI_2;
    assert!((c[0].bend_correction_mm - corner).abs() < 1e-6, "{}", c[0].bend_correction_mm);
    assert!((c[0].length_mm - (9000.0 + 9000.0 + 2.0 * 3000.0 - corner)).abs() < 1e-6);
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

    // Project class codes are mapped to catalog classes.
    let mut files2 = files.clone();
    let pid = files2["pump-skid-pid.ndjson"]
        .replace("\"pipeClass\":\"A1A2\"", "\"pipeClass\":\"75HB13\"");
    files2.insert("pump-skid-pid.ndjson".into(), pid);
    let read2 = |f: &str| files2.get(f).cloned().ok_or(format!("{f}: not given"));
    let mut q = p.clone();
    q.line_lists[0].lines = vec!["6\"-CW-202-A1A2".into()];
    let out = pmef_route::run(q.clone(), None, &read2).unwrap();
    assert!(!out.routes[0].ok, "unknown class 75HB13 must fail");
    q.line_lists[0]
        .class_map
        .insert("75HB13".into(), "A1A2".into());
    let out = pmef_route::run(q, None, &read2).unwrap();
    assert!(out.routes[0].ok, "{:?}", out.routes[0].error);
    assert_eq!(out.routes[0].pipe_class, "A1A2");

    // A missing file is an error, not a panic.
    let mut q = p;
    q.line_lists[0].file = "missing.ndjson".into();
    assert!(pmef_route::run(q, None, &read).is_err());
}

#[test]
fn cables_bound_to_trays_keep_their_trays_when_another_is_rerouted() {
    // Tray A takes one 100 mm cable (40 % of 300 x 100 mm), tray B runs parallel 1.2 m away.
    let p = |c2_diameter: f64, c1_trays: serde_json::Value| {
        cable_problem(json!({
            "trays": [
                {"id": "A", "points": [[0, 0, 3000], [20000, 0, 3000]], "width": 300, "height": 100},
                {"id": "B", "points": [[0, -1200, 3000], [20000, -1200, 3000]], "width": 300, "height": 100}
            ],
            "cables": [
                {"id": "C-1", "class": "power", "diameter": 100, "minBendRadius": 100,
                 "from": {"point": [2000, 200, 0]}, "to": {"point": [18000, 200, 0]},
                 "trays": c1_trays},
                {"id": "C-2", "class": "power", "diameter": c2_diameter, "minBendRadius": 100,
                 "from": {"point": [2000, 300, 0]}, "to": {"point": [18000, 300, 0]}}
            ]
        }))
    };
    let (c, _) = pmef_route::cable::route_cables(&p(90.0, json!([])));
    assert_eq!(
        (c[0].trays.clone(), c[1].trays.clone()),
        (vec!["A".to_string()], vec!["B".to_string()])
    );
    // C-2 grows to 110 mm: routed alone first (largest first) it would take tray A from C-1.
    let (c, _) = pmef_route::cable::route_cables(&p(110.0, json!([])));
    assert_eq!(c[1].trays, ["A"]);
    // Bound to the trays it used, C-1 keeps tray A and C-2 is re-routed around it.
    let (c, fill) = pmef_route::cable::route_cables(&p(110.0, json!(["A"])));
    assert!(c.iter().all(|c| c.ok), "{c:?}");
    assert_eq!(
        (c[0].trays.clone(), c[1].trays.clone()),
        (vec!["A".to_string()], vec!["B".to_string()])
    );
    assert_eq!(fill[0].cables, ["C-1"]);
    // A cable bound to trays it cannot reach fails.
    let (c, _) = pmef_route::cable::route_cables(&p(90.0, json!(["missing"])));
    assert!(!c[0].ok);
}

#[test]
fn a_line_list_line_given_as_route_is_not_routed_twice() {
    // A viewer edits a line-list line by copying its generated route (report lineListRoutes)
    // into routes[]; the line list then leaves that line to the explicit route.
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
    let mut p: RouteProblem = serde_json::from_str(&text).unwrap();
    p.line_lists[0].pipe_class = Some("A1A2".into());
    let out = pmef_route::run(p.clone(), None, &read).unwrap();
    let report = out.report();
    let generated: Vec<pmef_route::problem::RouteSpec> =
        serde_json::from_value(report["lineListRoutes"].clone()).unwrap();
    let ids: Vec<_> = generated.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["CW-202", "CW-206"]);

    let first: Vec<_> = out.routes.iter().map(|r| (r.ok, r.length_mm)).collect();

    // Copy CW-202 into routes[]: routed once, from routes[], with the same result.
    let mut q = p.clone();
    q.routes = vec![generated[0].clone()];
    let out = pmef_route::run(q, None, &read).unwrap();
    let ids: Vec<_> = out.routes.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["CW-202", "CW-206"]);
    // CW-206 starts in the clearance zone of the pump (as without the copy); CW-202 routes.
    assert!(out.routes[0].ok, "{:?}", out.routes[0].error);
    let again: Vec<_> = out.routes.iter().map(|r| (r.ok, r.length_mm)).collect();
    assert_eq!(again, first);
    assert_eq!(out.line_list_routes.len(), 1);
    assert_eq!(out.line_list_routes[0].id, "CW-206");
    assert!(out
        .objects
        .iter()
        .filter(
            |o| o["@type"] == "pmef:PipingSegment" && o["@id"].as_str().unwrap().contains("CW-202")
        )
        .all(|o| o["isPartOf"] == "urn:pmef:line:eaf-2026:CW-202"));
}

#[test]
fn line_list_route_ids_are_unique_across_the_problem() {
    // Generated route ids must not collide with explicit routes, with routes of another line
    // list, or with ids an earlier suffix would produce: segment, pipe and fitting @ids are
    // derived from the route id.
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
    let mut p: RouteProblem = serde_json::from_str(&text).unwrap();
    let explicit: RouteProblem =
        serde_json::from_str(&std::fs::read_to_string(dir.join("pump-skid-route.json")).unwrap())
            .unwrap();
    // Without `line`: a route naming the P&ID line would take that line out of the line list.
    let mut a = explicit.routes[0].clone();
    a.id = "CW-202".into();
    a.line = None;
    let mut b = explicit.routes[1].clone();
    b.id = "CW-202-2".into();
    b.line = None;
    p.routes = vec![a, b];
    p.line_lists.push(p.line_lists[0].clone());
    let out = pmef_route::run(p, None, &read).unwrap();
    let ids: Vec<_> = out.routes.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["CW-202", "CW-202-2", "CW-202-3", "CW-202-4"]);
    let mut seen = std::collections::HashSet::new();
    for o in &out.objects {
        let id = o["@id"].as_str().unwrap();
        assert!(seen.insert(id.to_string()), "duplicate @id {id}");
    }
}

/// Closed box as 12 outward-facing triangles.
fn box_mesh(min: [f64; 3], max: [f64; 3]) -> pmef_route::geom::Mesh {
    let c = |i: usize| {
        [
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        ]
    };
    // (corner indices of each face, counter-clockwise seen from outside)
    let quads = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    let mut tris = Vec::new();
    for q in quads {
        tris.push([c(q[0]), c(q[1]), c(q[2])]);
        tris.push([c(q[0]), c(q[2]), c(q[3])]);
    }
    pmef_route::geom::Mesh::new(tris)
}

#[test]
fn no_turns_inside_a_thick_soft_mesh() {
    // A 2 m thick block given as a mesh (not a box): only the nodes near its faces were soft, so
    // a line could enter it, turn inside and leave through the side. The whole volume is soft.
    let p = problem(json!({
        "routes": [{
            "id": "L1", "pipeClass": "A1A2", "dn": 100,
            "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
            "to": {"point": [2000, 6000, 0], "direction": [0, -1, 0]}
        }]
    }));
    let mut s = scene(&p);
    // Cheap penetration: entering through the -x face and turning inside towards the +y face
    // (8.7 k) would beat going around (9.8 k) if the inside were free.
    let (lo, hi) = ([1000.0, -5000.0, -5000.0], [3000.0, 5000.0, 5000.0]);
    s.push(
        "wall".into(),
        pmef_route::problem::ObstacleKind::Soft,
        100.0,
        pmef_route::scene::Shape::Mesh(std::sync::Arc::new(box_mesh(lo, hi))),
    );
    let r = route_all(&p, &s);
    assert!(r[0].ok, "{:?}", r[0].error);
    for q in &r[0].points[1..r[0].points.len() - 1] {
        let inside = (0..3).all(|a| q[a] > lo[a] && q[a] < hi[a]);
        assert!(!inside, "bend at {q:?} inside the wall: {:?}", r[0].points);
    }
}

/// Tray network from PMEF objects (RFC-0015): a run, a tee with three ports, a run joined to
/// the tee and a run without height (skipped).
fn tray_scene() -> String {
    let u = "urn:pmef:unit:t:U1";
    let objs = [
        json!({"@type": "pmef:CableTrayRun", "@id": "urn:pmef:obj:t:CT-A", "isPartOf": u, "trayMark": "CT-A",
               "width": 300, "height": 100, "fillLevel": 10,
               "startPoint": [0, 0, 3000], "endPoint": [10000, 0, 3000]}),
        json!({"@type": "pmef:CableTrayFitting", "@id": "urn:pmef:obj:t:CT-T", "isPartOf": u, "fittingMark": "CT-T",
               "fittingType": "TEE", "width": 300, "height": 100, "bendRadius": 300,
               "connectedTrayIds": ["urn:pmef:obj:t:CT-A", "urn:pmef:obj:t:CT-B"],
               "ports": [{"portId": "P1", "coordinate": [10000, 0, 3000], "connectedTo": "urn:pmef:obj:t:CT-A"},
                         {"portId": "P2", "coordinate": [10600, 0, 3000]},
                         {"portId": "P3", "coordinate": [10300, 300, 3000], "connectedTo": "urn:pmef:obj:t:CT-B"}]}),
        json!({"@type": "pmef:CableTrayRun", "@id": "urn:pmef:obj:t:CT-B", "isPartOf": u, "trayMark": "CT-B",
               "width": 300, "height": 100,
               "path": [[10300, 300, 3000], [10300, 8000, 3000], [10300, 8000, 1000]]}),
        json!({"@type": "pmef:CableTrayRun", "@id": "urn:pmef:obj:t:CT-C", "isPartOf": u, "trayMark": "CT-C",
               "width": 300, "startPoint": [0, 5000, 3000], "endPoint": [5000, 5000, 3000]}),
    ];
    objs.iter().map(|o| o.to_string() + "\n").collect()
}

fn tray_scene_problem(extra: serde_json::Value) -> RouteProblem {
    let mut base = json!({
        "project": "t", "revisionId": "r2026-11-27-001", "parent": "urn:pmef:unit:t:U1",
        "pmefScene": ["trays.ndjson"],
        "cableSettings": {"traysFromScene": true, "trayClasses": {"CT-A": ["power"]}},
        "cables": [
            {"id": "C-1", "class": "power", "cableType": "POWER_LV", "diameter": 32, "minBendRadius": 256,
             "from": {"point": [1000, -1000, 0]}, "to": {"point": [10300, 9000, 1000], "id": "urn:pmef:obj:t:M-1"}}
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
fn cables_route_on_trays_read_from_the_scene() {
    let scene = tray_scene();
    let read = |f: &str| if f == "trays.ndjson" { Ok(scene.clone()) } else { Err(format!("no {f}")) };
    let out = pmef_route::run(tray_scene_problem(json!({})), None, &read).unwrap();
    assert!(out.ok(), "{:?}", out.cables);
    let c = &out.cables[0];
    assert_eq!(
        c.trays,
        ["urn:pmef:obj:t:CT-A", "urn:pmef:obj:t:CT-T#P1", "urn:pmef:obj:t:CT-T#P3", "urn:pmef:obj:t:CT-B"]
    );
    // CT-C has no height; the tee becomes one tray per port, its free branch P2 unused.
    assert_eq!(out.skipped_trays.len(), 1);
    assert_eq!(out.skipped_trays[0].id, "urn:pmef:obj:t:CT-C");
    assert!(out.scene_trays.contains(&"urn:pmef:obj:t:CT-T#P2".to_string()));
    // Existing fill of CT-A (10 %) is kept; the cable adds π·16² / (300·100) = 2.7 %.
    let a = out.tray_fill.iter().find(|f| f.id == "urn:pmef:obj:t:CT-A").unwrap();
    assert!((a.fill - 12.7).abs() < 0.05, "{}", a.fill);
    // A branch id is not an @id: the CableObject names the run carrying the longest part.
    assert_eq!(out.objects[0]["cableTrayId"], "urn:pmef:obj:t:CT-B");
    let report = out.report();
    assert_eq!(report["skippedTrays"][0]["reason"], "width or height missing");
}

#[test]
fn scene_trays_keep_segregation_bend_radius_and_overrides() {
    let scene = tray_scene();
    let read = |f: &str| if f == "trays.ndjson" { Ok(scene.clone()) } else { Err(format!("no {f}")) };
    // CT-A carries power only.
    let mut p = tray_scene_problem(json!({}));
    p.cables[0].class = "signal".into();
    assert!(!pmef_route::run(p, None, &read).unwrap().ok());
    // The tee's bend radius (300 mm) is below the cable's minimum bend radius.
    let mut p = tray_scene_problem(json!({}));
    p.cables[0].min_bend_radius = 400.0;
    assert!(!pmef_route::run(p, None, &read).unwrap().ok());
    // An explicit tray with the same id replaces the scene tray (here: other classes).
    let p = tray_scene_problem(json!({"trays": [
        {"id": "urn:pmef:obj:t:CT-A", "points": [[0, 0, 3000], [10000, 0, 3000]], "width": 300,
         "height": 100, "classes": ["signal"]}]}));
    let out = pmef_route::run(p, None, &read).unwrap();
    assert!(!out.ok());
    assert!(!out.scene_trays.contains(&"urn:pmef:obj:t:CT-A".to_string()));
}

#[test]
fn scene_fittings_zero_radius_port_sizes_and_main_run() {
    // Short run CT-A → long two-port fitting CT-F (sizes only on its ports) → short run CT-B.
    let u = "urn:pmef:unit:t:U1";
    let scene = |bend: f64| -> String {
        [
            json!({"@type": "pmef:CableTrayRun", "@id": "urn:pmef:obj:t:CT-A", "isPartOf": u, "trayMark": "CT-A",
                   "width": 300, "height": 100, "startPoint": [0, 0, 3000], "endPoint": [1000, 0, 3000]}),
            json!({"@type": "pmef:CableTrayFitting", "@id": "urn:pmef:obj:t:CT-F", "isPartOf": u, "fittingMark": "CT-F",
                   "fittingType": "HORIZONTAL_BEND", "bendRadius": bend,
                   "ports": [{"portId": "P1", "coordinate": [1000, 0, 3000], "width": 300, "height": 100},
                             {"portId": "P2", "coordinate": [6000, 5000, 3000], "width": 300, "height": 100}]}),
            json!({"@type": "pmef:CableTrayRun", "@id": "urn:pmef:obj:t:CT-B", "isPartOf": u, "trayMark": "CT-B",
                   "width": 300, "height": 100, "startPoint": [6000, 5000, 3000], "endPoint": [7000, 5000, 3000]}),
        ]
        .iter()
        .map(|o| o.to_string() + "\n")
        .collect()
    };
    let problem = || -> RouteProblem {
        serde_json::from_value(json!({
            "project": "t", "revisionId": "r2026-11-27-001", "parent": u, "pmefScene": ["trays.ndjson"],
            // Short drops: the cable must enter at CT-A and leave at CT-B.
            "cableSettings": {"traysFromScene": true, "maxDrop": 600},
            "cables": [{"id": "C-1", "class": "power", "cableType": "POWER_LV", "diameter": 20, "minBendRadius": 150,
                        "from": {"point": [0, -500, 3000]}, "to": {"point": [7000, 5500, 3000]}}]
        }))
        .unwrap()
    };
    let s300 = scene(300.0);
    let read = |f: &str| if f == "trays.ndjson" { Ok(s300.clone()) } else { Err(format!("no {f}")) };
    let out = pmef_route::run(problem(), None, &read).unwrap();
    assert!(out.ok(), "{:?}", out.cables);
    // The fitting has no top-level size: its ports give it, so it is not skipped.
    assert!(out.skipped_trays.is_empty(), "{:?}", out.skipped_trays);
    assert!(out.cables[0].trays.contains(&"urn:pmef:obj:t:CT-F".to_string()));
    // The fitting carries the longest part, but cableTrayId names a run.
    let ct = out.objects[0]["cableTrayId"].as_str().unwrap();
    assert!(ct == "urn:pmef:obj:t:CT-A" || ct == "urn:pmef:obj:t:CT-B", "{ct}");
    // bendRadius 0 is a sharp junction, not "missing": the cable (150 mm) may not turn there.
    let s0 = scene(0.0);
    let read0 = |f: &str| if f == "trays.ndjson" { Ok(s0.clone()) } else { Err(format!("no {f}")) };
    let o0 = pmef_route::run(problem(), None, &read0).unwrap();
    assert!(!o0.ok(), "{:?}", o0.cables);
}

#[test]
fn scene_bends_follow_their_arc() {
    use pmef_route::cable::bend_arc;
    // 90° bend, centreline radius 500: P1 (6000,0) pointing −X, P2 (6500,500) pointing +Y.
    let arc = bend_arc([6000.0, 0.0, 3000.0], [-1.0, 0.0, 0.0], [6500.0, 500.0, 3000.0], [0.0, 1.0, 0.0]).unwrap();
    let len: f64 = arc.windows(2).map(|w| ((0..3).map(|i| (w[1][i] - w[0][i]).powi(2)).sum::<f64>()).sqrt()).sum();
    let exact = 500.0 * std::f64::consts::FRAC_PI_2;
    assert!((len - exact).abs() / exact < 1e-3, "{len} vs {exact}");
    let last = arc.last().unwrap();
    assert!((last[0] - 6500.0).abs() < 1e-6 && (last[1] - 500.0).abs() < 1e-6);
    // Every sample lies on the circle around (6000, 500).
    for p in &arc {
        let r = ((p[0] - 6000.0).powi(2) + (p[1] - 500.0).powi(2)).sqrt();
        assert!((r - 500.0).abs() < 1e-6);
    }
    // Ports slightly off the arc (rounded coordinates): the ends are the ports exactly.
    let p2 = [500.0, 500.1, 0.0];
    let arc = bend_arc([0.0, 0.0, 0.0], [-1.0, 0.0, 0.0], p2, [0.0, 1.0, 0.0]).unwrap();
    assert_eq!(*arc.last().unwrap(), p2);
    assert_eq!(arc[0], [0.0, 0.0, 0.0]);
    // Ports that do not lie on a tangent arc keep the chord.
    assert!(bend_arc([0.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [800.0, 500.0, 0.0], [0.0, 1.0, 0.0]).is_none());
    // Straight "bend": no arc.
    assert!(bend_arc([0.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [500.0, 0.0, 0.0], [1.0, 0.0, 0.0]).is_none());
}

fn drop_problem(obstacles: serde_json::Value) -> RouteProblem {
    serde_json::from_value(json!({
        "project": "t", "revisionId": "r2026-11-27-001", "parent": "urn:pmef:unit:t:U1",
        "obstacles": obstacles,
        "trays": [{"id": "urn:pmef:obj:t:CT-1", "points": [[0, 0, 3000], [10000, 0, 3000]],
                   "width": 300, "height": 100}],
        "cableSettings": {"routeDrops": true, "dropStep": 100, "dropClearance": 50},
        "cables": [{"id": "C-1", "class": "power", "cableType": "POWER_LV", "diameter": 20, "minBendRadius": 150,
                    "from": {"point": [5000, -1500, 0], "id": "urn:pmef:obj:t:M-1"},
                    "to": {"point": [9000, 0, 2000]}}]
    }))
    .unwrap()
}

#[test]
fn drops_are_routed_around_scene_obstacles() {
    let read = |f: &str| Err(format!("no {f}"));
    // Without obstacles the drop is the L1 distance: 1500 + 3000 (from), 1000 (to).
    let free = pmef_route::run(drop_problem(json!([])), None, &read).unwrap();
    let c = &free.cables[0];
    assert!(c.ok && c.warnings.is_empty(), "{:?}", c);
    assert_eq!(c.routed_drops, 2);
    assert!((c.drop_mm - 5500.0).abs() < 1e-6, "{}", c.drop_mm);
    // A wall between the motor and the tray, reaching below the floor (and the motor's own
    // box, which is skipped):
    // the drop goes around it and gets longer.
    let wall = json!([
        {"id": "urn:pmef:obj:t:WALL", "aabb": {"min": [3000, -1000, -3000], "max": [7000, -800, 3500]}},
        {"id": "urn:pmef:obj:t:M-1", "aabb": {"min": [4800, -1700, 0], "max": [5200, -1300, 400]}}
    ]);
    let out = pmef_route::run(drop_problem(wall), None, &read).unwrap();
    let c = &out.cables[0];
    assert!(c.ok && c.warnings.is_empty(), "{:?}", c);
    assert!(c.drop_mm > 5500.0 + 1000.0, "{}", c.drop_mm);
    // No point of the drop lies inside the wall grown by radius + clearance.
    let inside = |p: &[f64; 3]| p[0] > 2940.0 && p[0] < 7060.0 && p[1] > -1060.0 && p[1] < -740.0 && p[2] < 3560.0;
    assert!(c.points.iter().all(|p| !inside(p)), "{:?}", c.points);
    assert!((c.length_mm - (c.drop_mm + 4000.0)).abs() < 1e-6);
    // Both ends are still the cable ends, and the path ends with the (free) "to" drop.
    assert_eq!(c.points[0], [5000.0, -1500.0, 0.0]);
    assert_eq!(*c.points.last().unwrap(), [9000.0, 0.0, 2000.0]);
    assert_eq!(c.points[c.points.len() - 2], [9000.0, 0.0, 3000.0]);
    // An obstacle at both ends: the "from" drop gains corners, and the "to" drop is still
    // routed past its own obstacle (both drops are found after the insertion).
    let both = json!([
        {"id": "urn:pmef:obj:t:WALL", "aabb": {"min": [3000, -1000, -3000], "max": [7000, -800, 3500]}},
        {"id": "urn:pmef:obj:t:PLATE", "aabb": {"min": [8500, -500, 2400], "max": [9500, 500, 2600]}}
    ]);
    let out = pmef_route::run(drop_problem(both), None, &read).unwrap();
    let c = &out.cables[0];
    assert!(c.ok && c.warnings.is_empty() && c.routed_drops == 2, "{:?}", c);
    let in_plate = |p: &[f64; 3]| p[0] > 8440.0 && p[0] < 9560.0 && p[1] > -560.0 && p[1] < 560.0 && p[2] > 2340.0 && p[2] < 2660.0;
    // No segment of the path crosses the plate (sampled every 10 mm).
    for w in c.points.windows(2) {
        let n = ((0..3).map(|i| (w[1][i] - w[0][i]).abs()).sum::<f64>() / 10.0).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let p = [0, 1, 2].map(|i| w[0][i] + t * (w[1][i] - w[0][i]));
            assert!(!in_plate(&p), "segment {:?} crosses the plate", w);
        }
    }
    // An enclosed cable end: the drop keeps its L1 length and the cable gets a warning.
    let boxed = json!([{"id": "urn:pmef:obj:t:CAGE", "aabb": {"min": [4000, -2500, -1000], "max": [6000, -500, 1000]}}]);
    let out = pmef_route::run(drop_problem(boxed), None, &read).unwrap();
    let c = &out.cables[0];
    assert!(c.ok);
    assert_eq!(c.routed_drops, 1);
    assert!(c.warnings[0].starts_with("from drop kept as L1 distance"), "{:?}", c.warnings);
}

#[test]
fn routed_drop_lengths_decide_the_tray() {
    // T-NEAR is 2 m from each cable end by L1, but behind a wall; T-FAR is 3.5 m away and free.
    let p = |route_drops: bool| -> RouteProblem {
        serde_json::from_value(json!({
            "project": "t", "revisionId": "r2026-11-28-001", "parent": "urn:pmef:unit:t:U1",
            "obstacles": [{"id": "urn:pmef:obj:t:WALL", "aabb": {"min": [-2000, -600, -3000], "max": [12000, -400, 3000]}}],
            "trays": [
                {"id": "urn:pmef:obj:t:T-NEAR", "points": [[-1000, 0, 1000], [11000, 0, 1000]], "width": 300, "height": 100},
                {"id": "urn:pmef:obj:t:T-FAR", "points": [[-1000, -1000, 3500], [11000, -1000, 3500]], "width": 300, "height": 100}
            ],
            "cableSettings": {"routeDrops": route_drops, "dropFactor": 1},
            "cables": [{"id": "C-1", "class": "power", "cableType": "POWER_LV", "diameter": 20, "minBendRadius": 150,
                        "from": {"point": [0, -1000, 0]}, "to": {"point": [10000, -1000, 0]}}]
        }))
        .unwrap()
    };
    let read = |f: &str| Err(format!("no {f}"));
    let l1 = pmef_route::run(p(false), None, &read).unwrap();
    assert_eq!(l1.cables[0].trays, ["urn:pmef:obj:t:T-NEAR"]);
    let routed = pmef_route::run(p(true), None, &read).unwrap();
    let c = &routed.cables[0];
    assert!(c.ok && c.warnings.is_empty(), "{:?}", c);
    assert_eq!(c.trays, ["urn:pmef:obj:t:T-FAR"]);
    assert_eq!(c.routed_drops, 2);
    // Both drops are straight up: 3500 mm each.
    assert!((c.drop_mm - 7000.0).abs() < 1e-6, "{}", c.drop_mm);
    // Only the nearest candidate (T-NEAR) is routed up front; T-FAR keeps its L1 cost, wins,
    // and its drops are routed once the path uses them.
    let mut one = p(true);
    one.cable_settings.as_mut().unwrap().drop_candidates = 1;
    let c = &pmef_route::run(one, None, &read).unwrap().cables[0];
    assert!(c.ok && c.warnings.is_empty(), "{:?}", c);
    assert_eq!(c.trays, ["urn:pmef:obj:t:T-FAR"]);
    assert_eq!(c.routed_drops, 2);
}

#[test]
fn cable_end_on_a_tray_keeps_the_tray_segment() {
    // The "from" end lies on tray A; its drop candidate onto B is the end of A's own segment.
    for route_drops in [false, true] {
        let p: RouteProblem = serde_json::from_value(json!({
            "project": "t", "revisionId": "r2026-11-28-001", "parent": "urn:pmef:unit:t:U1",
            "trays": [
                {"id": "urn:pmef:obj:t:A", "points": [[0, 0, 0], [1000, 1000, 0]], "width": 300, "height": 100, "bendRadius": 0},
                {"id": "urn:pmef:obj:t:B", "points": [[1000, 1000, 0], [1000, 2000, 0]], "width": 300, "height": 100, "bendRadius": 0}
            ],
            "cableSettings": {"routeDrops": route_drops},
            "cables": [{"id": "C-1", "class": "power", "cableType": "POWER_LV", "diameter": 20, "minBendRadius": 0,
                        "from": {"point": [0, 0, 0]}, "to": {"point": [1000, 2000, 1000]}}]
        }))
        .unwrap();
        let out = pmef_route::run(p, None, &|f: &str| Err(format!("no {f}"))).unwrap();
        let c = &out.cables[0];
        assert!(c.ok, "{:?}", c);
        // The path starts with A's diagonal, not with an orthogonal drop to [1000, 1000, 0].
        assert_eq!(c.points[0], [0.0, 0.0, 0.0]);
        assert_eq!(c.points[1], [1000.0, 1000.0, 0.0], "routeDrops={route_drops}: {:?}", c.points);
        assert!(c.trays.contains(&"urn:pmef:obj:t:A".to_string()));
    }
}

#[test]
fn full_trays_get_a_width_proposal() {
    // 100 × 60 tray, maxFill 40 %: 2400 mm² for three 40 mm cables (1256.6 mm² each).
    let p = |n: usize, widths: serde_json::Value| -> RouteProblem {
        let cables: Vec<_> = (0..n)
            .map(|i| json!({"id": format!("C-{i}"), "class": "power", "cableType": "POWER_LV", "diameter": 40,
                            "minBendRadius": 0, "from": {"point": [0, -200, 0]}, "to": {"point": [5000, -200, 0]}}))
            .collect();
        serde_json::from_value(json!({
            "project": "t", "revisionId": "r2026-11-28-001", "parent": "urn:pmef:unit:t:U1",
            "trays": [{"id": "urn:pmef:obj:t:CT-1", "points": [[0, 0, 0], [5000, 0, 0]], "width": 100, "height": 60}],
            "cableSettings": {"maxFill": 40, "trayWidths": widths},
            "cables": cables
        }))
        .unwrap()
    };
    let (c, fill) = pmef_route::cable::route_cables(&p(3, json!([100, 150, 200, 300])));
    assert_eq!(c.iter().filter(|c| c.ok).count(), 1);
    let s = fill[0].sizing.as_ref().expect("sizing proposal");
    let total = 3.0 * std::f64::consts::PI * 400.0;
    assert!((s.required_width - (total / (0.4 * 60.0) * 10.0).round() / 10.0).abs() < 1e-9, "{}", s.required_width);
    assert_eq!(s.proposed_width, Some(200.0));
    assert_eq!(s.rejected_cables.len(), 2);
    assert!(s.fill_with_rejected > 60.0);
    // Fill itself still counts only the routed cable.
    assert!((fill[0].fill - 20.9).abs() < 0.05, "{}", fill[0].fill);
    // No standard width large enough: required width only.
    let (_, fill) = pmef_route::cable::route_cables(&p(3, json!([100, 150])));
    assert_eq!(fill[0].sizing.as_ref().unwrap().proposed_width, None);
    // Everything fits: no proposal.
    let (_, fill) = pmef_route::cable::route_cables(&p(1, json!([100])));
    assert!(fill[0].sizing.is_none());
}

#[test]
fn gaps_in_the_tray_network_are_joined_within_tolerance() {
    // B ends 180 mm beside A's centreline (300 mm trays: 150 + 150 + 30 mm gap); C is 2 m off.
    let p = |tol: f64| -> RouteProblem {
        serde_json::from_value(json!({
            "project": "t", "revisionId": "r2026-11-28-001", "parent": "urn:pmef:unit:t:U1",
            "trays": [
                {"id": "urn:pmef:obj:t:A", "points": [[0, 0, 3000], [10000, 0, 3000]], "width": 300, "height": 100},
                {"id": "urn:pmef:obj:t:B", "points": [[5000, 180, 3000], [5000, 8000, 3000]], "width": 300, "height": 100},
                {"id": "urn:pmef:obj:t:C", "points": [[0, 2000, 6000], [3000, 2000, 6000]], "width": 300, "height": 100}
            ],
            "cableSettings": {"trayJoinTolerance": tol, "maxDrop": 500},
            "cables": [{"id": "C-1", "class": "power", "cableType": "POWER_LV", "diameter": 20, "minBendRadius": 100,
                        "from": {"point": [500, 0, 2800]}, "to": {"point": [5000, 7800, 2800]}}]
        }))
        .unwrap()
    };
    let read = |f: &str| Err(format!("no {f}"));
    let none = pmef_route::run(p(0.0), None, &read).unwrap();
    assert!(!none.cables[0].ok);
    assert_eq!(none.tray_joins, 0);
    let out = pmef_route::run(p(50.0), None, &read).unwrap();
    // B's free end at y = 180 joins A; B's other end, A's ends and C stay free.
    assert_eq!(out.tray_joins, 1);
    let c = &out.cables[0];
    assert!(c.ok, "{:?}", c);
    assert!(c.trays.iter().any(|t| t.ends_with("#join1")));
    // The connector is never the cable's main tray.
    assert!(!out.objects[0]["cableTrayId"].as_str().unwrap().contains('#'));
    // Path: along A to x = 5000, across the 180 mm gap, along B.
    assert!(c.points.contains(&[5000.0, 0.0, 3000.0]), "{:?}", c.points);
    assert!((c.length_mm - (200.0 + 4500.0 + 180.0 + 7620.0 + 200.0)).abs() < 1.0, "{}", c.length_mm);
}

#[test]
fn scene_trays_without_height_take_the_default_height() {
    let run = json!({"@type": "pmef:CableTrayRun", "@id": "urn:pmef:obj:t:CT-1", "isPartOf": "urn:pmef:unit:t:U1",
                     "trayMark": "CT-1", "width": 300, "startPoint": [0, 0, 3000], "endPoint": [5000, 0, 3000]});
    let objs = vec![run];
    let (t, s, n) = pmef_route::cable::trays_from_pmef_with(&objs, &Default::default(), None);
    assert!(t.is_empty() && s.len() == 1 && n == 0);
    let (t, s, n) = pmef_route::cable::trays_from_pmef_with(&objs, &Default::default(), Some(100.0));
    assert_eq!((t.len(), s.len(), n), (1, 0, 1));
    assert_eq!(t[0].height, 100.0);
}

#[test]
fn tray_ends_exactly_on_another_centreline_are_joined() {
    // B starts on A's centreline (a T-junction without a shared vertex).
    let p: RouteProblem = serde_json::from_value(json!({
        "project": "t", "revisionId": "r2026-11-28-001", "parent": "urn:pmef:unit:t:U1",
        "trays": [
            {"id": "urn:pmef:obj:t:A", "points": [[0, 0, 3000], [10000, 0, 3000]], "width": 300, "height": 100},
            {"id": "urn:pmef:obj:t:B", "points": [[5000, 0, 3000], [5000, 8000, 3000]], "width": 300, "height": 100}
        ],
        "cableSettings": {"trayJoinTolerance": 50, "maxDrop": 500},
        "cables": [{"id": "C-1", "class": "power", "cableType": "POWER_LV", "diameter": 20, "minBendRadius": 100,
                    "from": {"point": [500, 0, 2800]}, "to": {"point": [5000, 7800, 2800]}}]
    }))
    .unwrap();
    let out = pmef_route::run(p, None, &|f: &str| Err(format!("no {f}"))).unwrap();
    assert!(out.cables[0].ok, "{:?}", out.cables[0]);
    assert!(out.cables[0].trays.iter().all(|t| !t.contains('#')), "no connector needed");
}

#[test]
fn lazy_drops_rerun_the_tray_search_with_their_routed_cost() {
    // T-B is nearest by L1 (2.5 m) but behind a 1.8 m wall; T-C is 4 m by L1 but cut off by a
    // wall the drop grid cannot pass. With one routed candidate per end, T-C keeps its L1
    // lower bound and wins first; once its drops turn out blocked, the search runs again and
    // takes T-B.
    let mk = |n: usize| -> RouteProblem {
        serde_json::from_value(json!({
            "project": "t", "revisionId": "r2026-11-28-001", "parent": "urn:pmef:unit:t:U1",
            "obstacles": [
                {"id": "urn:pmef:obj:t:WALL-B", "aabb": {"min": [-2000, 600, -3000], "max": [12000, 800, 1800]}},
                {"id": "urn:pmef:obj:t:WALL-C", "aabb": {"min": [-2000, -2400, -3000], "max": [12000, -2200, 4000]}}
            ],
            "trays": [
                {"id": "urn:pmef:obj:t:T-B", "points": [[-1000, 1500, 1000], [11000, 1500, 1000]], "width": 300, "height": 100},
                {"id": "urn:pmef:obj:t:T-C", "points": [[-1000, -3000, 1000], [11000, -3000, 1000]], "width": 300, "height": 100}
            ],
            "cableSettings": {"routeDrops": true, "dropFactor": 1, "dropCandidates": n},
            "cables": [{"id": "C-1", "class": "power", "cableType": "POWER_LV", "diameter": 20, "minBendRadius": 0,
                        "from": {"point": [0, 0, 0]}, "to": {"point": [10000, 0, 0]}}]
        }))
        .unwrap()
    };
    let read = |f: &str| Err(format!("no {f}"));
    let all = pmef_route::run(mk(2), None, &read).unwrap().cables.remove(0);
    let one = pmef_route::run(mk(1), None, &read).unwrap().cables.remove(0);
    assert!(all.ok && one.ok, "{:?} {:?}", all, one);
    assert!(one.warnings.is_empty(), "{:?}", one.warnings);
    assert_eq!(all.trays, ["urn:pmef:obj:t:T-B"]);
    assert_eq!(one.trays, all.trays);
    assert!((one.length_mm - all.length_mm).abs() < 1e-6, "{} vs {}", one.length_mm, all.length_mm);
}
