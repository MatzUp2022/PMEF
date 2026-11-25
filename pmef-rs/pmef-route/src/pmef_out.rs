//! Turn routed centrelines into PMEF objects (spec 02 piping, RFC-0001 provenance).
//!
//! Per route: optionally a `PipingNetworkSystem`, one `PipingSegment`, and alternating `Pipe` /
//! `Elbow` components whose ports are chained with `connectedTo`. Elbows sit on the bend points
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
    let seg_id = format!("urn:pmef:seg:{proj}:{}-S1", r.id);
    let pts = &r.points;
    let c2f = rules.elbow_c2f;
    let n = pts.len();

    // Component layout: (kind, ports[(point, outward dir)]).
    struct Item {
        elbow: bool,
        p1: (P3, P3),
        p2: (P3, P3),
    }
    let mut items: Vec<Item> = Vec::new();
    let mut cursor = (pts[0], unit(pts[0], pts[1]));
    for k in 1..n {
        let (end, next_elbow) = if k == n - 1 {
            (pts[k], None)
        } else {
            let u = unit(pts[k - 1], pts[k]);
            let w = unit(pts[k], pts[k + 1]);
            (add(pts[k], u, -c2f), Some((pts[k], u, w)))
        };
        let dir = unit(pts[k - 1], pts[k]);
        let len = (0..3).map(|i| (end[i] - cursor.0[i]).abs()).sum::<f64>();
        if len > 0.5 {
            items.push(Item {
                elbow: false,
                p1: (cursor.0, neg(dir)),
                p2: (end, dir),
            });
        }
        if let Some((c, u, w)) = next_elbow {
            let out = add(c, w, c2f);
            items.push(Item {
                elbow: true,
                p1: (end, neg(u)),
                p2: (out, w),
            });
            cursor = (out, w);
        }
    }

    let mut ids = Vec::new();
    let (mut np, mut ne) = (0, 0);
    for it in &items {
        if it.elbow {
            ne += 1;
            ids.push(format!("urn:pmef:obj:{proj}:{}-ELB-{ne:03}", r.id));
        } else {
            np += 1;
            ids.push(format!("urn:pmef:obj:{proj}:{}-PIPE-{np:03}", r.id));
        }
    }
    let mut comps = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let prev = if i == 0 {
            spec.from.connected_to.clone()
        } else {
            Some(ids[i - 1].clone())
        };
        let next = if i + 1 == items.len() {
            spec.to.connected_to.clone()
        } else {
            Some(ids[i + 1].clone())
        };
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
        let ports = json!([port("P1", it.p1, &prev), port("P2", it.p2, &next)]);
        let o = if it.elbow {
            meta(json!({
                "@type": "pmef:Elbow", "@id": ids[i], "pmefVersion": PMEF_VERSION, "isPartOf": seg_id,
                "itemNumber": (i + 1).to_string(),
                "componentSpec": {"componentClass": "ELBOW", "endType1": "BW", "endType2": "BW", "centreToFace": c2f},
                "angle": 90.0, "radius": elbow_radius(&rules.elbow_radius),
                "catalogRef": {"catalogId": rules.elbow_entry},
                "ports": ports,
            }))
        } else {
            let len = it
                .p1
                .0
                .iter()
                .zip(it.p2.0)
                .map(|(a, b)| (a - b).abs())
                .sum::<f64>();
            meta(json!({
                "@type": "pmef:Pipe", "@id": ids[i], "pmefVersion": PMEF_VERSION, "isPartOf": seg_id,
                "itemNumber": (i + 1).to_string(),
                "componentSpec": {"componentClass": "PIPE", "endType1": "BW", "endType2": "BW"},
                "pipeLength": (len * 1000.0).round() / 1000.0,
                "catalogRef": {"catalogId": rules.pipe_entry},
                "ports": ports,
            }))
        };
        comps.push(o);
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
            "segments": [seg_id],
        }));
        if let Some(p) = &problem.parent {
            line["isPartOf"] = json!(p);
        }
        objs.push(line);
    }
    objs.push(meta(json!({
        "@type": "pmef:PipingSegment", "@id": seg_id, "isPartOf": line_id, "segmentNumber": 1,
        "specification": specification, "components": ids,
    })));
    objs.extend(comps);
    objs
}
