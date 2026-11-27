//! Branch topology: ports from E3D arrive/leave points, `connectedTo` between neighbours,
//! implicit tubes (TUBI) from the gaps (spec 05 §AVEVA E3D, finding "E3D ports and pipes").
//!
//! The PML export writes per component the arrive and leave p-points
//! (`APOS`/`LPOS`, `ADIR`/`LDIR`, `ABOR`/`LBOR`, `ACON`/`LCON`) and for tees the branch
//! p-point (`P3POS`, `P3DIR`, `P3BOR`, `P3CON`). Per branch it writes head and tail
//! (`HPOS`/`TPOS`, `HBOR`/`TBOR`, `HREF`/`TREF`). Components without arrive/leave points fall
//! back to one port `P0` at `POS` with empty `connectedTo` (attribute-only data).

use crate::pml::PmlElement;
use crate::E3DCoordinateSystem;
use serde_json::{json, Value};

/// Gap below which leave(i) and arrive(i+1) count as the same point [mm].
pub const JOIN_TOLERANCE_MM: f64 = 1.0;

type P3 = [f64; 3];

fn point(e: &PmlElement, key: &str, cs: E3DCoordinateSystem) -> Option<P3> {
    let (x, y, z) = e.attr(key)?.as_vec3()?;
    let (x, y, z) = cs.to_pmef(x, y, z);
    Some([x, y, z])
}

fn dist(a: P3, b: P3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// PMEF `endType` of an E3D connection type (`BWD`, `FBD`, `SWF`, `SCF`, ...).
pub fn end_type(con: &str) -> Option<&'static str> {
    let c = con.trim().to_uppercase();
    Some(match () {
        _ if c.starts_with("BW") => "BW",
        _ if c.starts_with("SW") => "SW",
        _ if c.starts_with("SC") || c.starts_with("TH") || c.starts_with("NPT") => "SC",
        _ if c.starts_with("GR") => "GROOVED",
        _ if c.starts_with("PL") || c == "OPEN" => "PLAIN",
        _ if c.starts_with('F') => "FL",
        _ => return None,
    })
}

fn port(id: &str, at: P3, dir: Option<P3>, bore: f64, con: Option<&str>) -> Value {
    let mut p = json!({ "portId": id, "coordinate": at, "nominalDiameter": bore });
    if let Some(d) = dir {
        let n = dist(d, [0.0; 3]);
        if n > 0.0 {
            p["direction"] = json!(d.map(|x| x / n));
        }
    }
    if let Some(t) = con.and_then(end_type) {
        p["endType"] = json!(t);
    }
    p
}

/// Ports of a component and whether they come from arrive/leave points.
pub fn component_ports(c: &PmlElement, cs: E3DCoordinateSystem, bore: f64) -> (Vec<Value>, bool) {
    let p = |k: &str| point(c, k, cs);
    let b = |k: &str| c.attr_f64(k).unwrap_or(bore);
    match (p("APOS"), p("LPOS")) {
        (Some(a), Some(l)) => {
            let mut ports = vec![
                port("P1", a, p("ADIR"), b("ABOR"), c.attr_str("ACON")),
                port("P2", l, p("LDIR"), b("LBOR"), c.attr_str("LCON")),
            ];
            if let Some(t) = p("P3POS") {
                ports.push(port("P3", t, p("P3DIR"), b("P3BOR"), c.attr_str("P3CON")));
            }
            (ports, true)
        }
        _ => {
            let at = p("POS").unwrap_or([0.0; 3]);
            (vec![port("P0", at, None, bore, None)], false)
        }
    }
}

/// A reference from a branch end to an element outside the branch (`HREF`/`TREF`).
#[derive(Debug, Clone, PartialEq)]
pub struct EndRef {
    /// PMEF `@id` of the component that holds the end port.
    pub component: String,
    /// Its port id.
    pub port: String,
    /// E3D address the branch end refers to (nozzle, tee, other branch).
    pub target: String,
}

/// Open end of the chain being built.
enum End {
    Head { at: P3, bore: f64, href: Option<String> },
    Port { obj: usize, port: usize, at: P3, bore: f64 },
}

fn port_at(o: &Value, i: usize) -> P3 {
    let c = &o["ports"][i]["coordinate"];
    [0, 1, 2].map(|k| c[k].as_f64().unwrap_or(0.0))
}

/// Builds the routing sequence of one branch.
pub struct BranchChain<'a> {
    /// Objects in routing order (components and generated tubes).
    pub objs: Vec<Value>,
    /// Branch ends that refer to other elements.
    pub refs: Vec<EndRef>,
    /// `(@id, portId)` of the branch head and tail ports, for references to this branch.
    pub ends: [Option<(String, String)>; 2],
    end: Option<End>,
    new_tube: &'a mut dyn FnMut() -> String,
    tube_template: Value,
}

