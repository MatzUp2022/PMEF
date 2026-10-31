//! PMEF ⇄ PCF bridge for the AutoCAD Plant 3D round trip.
//!
//! **Import direction (PMEF → Plant 3D):** [`write_pcf`] writes one PCF (ISOGEN Piping Component
//! File, an openly documented text format) per `pmef:PipingNetworkSystem`. Plant 3D imports PCF
//! with its built-in PCF import; no proprietary API or format is used.
//!
//! **Return direction (Plant 3D → PMEF):** [`apply_pcf`] reads the PCF exported by Plant 3D and
//! *updates* the PMEF model it came from. Components are re-identified by the PCF attribute
//! `UNIQUE-COMPONENT-IDENTIFIER`, which carries the PMEF `@id`. Only what PCF actually transports
//! is taken from the PCF (port coordinates, bores, SKEY, weight, valve tag); every other field is
//! kept from the reference model. Components without a known identifier are created; reference
//! components of a written line that are missing from its PCF are deleted.

use crate::{parse_pcf, pcf_to_pmef_type, PcfComponent, PcfUnits};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// PCF attribute carrying the PMEF `@id`.
pub const UCI: &str = "UNIQUE-COMPONENT-IDENTIFIER";

/// PMEF piping component types written to PCF, with their PCF keyword.
fn pcf_keyword(o: &Value) -> Option<&'static str> {
    Some(match o.get("@type")?.as_str()? {
        "pmef:Pipe" => "PIPE",
        "pmef:Elbow" => "ELBOW",
        "pmef:Tee" => "TEE",
        "pmef:Reducer" => {
            if o.get("reducerType").and_then(Value::as_str) == Some("ECCENTRIC") { "REDUCER-ECCENTRIC" } else { "REDUCER-CONCENTRIC" }
        }
        "pmef:Flange" => {
            if o.get("flangeType").and_then(Value::as_str) == Some("BLIND") { "FLANGE-BLIND" } else { "FLANGE" }
        }
        "pmef:Valve" => "VALVE",
        "pmef:Gasket" => "GASKET",
        "pmef:Olet" => "OLET",
        "pmef:Weld" => "WELD",
        "pmef:PipeSupport" => "SUPPORT",
        _ => return None,
    })
}

fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

fn point(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
}

/// Number formatting: shortest representation that round-trips exactly (no precision loss).
fn n(x: f64) -> String {
    let t = format!("{x}");
    if t.contains('.') || t.contains('e') { t } else { format!("{t}.0") }
}

/// Intersection of the two port axes of a bend (PCF `CENTRE-POINT`), if they meet within 0.5 mm.
fn centre_point(ports: &[Value]) -> Option<[f64; 3]> {
    let (p1, p2) = (point(ports.first()?.get("coordinate")?)?, point(ports.get(1)?.get("coordinate")?)?);
    let (d1, d2) = (point(ports[0].get("direction")?)?, point(ports[1].get("direction")?)?);
    // closest points of lines p1 + s·d1 and p2 + u·d2
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let w = [p1[0] - p2[0], p1[1] - p2[1], p1[2] - p2[2]];
    let (a, b, c, d, e) = (dot(d1, d1), dot(d1, d2), dot(d2, d2), dot(d1, w), dot(d2, w));
    let den = a * c - b * b;
    if den.abs() < 1e-12 {
        return None; // parallel axes
    }
    let (sp, up) = ((b * e - c * d) / den, (a * e - b * d) / den);
    let q1 = [p1[0] + sp * d1[0], p1[1] + sp * d1[1], p1[2] + sp * d1[2]];
    let q2 = [p2[0] + up * d2[0], p2[1] + up * d2[1], p2[2] + up * d2[2]];
    let gap = ((q1[0] - q2[0]).powi(2) + (q1[1] - q2[1]).powi(2) + (q1[2] - q2[2]).powi(2)).sqrt();
    let off_ports = dot([q1[0] - p1[0], q1[1] - p1[1], q1[2] - p1[2]], [q1[0] - p1[0], q1[1] - p1[1], q1[2] - p1[2]]) > 1e-6;
    (gap <= 0.5 && off_ports).then_some(q1)
}

