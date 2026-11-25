//! Turn routed centrelines into PMEF objects (spec 02 piping, RFC-0001 provenance).
//!
//! Per route: optionally a `PipingNetworkSystem`, one `PipingSegment` for the run and one per
//! branch, and `Pipe` / `Elbow` / `Tee` components whose ports are chained with `connectedTo`
//! (a tee's branch port `P3` connects to the first component of its branch segment), plus
//! `PipeSupport`s (one port at the bearing point, pointing down) when supports are placed. Elbows sit on the bend points
//! with their faces at the catalog centre-to-face; pipes fill the gaps (omitted when two elbows
//! are welded face to face). Every object carries `provenance.sourceType = "generated"` and
//! `review.status = "detected"` in a `WIP` revision: auto-routed lines are proposals until a
//! person reviews them.

use crate::geom::P3;
use crate::problem::RouteProblem;
use crate::route::RoutedLine;
use serde_json::{json, Value};

const PMEF_VERSION: &str = "0.9.0";

fn r3(p: P3) -> Value {
    let f = |x: f64| (x * 1000.0).round() / 1000.0;
    json!([f(p[0]), f(p[1]), f(p[2])])
}

fn unit(a: P3, b: P3) -> P3 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    [d[0] / l, d[1] / l, d[2] / l]
}

fn add(p: P3, d: P3, s: f64) -> P3 {
    [p[0] + d[0] * s, p[1] + d[1] * s, p[2] + d[2] * s]
}

fn neg(d: P3) -> P3 {
    [-d[0], -d[1], -d[2]]
}

fn elbow_radius(cat: &str) -> &'static str {
    match cat {
        "LONG_RADIUS" | "1.5D" => "LONG_RADIUS",
        "SHORT_RADIUS" | "1D" => "SHORT_RADIUS",
        "3D" => "3D",
        "5D" => "5D",
        _ => "CUSTOM",
    }
}

/// PMEF objects for all successfully routed lines (failed routes are skipped).
pub fn to_pmef(problem: &RouteProblem, routed: &[RoutedLine]) -> Vec<Value> {
    let mut out = Vec::new();
    for r in routed.iter().filter(|r| r.ok) {
        out.extend(line_objects(problem, r));
    }
    out
}

/// One fitting or pipe of a polyline layout.
enum Kind {
    Pipe,
    Elbow,
    /// Tee with the branch port (point, outward direction) of branch `usize`.
    Tee(usize, (P3, P3)),
}

struct Item {
    kind: Kind,
    p1: (P3, P3),
    p2: (P3, P3),
}

/// Tee on a polyline: centre, branch direction, branch index.
type TeeAt = (P3, P3, usize);

/// Lay out pipes, elbows and tees along a polyline. The first component starts `start_off`
/// after the first point (a branch starts at its tee centre).
fn layout(
    pts: &[P3],
    c2f: f64,
    start_off: f64,
    tees: &[TeeAt],
    run_c2e: f64,
    branch_c2e: f64,
) -> Vec<Item> {
    let n = pts.len();
    let mut items: Vec<Item> = Vec::new();
    let u0 = unit(pts[0], pts[1]);
    let mut cursor = (add(pts[0], u0, start_off), u0);
    let dist = |a: P3, b: P3| (0..3).map(|i| (a[i] - b[i]).abs()).sum::<f64>();
    let pipe_to = |items: &mut Vec<Item>, cursor: (P3, P3), end: P3, dir: P3| {
        if dist(cursor.0, end) > 0.5 {
            items.push(Item {
                kind: Kind::Pipe,
                p1: (cursor.0, neg(dir)),
                p2: (end, dir),
            });
        }
    };
    for k in 1..n {
        let (a, b) = (pts[k - 1], pts[k]);
        let dir = unit(a, b);
        // Tees on this run, from its start.
        let mut here: Vec<&TeeAt> = tees
            .iter()
            .filter(|(t, ..)| {
                let along = dist(a, *t) + dist(*t, b);
                (along - dist(a, b)).abs() < 1e-6 && dist(a, *t) > 1e-6 && dist(*t, b) > 1e-6
            })
            .collect();
        here.sort_by(|x, y| dist(a, x.0).total_cmp(&dist(a, y.0)));
        for (t, bd, bi) in here {
            let start = add(*t, dir, -run_c2e);
            pipe_to(&mut items, cursor, start, dir);
            let end = add(*t, dir, run_c2e);
            items.push(Item {
                kind: Kind::Tee(*bi, (add(*t, *bd, branch_c2e), *bd)),
                p1: (start, neg(dir)),
                p2: (end, dir),
            });
            cursor = (end, dir);
        }
        if k == n - 1 {
            pipe_to(&mut items, cursor, b, dir);
        } else {
            let w = unit(b, pts[k + 1]);
            let end = add(b, dir, -c2f);
            pipe_to(&mut items, cursor, end, dir);
            let out = add(b, w, c2f);
            items.push(Item {
                kind: Kind::Elbow,
                p1: (end, neg(dir)),
                p2: (out, w),
            });
            cursor = (out, w);
        }
    }
    items
}