impl<'a> BranchChain<'a> {
    /// Start a chain at the branch head. `tube_template` holds the fields every generated
    /// tube shares (`pmefVersion`, `isPartOf`, `revision`); `new_tube` yields tube `@id`s.
    pub fn new(
        bran: &PmlElement,
        cs: E3DCoordinateSystem,
        bore: f64,
        tube_template: Value,
        new_tube: &'a mut dyn FnMut() -> String,
    ) -> Self {
        let bore = bran.attr_f64("HBOR").unwrap_or(bore);
        let end = point(bran, "HPOS", cs).map(|at| End::Head {
            at,
            bore,
            href: bran.attr_str("HREF").map(str::to_owned),
        });
        Self { objs: Vec::new(), refs: Vec::new(), ends: [None, None], end, new_tube, tube_template }
    }

    fn link(&mut self, end: End, obj: usize, port: usize) {
        let id = self.objs[obj]["@id"].as_str().unwrap_or_default().to_owned();
        match end {
            End::Head { href, .. } => {
                let port = self.objs[obj]["ports"][port]["portId"].as_str().unwrap_or("P1").to_owned();
                self.ends[0] = Some((id.clone(), port.clone()));
                if let Some(target) = href {
                    self.refs.push(EndRef { component: id, port, target });
                }
            }
            End::Port { obj: o, port: p, .. } => {
                let prev = self.objs[o]["@id"].clone();
                self.objs[o]["ports"][p]["connectedTo"] = json!(id);
                self.objs[obj]["ports"][port]["connectedTo"] = prev;
            }
        }
    }

    /// Close the gap from the open end to `at` with a generated tube; returns the new end.
    fn bridge(&mut self, end: End, at: P3) -> End {
        let (from, bore) = match &end {
            End::Head { at, bore, .. } | End::Port { at, bore, .. } => (*at, *bore),
        };
        let len = dist(from, at);
        if len <= JOIN_TOLERANCE_MM {
            return end;
        }
        let d = [0, 1, 2].map(|k| (at[k] - from[k]) / len);
        let mut tube = self.tube_template.clone();
        tube["@type"] = json!("pmef:Pipe");
        tube["@id"] = json!((self.new_tube)());
        tube["componentSpec"] = json!({ "componentClass": "PIPE" });
        tube["pipeLength"] = json!(len);
        tube["customAttributes"] = json!({ "e3dGenerated": "TUBI" });
        tube["ports"] = json!([
            port("P1", from, Some(d.map(|x| -x)), bore, None),
            port("P2", at, Some(d), bore, None),
        ]);
        self.objs.push(tube);
        let t = self.objs.len() - 1;
        self.link(end, t, 0);
        End::Port { obj: t, port: 1, at, bore }
    }

    /// Append a mapped component. `connected` = its ports come from arrive/leave points.
    /// A support without p-points sits on the pipe and does not interrupt the chain; any
    /// other component without them breaks it (fallback, spec 05).
    pub fn push(&mut self, obj: Value, connected: bool, inline: bool) {
        self.objs.push(obj);
        let i = self.objs.len() - 1;
        if !connected {
            if inline {
                self.end = None;
            }
            return;
        }
        let arrive = port_at(&self.objs[i], 0);
        if let Some(end) = self.end.take() {
            let end = self.bridge(end, arrive);
            self.link(end, i, 0);
        }
        let bore = self.objs[i]["ports"][1]["nominalDiameter"].as_f64().unwrap_or(0.0);
        self.end = Some(End::Port { obj: i, port: 1, at: port_at(&self.objs[i], 1), bore });
    }

    /// Close the chain at the branch tail (`TPOS`, `TREF`).
    /// Returns the objects, the end references and the head/tail ports ([`Self::ends`]).
    pub fn finish(
        mut self, bran: &PmlElement, cs: E3DCoordinateSystem,
    ) -> (Vec<Value>, Vec<EndRef>, [Option<(String, String)>; 2]) {
        if let (Some(end), Some(tail)) = (self.end.take(), point(bran, "TPOS", cs)) {
            if let End::Port { obj, port, .. } = self.bridge(end, tail) {
                let component = self.objs[obj]["@id"].as_str().unwrap_or_default().to_owned();
                let port = self.objs[obj]["ports"][port]["portId"].as_str().unwrap_or("P2").to_owned();
                self.ends[1] = Some((component.clone(), port.clone()));
                if let Some(target) = bran.attr_str("TREF") {
                    self.refs.push(EndRef { component, port, target: target.to_owned() });
                }
            }
        }
        (self.objs, self.refs, self.ends)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_connection_types() {
        assert_eq!(end_type("BWD"), Some("BW"));
        assert_eq!(end_type("FBD"), Some("FL"));
        assert_eq!(end_type("swf"), Some("SW"));
        assert_eq!(end_type("SCF"), Some("SC"));
        assert_eq!(end_type("XYZ"), None);
    }
}