/// Result of [`write_pcf`].
#[derive(Debug, Default)]
pub struct PcfExport {
    /// `(lineNumber, PCF text)`, sorted by line number.
    pub files: Vec<(String, String)>,
    /// Problems that did not stop the export (e.g. bends without a computable centre point).
    pub warnings: Vec<String>,
}

/// Write one PCF per piping line of a PMEF package.
pub fn write_pcf(objects: &[Value]) -> PcfExport {
    let by_id: BTreeMap<&str, &Value> = objects.iter().filter_map(|o| Some((s(o, "@id")?, o))).collect();
    let mut out = PcfExport::default();
    let mut lines: Vec<&Value> = objects.iter().filter(|o| s(o, "@type") == Some("pmef:PipingNetworkSystem")).collect();
    lines.sort_by_key(|l| s(l, "lineNumber").unwrap_or(""));
    for line in lines {
        let (Some(line_id), Some(line_no)) = (s(line, "@id"), s(line, "lineNumber")) else { continue };
        let segs: BTreeSet<&str> = objects.iter()
            .filter(|o| s(o, "@type") == Some("pmef:PipingSegment") && s(o, "isPartOf") == Some(line_id))
            .filter_map(|o| s(o, "@id")).collect();
        // segment component order first, then the remaining components by @id
        let mut order: Vec<&str> = Vec::new();
        for seg in &segs {
            for c in by_id[seg].get("components").and_then(Value::as_array).into_iter().flatten() {
                if let Some(c) = c.as_str() {
                    if by_id.get(c).is_some_and(|o| pcf_keyword(o).is_some()) && !order.contains(&c) {
                        order.push(c);
                    }
                }
            }
        }
        let mut rest: Vec<&str> = objects.iter()
            .filter(|o| pcf_keyword(o).is_some() && s(o, "isPartOf").is_some_and(|p| segs.contains(p)))
            .filter_map(|o| s(o, "@id")).filter(|id| !order.contains(id)).collect();
        rest.sort_unstable();
        order.extend(rest);
        if order.is_empty() {
            continue;
        }

        let mut t = String::new();
        t.push_str("ISOGEN-FILES ISOGEN.FLS\nUNITS-BORE MM\nUNITS-CO-ORDS MM\nUNITS-WEIGHT KGS\n");
        t.push_str(&format!("PIPELINE-REFERENCE {line_no}\n"));
        for id in order {
            let o = by_id[id];
            let kw = pcf_keyword(o).expect("filtered");
            t.push_str(kw);
            t.push('\n');
            let ports: Vec<Value> = o.get("ports").and_then(Value::as_array).cloned().unwrap_or_default();
            for p in &ports {
                let Some(c) = p.get("coordinate").and_then(point) else { continue };
                let bore = p.get("nominalDiameter").and_then(Value::as_f64).unwrap_or(0.0);
                let key = if kw == "SUPPORT" { "CO-ORDS" } else { "END-POINT" };
                t.push_str(&format!("    {key} {} {} {} {}\n", n(c[0]), n(c[1]), n(c[2]), n(bore)));
            }
            if kw == "WELD" && ports.len() == 1 {
                // PCF welds carry two end points at the same location
                if let Some(c) = ports[0].get("coordinate").and_then(point) {
                    let bore = ports[0].get("nominalDiameter").and_then(Value::as_f64).unwrap_or(0.0);
                    t.push_str(&format!("    END-POINT {} {} {} {}\n", n(c[0]), n(c[1]), n(c[2]), n(bore)));
                }
            }
            if kw == "ELBOW" {
                match centre_point(&ports) {
                    Some(c) => t.push_str(&format!("    CENTRE-POINT {} {} {}\n", n(c[0]), n(c[1]), n(c[2]))),
                    None => out.warnings.push(format!("{id}: no CENTRE-POINT (port axes do not intersect)")),
                }
            }
            if let Some(sk) = o.get("componentSpec").and_then(|c| s(c, "skey")) {
                t.push_str(&format!("    SKEY {}\n", sk.trim()));
            }
            if let Some(w) = o.get("componentSpec").and_then(|c| c.get("weight")).and_then(Value::as_f64) {
                t.push_str(&format!("    WEIGHT {}\n", n(w)));
            }
            if let Some(tag) = s(o, "tagNumber") {
                t.push_str(&format!("    COMPONENT-TAG {tag}\n"));
            }
            t.push_str(&format!("    {UCI} {id}\n"));
        }
        out.files.push((line_no.to_owned(), t));
    }
    out
}