fn line_objects(problem: &RouteProblem, r: &RoutedLine) -> Vec<Value> {
    let (Some(rules), Some(spec)) = (&r.rules, &r.spec) else {
        return Vec::new();
    };
    let proj = &problem.project;
    // Object fields first, then revision / provenance / review (readable NDJSON).
    let meta = |mut v: Value| -> Value {
        v["revision"] = json!({
            "revisionId": problem.revision_id,
            "changeState": "WIP",
            "authoringTool": crate::GENERATOR,
        });
        v["provenance"] = json!({
            "sourceType": "generated",
            "sourceRef": format!("urn:pmef-route:{proj}:{}", r.id),
        });
        v["review"] = json!({ "status": "detected" });
        v
    };
    let line_id = spec
        .line
        .clone()
        .unwrap_or_else(|| format!("urn:pmef:line:{proj}:{}", r.id));
    let c2f = rules.elbow_c2f;
    let (run_c2e, branch_c2e) = r
        .tee
        .as_ref()
        .map_or((0.0, 0.0), |t| (t.run_c2e, t.branch_c2e));

    // Polylines: 0 = run from–to, k = branch k-1 (starting at its tee centre).
    let branches: Vec<_> = r.branches.iter().filter(|b| b.ok).collect();
    // `RoutedBranch::on` of each polyline: 0 = run, i + 1 = branch i of the route.
    let poly_on: Vec<usize> = std::iter::once(0)
        .chain(
            r.branches
                .iter()
                .enumerate()
                .filter(|(_, b)| b.ok)
                .map(|(i, _)| i + 1),
        )
        .collect();
    let mut polys: Vec<(&Vec<P3>, f64, Option<String>)> =
        vec![(&r.points, 0.0, spec.to.connected_to.clone())];
    for b in &branches {
        let end = b.spec.as_ref().and_then(|s| s.to.connected_to.clone());
        polys.push((&b.points, branch_c2e, end));
    }
    let layouts: Vec<Vec<Item>> = polys
        .iter()
        .enumerate()
        .map(|(k, (pts, off, _))| {
            let tees: Vec<TeeAt> = branches
                .iter()
                .enumerate()
                .filter(|(_, b)| b.on == poly_on[k])
                .map(|(bi, b)| (b.points[0], unit(b.points[0], b.points[1]), bi))
                .collect();
            layout(pts, c2f, *off, &tees, run_c2e, branch_c2e)
        })
        .collect();

    // @ids: numbered per kind over the whole line.
    let (mut np, mut ne, mut nt) = (0, 0, 0);
    let ids: Vec<Vec<String>> = layouts
        .iter()
        .map(|items| {
            items
                .iter()
                .map(|it| match it.kind {
                    Kind::Pipe => {
                        np += 1;
                        format!("urn:pmef:obj:{proj}:{}-PIPE-{np:03}", r.id)
                    }
                    Kind::Elbow => {
                        ne += 1;
                        format!("urn:pmef:obj:{proj}:{}-ELB-{ne:03}", r.id)
                    }
                    Kind::Tee(..) => {
                        nt += 1;
                        format!("urn:pmef:obj:{proj}:{}-TEE-{nt:03}", r.id)
                    }
                })
                .collect()
        })
        .collect();
    let seg_ids: Vec<String> = (0..layouts.len())
        .map(|k| format!("urn:pmef:seg:{proj}:{}-S{}", r.id, k + 1))
        .collect();
    // Tee of each branch (its first component connects back to it).
    let mut tee_of: Vec<Option<String>> = vec![None; branches.len()];
    for (k, items) in layouts.iter().enumerate() {
        for (i, it) in items.iter().enumerate() {
            if let Kind::Tee(bi, _) = it.kind {
                tee_of[bi] = Some(ids[k][i].clone());
            }
        }
    }

    let port = |pid: &str, (p, d): (P3, P3), conn: &Option<String>| {
        let mut v = json!({
            "portId": pid, "portType": "BIDIRECTIONAL", "coordinate": r3(p), "direction": r3(d),
            "nominalDiameter": spec.dn, "endType": "BW",
        });
        if let Some(c) = conn {
            v["connectedTo"] = json!(c);
        }
        v
    };
    let mut comps = Vec::new();
    for (k, items) in layouts.iter().enumerate() {
        let start = if k == 0 {
            spec.from.connected_to.clone()
        } else {
            tee_of[k - 1].clone()
        };
        for (i, it) in items.iter().enumerate() {
            let prev = if i == 0 {
                start.clone()
            } else {
                Some(ids[k][i - 1].clone())
            };
            let next = if i + 1 == items.len() {
                polys[k].2.clone()
            } else {
                Some(ids[k][i + 1].clone())
            };
            let base = |t: &str| {
                json!({
                    "@type": t, "@id": ids[k][i], "pmefVersion": PMEF_VERSION, "isPartOf": seg_ids[k],
                    "itemNumber": (i + 1).to_string(),
                })
            };
            let mut o = match &it.kind {
                Kind::Elbow => {
                    let mut o = base("pmef:Elbow");
                    o["componentSpec"] = json!({"componentClass": "ELBOW", "endType1": "BW", "endType2": "BW", "centreToFace": c2f});
                    o["angle"] = json!(90.0);
                    o["radius"] = json!(elbow_radius(&rules.elbow_radius));
                    o["catalogRef"] = json!({"catalogId": rules.elbow_entry});
                    o
                }
                Kind::Pipe => {
                    let len = it
                        .p1
                        .0
                        .iter()
                        .zip(it.p2.0)
                        .map(|(a, b)| (a - b).abs())
                        .sum::<f64>();
                    let mut o = base("pmef:Pipe");
                    o["componentSpec"] =
                        json!({"componentClass": "PIPE", "endType1": "BW", "endType2": "BW"});
                    o["pipeLength"] = json!((len * 1000.0).round() / 1000.0);
                    o["catalogRef"] = json!({"catalogId": rules.pipe_entry});
                    o
                }
                Kind::Tee(..) => {
                    let mut o = base("pmef:Tee");
                    o["componentSpec"] =
                        json!({"componentClass": "TEE", "endType1": "BW", "endType2": "BW"});
                    o["teeType"] = json!("EQUAL");
                    o["branchDiameter"] = json!(spec.dn);
                    o["branchAngle"] = json!(90.0);
                    o["catalogRef"] = json!({"catalogId": r.tee_entry});
                    o
                }
            };
            let mut ports = vec![port("P1", it.p1, &prev), port("P2", it.p2, &next)];
            if let Kind::Tee(bi, bp) = it.kind {
                let first = ids[bi + 1].first().cloned();
                ports.push(port("P3", bp, &first));
            }
            o["ports"] = json!(ports);
            comps.push(meta(o));
        }
    }

    for (i, sp) in r.supports.iter().enumerate() {
        let mut o = json!({
            "@type": "pmef:PipeSupport",
            "@id": format!("urn:pmef:obj:{proj}:{}-SUP-{:03}", r.id, i + 1),
            "pmefVersion": PMEF_VERSION, "isPartOf": seg_ids[sp.on],
            "itemNumber": format!("S{:02}", i + 1),
            "componentSpec": {"componentClass": "PIPE_SUPPORT"},
            "supportsMark": format!("{}-S{:02}", r.id, i + 1),
            "supportSpec": {"supportType": "RESTING"},
            "ports": [{
                "portId": "P1", "portType": "BIDIRECTIONAL", "coordinate": r3(sp.bearing),
                "direction": [0.0, 0.0, -1.0], "nominalDiameter": spec.dn, "endType": "BW",
            }],
        });
        if let Some(st) = &sp.steel {
            o["supportSpec"]["attachmentType"] = json!("WELDED");
            if st.starts_with("urn:pmef:") {
                o["structuralAttachmentId"] = json!(st);
                o["supportSpec"]["structuralRef"] = json!(st);
            }
        }
        comps.push(meta(o));
    }

    let specification = json!({
        "nominalDiameter": spec.dn, "outsideDiameter": rules.od, "pipeClass": rules.pipe_class,
    });
    let mut objs = Vec::new();
    if spec.line.is_none() {
        let mut line = meta(json!({
            "@type": "pmef:PipingNetworkSystem", "@id": line_id, "pmefVersion": PMEF_VERSION,
            "lineNumber": spec.line_number.clone().unwrap_or_else(|| r.id.clone()),
            "nominalDiameter": spec.dn, "pipeClass": rules.pipe_class,
            "specification": specification.clone(),
            "segments": seg_ids,
        }));
        if let Some(p) = &problem.parent {
            line["isPartOf"] = json!(p);
        }
        objs.push(line);
    }
    for (k, sid) in seg_ids.iter().enumerate() {
        objs.push(meta(json!({
            "@type": "pmef:PipingSegment", "@id": sid, "isPartOf": line_id, "segmentNumber": k + 1,
            "specification": specification, "components": ids[k],
        })));
    }
    objs.extend(comps);
    objs
}