/// Counts reported by [`apply_pcf`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PcfImportStats {
    /// Reference components updated from the PCF.
    pub updated: usize,
    /// Components created (no known identifier).
    pub created: usize,
    /// Reference components deleted (missing from their line's PCF).
    pub deleted: usize,
    /// PCF files whose PIPELINE-REFERENCE matched no reference line (skipped).
    pub unknown_lines: Vec<String>,
}

/// Set a number only if it really changed, so an unchanged round trip keeps the original
/// JSON representation (e.g. `150` stays `150`, not `150.0`).
fn set_num(slot: &mut Value, new: f64) {
    match slot.as_f64() {
        Some(old) if (old - new).abs() <= 1e-9 => {}
        _ => *slot = json!(new),
    }
}

fn update_from_pcf(obj: &mut Value, comp: &PcfComponent, units: PcfUnits) {
    let pts: Vec<[f64; 4]> = comp.end_points.clone();
    if let Some(ports) = obj.get_mut("ports").and_then(Value::as_array_mut) {
        for (port, ep) in ports.iter_mut().zip(&pts) {
            let c = [units.to_mm(ep[0]), units.to_mm(ep[1]), units.to_mm(ep[2])];
            match port.get_mut("coordinate").and_then(Value::as_array_mut).filter(|a| a.len() == 3) {
                Some(a) => (0..3).for_each(|i| set_num(&mut a[i], c[i])),
                None => port["coordinate"] = json!(c),
            }
            if ep[3] > 0.0 {
                set_num(&mut port["nominalDiameter"], units.to_mm(ep[3]));
            }
        }
    }
    if let Some(spec) = obj.get_mut("componentSpec").and_then(Value::as_object_mut) {
        if let Some(sk) = comp.skey() {
            spec.insert("skey".into(), json!(sk));
        }
        if let Some(w) = comp.weight() {
            set_num(spec.entry("weight").or_insert(Value::Null), w);
        }
    }
    if obj.get("@type").and_then(Value::as_str) == Some("pmef:Pipe") && pts.len() >= 2 {
        let d: f64 = (0..3).map(|i| units.to_mm(pts[1][i] - pts[0][i]).powi(2)).sum::<f64>().sqrt();
        set_num(&mut obj["pipeLength"], d);
    }
    if let Some(tag) = comp.attr("COMPONENT-TAG") {
        obj["tagNumber"] = json!(tag);
    }
}

/// Update a PMEF model from PCF files exported by Plant 3D (see module docs).
pub fn apply_pcf(reference: &[Value], pcfs: &[String]) -> (Vec<Value>, PcfImportStats) {
    let mut objs: Vec<Value> = reference.to_vec();
    let mut stats = PcfImportStats::default();
    for pcf in pcfs {
        let (units, line_no, comps) = parse_pcf(pcf);
        let Some(line) = objs.iter().find(|o| s(o, "@type") == Some("pmef:PipingNetworkSystem") && s(o, "lineNumber") == Some(line_no.as_str())).cloned() else {
            stats.unknown_lines.push(line_no);
            continue;
        };
        let line_id = s(&line, "@id").unwrap_or("").to_owned();
        let segs: Vec<String> = objs.iter()
            .filter(|o| s(o, "@type") == Some("pmef:PipingSegment") && s(o, "isPartOf") == Some(line_id.as_str()))
            .filter_map(|o| s(o, "@id").map(str::to_owned)).collect();
        let Some(first_seg) = segs.iter().min().cloned() else { continue };
        let in_line = |o: &Value| pcf_keyword(o).is_some() && s(o, "isPartOf").is_some_and(|p| segs.iter().any(|x| x == p));
        let before: BTreeSet<String> = objs.iter().filter(|o| in_line(o)).filter_map(|o| s(o, "@id").map(str::to_owned)).collect();
        let project = line_id.split(':').nth(3).unwrap_or("proj").to_owned();
        let line_clean: String = line_no.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_')).collect();

        let mut seen = BTreeSet::new();
        let mut created: Vec<Value> = Vec::new();
        for (i, comp) in comps.iter().enumerate() {
            match comp.attr(UCI).filter(|u| before.contains(*u)) {
                Some(uci) => {
                    let uci = uci.to_owned();
                    if let Some(o) = objs.iter_mut().find(|o| s(o, "@id") == Some(uci.as_str())) {
                        update_from_pcf(o, comp, units);
                    }
                    seen.insert(uci);
                    stats.updated += 1;
                }
                None => {
                    let (ty, cls, skey) = pcf_to_pmef_type(&comp.keyword, comp.skey());
                    let id = format!("urn:pmef:obj:{project}:{line_clean}-PCF-{:03}", i + 1);
                    let ports: Vec<Value> = comp.end_points.iter().enumerate().map(|(k, _)| json!({"portId": format!("P{}", k + 1)})).collect();
                    let mut o = json!({"@type": ty, "@id": id, "pmefVersion": "0.10.0", "isPartOf": first_seg,
                        "componentSpec": {"componentClass": cls, "skey": comp.skey().unwrap_or(skey)}, "ports": ports,
                        "revision": {"revisionId": "r2026-01-01-001", "changeState": "WIP", "authoringTool": "pmef-adapter-plant3d (PCF)"}});
                    update_from_pcf(&mut o, comp, units);
                    created.push(o);
                    stats.created += 1;
                }
            }
        }
        // delete reference components of this line that are missing from the PCF
        let deleted: BTreeSet<String> = before.difference(&seen).cloned().collect();
        stats.deleted += deleted.len();
        objs.retain(|o| !s(o, "@id").is_some_and(|i| deleted.contains(i)));
        let new_ids: Vec<Value> = created.iter().map(|o| o["@id"].clone()).collect();
        for o in objs.iter_mut() {
            if let Some(list) = o.get_mut("components").and_then(Value::as_array_mut) {
                list.retain(|c| !c.as_str().is_some_and(|c| deleted.contains(c)));
                if s_owned(o, "@id").as_deref() == Some(first_seg.as_str()) {
                    if let Some(list) = o.get_mut("components").and_then(Value::as_array_mut) {
                        list.extend(new_ids.iter().cloned());
                    }
                }
            }
        }
        objs.extend(created);
    }
    (objs, stats)
}

fn s_owned(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Vec<Value> {
        vec![
            json!({"@type": "pmef:PipingNetworkSystem", "@id": "urn:pmef:line:p:L1", "lineNumber": "6\"-CW-1", "designConditions": {"designPressure": 1e6}}),
            json!({"@type": "pmef:PipingSegment", "@id": "urn:pmef:seg:p:L1-S1", "isPartOf": "urn:pmef:line:p:L1",
                   "components": ["urn:pmef:obj:p:PIPE-1", "urn:pmef:obj:p:ELB-1"]}),
            json!({"@type": "pmef:Pipe", "@id": "urn:pmef:obj:p:PIPE-1", "isPartOf": "urn:pmef:seg:p:L1-S1", "pipeLength": 1000.0,
                   "componentSpec": {"componentClass": "PIPE", "skey": "PIPW"}, "catalogRef": {"catalogId": "X"},
                   "ports": [{"portId": "P1", "coordinate": [0.0, 0.0, 0.0], "nominalDiameter": 150, "direction": [-1, 0, 0]},
                             {"portId": "P2", "coordinate": [1000.0, 0.0, 0.0], "nominalDiameter": 150, "direction": [1, 0, 0]}]}),
            json!({"@type": "pmef:Elbow", "@id": "urn:pmef:obj:p:ELB-1", "isPartOf": "urn:pmef:seg:p:L1-S1", "angle": 90.0,
                   "componentSpec": {"componentClass": "ELBOW", "skey": "ELBW"},
                   "ports": [{"portId": "P1", "coordinate": [1000.0, 0.0, 0.0], "nominalDiameter": 150, "direction": [-1, 0, 0]},
                             {"portId": "P2", "coordinate": [1228.6, 0.0, 228.6], "nominalDiameter": 150, "direction": [0, 0, 1]}]}),
            json!({"@type": "pmef:Pump", "@id": "urn:pmef:obj:p:P-1", "equipmentBasic": {"tagNumber": "P-1"}}),
        ]
    }

    #[test]
    fn pcf_text_contains_identity_geometry_and_centre_point() {
        let e = write_pcf(&model());
        assert_eq!(e.files.len(), 1);
        let (line, text) = &e.files[0];
        assert_eq!(line, "6\"-CW-1");
        assert!(text.contains("PIPELINE-REFERENCE 6\"-CW-1"));
        assert!(text.contains("END-POINT 1000.0 0.0 0.0 150.0"));
        assert!(text.contains("CENTRE-POINT 1228.6 0.0 0.0"), "{text}");
        assert!(text.contains("UNIQUE-COMPONENT-IDENTIFIER urn:pmef:obj:p:ELB-1"));
        assert!(e.warnings.is_empty());
    }

    #[test]
    fn unchanged_round_trip_is_lossless() {
        let m = model();
        let pcfs: Vec<String> = write_pcf(&m).files.into_iter().map(|f| f.1).collect();
        let (back, stats) = apply_pcf(&m, &pcfs);
        assert_eq!(stats, PcfImportStats { updated: 2, ..Default::default() });
        assert_eq!(back, m);
    }

    #[test]
    fn edits_in_the_tool_are_applied() {
        let m = model();
        let pcf = write_pcf(&m).files.remove(0).1;
        // the tool moved the pipe start, deleted the elbow and added a flange without identifier
        let edited = pcf
            .replace("END-POINT 0.0 0.0 0.0 150.0", "END-POINT -50.0 0.0 0.0 150.0")
            .split("ELBOW").next().unwrap().to_owned()
            + "FLANGE\n    END-POINT 1000.0 0.0 0.0 150.0\n    END-POINT 1080.0 0.0 0.0 150.0\n    SKEY FLWN\n";
        let (back, stats) = apply_pcf(&m, &[edited]);
        assert_eq!((stats.updated, stats.created, stats.deleted), (1, 1, 1));
        let pipe = back.iter().find(|o| o["@id"] == "urn:pmef:obj:p:PIPE-1").unwrap();
        assert_eq!(pipe["pipeLength"], 1050.0);
        assert_eq!(pipe["catalogRef"]["catalogId"], "X", "fields PCF does not carry are kept");
        assert!(back.iter().all(|o| o["@id"] != "urn:pmef:obj:p:ELB-1"));
        let seg = back.iter().find(|o| o["@type"] == "pmef:PipingSegment").unwrap();
        assert_eq!(seg["components"], json!(["urn:pmef:obj:p:PIPE-1", "urn:pmef:obj:p:6-CW-1-PCF-002"]));
        assert!(back.iter().any(|o| o["@type"] == "pmef:Pump"), "non-piping objects are untouched");
    }
}
