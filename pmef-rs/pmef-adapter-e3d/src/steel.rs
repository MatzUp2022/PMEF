//! Structural steel: STRU → FRMW → SBFR (nested `pmef:SteelSystem`, RFC-0013), SCTN
//! (`pmef:SteelMember` from POSS/POSE/BANG/SPRE) and PANE (`pmef:SteelPlate` from the PLOO
//! loop with its PAVE vertices; PLOO HEIG = thickness).
//!
//! Profiles: the last component of SPRE (`/ESHUL-PRF/HEB300` → `HEB300`) is looked up as
//! `EN:<name>` and `AISC:<name>` in the shared catalogs (`FB` = `FLAT`, `RB` round bar);
//! anything else is a project profile `<PROJECT>:<name>` (spec 05 §7.2) that a project
//! catalog of the package defines (spec 02 §7.1); the native name is kept in
//! `customAttributes.nativeProfileName`.

use crate::pml::{E3dElementType, PmlElement};
use crate::E3DCoordinateSystem;
use serde_json::{json, Value};

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: V3) -> Option<V3> {
    let l = dot(a, a).sqrt();
    (l > 1e-9).then(|| [a[0] / l, a[1] / l, a[2] / l])
}
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
fn round_unit(v: V3) -> V3 {
    v.map(|c| (c * 1e6).round() / 1e6)
}

/// PMEF profile ID for an E3D profile reference (SPRE or a profile name).
pub fn profile_id(spre: &str, project_code: &str) -> String {
    let name = spre.rsplit('/').next().unwrap_or(spre).trim();
    let shared = |id: &str| {
        ["profiles-en", "profiles-en-extended", "profiles-aisc", "profiles-aisc-extended"]
            .iter()
            .filter_map(|c| pmef_catalogs::document(c))
            .any(|d| d["entries"].as_array().is_some_and(|es| es.iter().any(|e| e["catalogId"] == id)))
    };
    // family letters upper-case, dimensions as written (`fb200x15` → `FB200x15`)
    let split = name.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(name.len());
    let (fam, dims) = name.split_at(split);
    let fam = fam.to_uppercase();
    let fam = if fam == "FB" { "FLAT".to_owned() } else { fam };
    let candidates = [format!("EN:{fam}{dims}"), format!("AISC:{name}")];
    candidates
        .into_iter()
        .find(|c| shared(c))
        .unwrap_or_else(|| format!("{}:{name}", project_code.to_uppercase()))
}

/// Member type from E3D GTYP (BEAM, COLU, BRAC, …).
fn member_type(gtyp: Option<&str>) -> &'static str {
    match gtyp.unwrap_or("").to_uppercase().as_str() {
        "COLU" | "COLUMN" => "COLUMN",
        "BRAC" | "BRACE" => "BRACE",
        "PURL" => "PURLIN",
        "HAND" | "HANDRAIL" => "HANDRAIL",
        "STRI" | "STRINGER" => "STRINGER",
        "BEAM" => "BEAM",
        _ => "GENERIC",
    }
}

/// Plate type from E3D PURP (GRAT, CHEQ, PANL/CLAD, …).
fn plate_type(purp: Option<&str>) -> &'static str {
    match purp.unwrap_or("").to_uppercase().as_str() {
        p if p.starts_with("GRAT") => "GRATING",
        p if p.starts_with("CHEQ") || p.starts_with("TREAD") => "CHEQUERED",
        p if p.starts_with("PAN") || p.starts_with("CLAD") || p.starts_with("WALL") => "PANEL",
        "" | "PLAT" | "PLATE" => "PLATE",
        _ => "GENERIC",
    }
}

/// Local plane of a planar contour: origin (first vertex), unit normal (Newell), refDirection
/// (first edge) and the 2D outline. `None` for fewer than 3 vertices or a degenerate loop.
pub fn plate_frame(pts: &[V3]) -> Option<(V3, V3, V3, Vec<[f64; 2]>)> {
    if pts.len() < 3 {
        return None;
    }
    let mut n = [0.0; 3];
    for (i, a) in pts.iter().enumerate() {
        let b = pts[(i + 1) % pts.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let n = unit(n)?;
    let o = pts[0];
    let x = pts[1..].iter().find_map(|p| {
        let e = sub(*p, o);
        unit(sub(e, n.map(|c| c * dot(e, n))))
    })?;
    let y = cross(n, x);
    let outline = pts.iter().map(|p| {
        let d = sub(*p, o);
        [round3(dot(d, x)), round3(dot(d, y))]
    });
    Some((o, round_unit(n), round_unit(x), outline.collect()))
}

/// Maps one STRU tree into PMEF steel objects.
pub struct SteelMapper<'a> {
    pub project_code: &'a str,
    pub unit_id: String,
    pub cs: E3DCoordinateSystem,
    pub warnings: Vec<String>,
    /// `@id` → DB address it was issued for; shared across structures of one export.
    pub issued: std::cell::RefCell<std::collections::HashMap<String, String>>,
}

impl SteelMapper<'_> {
    /// `@id` from the element name. If another element (other DB address) already got that
    /// `@id`, e.g. `/STR-A/FRAME` and `/STR-B/FRAME`, the full address is used instead
    /// (`STR-B.FRAME`), so identities stay unique and deterministic for a given export.
    fn id(&self, domain: &str, elem: &PmlElement) -> String {
        let clean = |s: &str| -> String {
            s.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.')).collect()
        };
        let addr = &elem.db_address;
        let short = format!("urn:pmef:{domain}:{}:{}", self.project_code, clean(elem.local_name()));
        let mut issued = self.issued.borrow_mut();
        match issued.get(&short) {
            None => {
                issued.insert(short.clone(), addr.clone());
                short
            }
            Some(a) if a == addr => short,
            Some(_) => {
                let full = format!("urn:pmef:{domain}:{}:{}", self.project_code,
                    clean(&addr.trim_start_matches('/').replace('/', ".")));
                issued.insert(full.clone(), addr.clone());
                full
            }
        }
    }

    fn point(&self, elem: &PmlElement, key: &str) -> Option<V3> {
        let (x, y, z) = elem.attr(key)?.as_vec3()?;
        let (x, y, z) = self.cs.to_pmef(x, y, z);
        Some([x, y, z])
    }

    fn revision(elem: &PmlElement) -> Value {
        json!({
            "revisionId": "r2026-01-01-001",
            "changeState": "SHARED",
            "authoringToolObjectId": elem.db_address,
            "authoringTool": "pmef-adapter-e3d 0.9.0"
        })
    }

    /// STRU/FRMW/SBFR subtree → systems, members and plates (parents before children).
    pub fn map_system(&mut self, elem: &PmlElement, parent: &str, out: &mut Vec<Value>) {
        let level = match elem.element_type {
            E3dElementType::Framework => "FRAMEWORK",
            E3dElementType::SubFramework => "SUBFRAMEWORK",
            _ => "STRUCTURE",
        };
        let sys_id = self.id("obj", elem);
        let mut members = Vec::new();
        let mut plates = Vec::new();
        let mut children = Vec::new();
        for c in &elem.children {
            match c.element_type {
                E3dElementType::Section => {
                    if let Some(m) = self.map_section(c, &sys_id) {
                        members.push(m["@id"].clone());
                        children.push(m);
                    }
                }
                E3dElementType::Panel => {
                    if let Some(p) = self.map_panel(c, &sys_id) {
                        plates.push(p["@id"].clone());
                        children.push(p);
                    }
                }
                _ => {}
            }
        }
        out.push(json!({
            "@type": "pmef:SteelSystem",
            "@id": sys_id,
            "pmefVersion": "0.10.0",
            "isPartOf": parent,
            "systemName": elem.attr_str("DESC").unwrap_or(elem.local_name()),
            "systemLevel": level,
            "memberIds": members,
            "plateIds": plates,
            "revision": Self::revision(elem)
        }));
        out.extend(children);
        for c in &elem.children {
            if matches!(c.element_type, E3dElementType::Framework | E3dElementType::SubFramework) {
                self.map_system(c, &sys_id, out);
            }
        }
    }

    fn map_section(&mut self, s: &PmlElement, system: &str) -> Option<Value> {
        let (Some(a), Some(b)) = (self.point(s, "POSS"), self.point(s, "POSE")) else {
            self.warnings.push(format!("{}: SCTN without POSS/POSE skipped", s.db_address));
            return None;
        };
        let spre = s.attr_str("SPRE").unwrap_or("");
        if spre.is_empty() {
            self.warnings.push(format!("{}: SCTN without SPRE skipped", s.db_address));
            return None;
        }
        Some(json!({
            "@type": "pmef:SteelMember",
            "@id": self.id("obj", s),
            "pmefVersion": "0.10.0",
            "isPartOf": system,
            "memberMark": s.attr_str("NAME").unwrap_or(s.local_name()),
            "memberType": member_type(s.attr_str("GTYP")),
            "profileId": profile_id(spre, self.project_code),
            "startPoint": a,
            "endPoint": b,
            "rollAngle": s.attr_f64("BANG").unwrap_or(0.0),
            "customAttributes": {
                "e3dDbAddress": s.db_address,
                "nativeProfileName": spre.rsplit('/').next().unwrap_or(spre)
            },
            "revision": Self::revision(s)
        }))
    }

    fn map_panel(&mut self, p: &PmlElement, system: &str) -> Option<Value> {
        let lp = p.children.iter().find(|c| c.element_type == E3dElementType::PanelLoop);
        let pts: Vec<V3> = lp
            .into_iter()
            .flat_map(|l| l.children.iter())
            .filter(|v| v.element_type == E3dElementType::PanelVertex)
            .filter_map(|v| self.point(v, "POS"))
            .collect();
        let thickness = lp.and_then(|l| l.attr_f64("HEIG")).filter(|t| *t > 0.0);
        let (Some((o, n, x, outline)), Some(t)) = (plate_frame(&pts), thickness) else {
            self.warnings.push(format!("{}: PANE without a valid PLOO (≥ 3 PAVE, HEIG > 0) skipped", p.db_address));
            return None;
        };
        Some(json!({
            "@type": "pmef:SteelPlate",
            "@id": self.id("obj", p),
            "pmefVersion": "0.10.0",
            "isPartOf": system,
            "plateMark": p.attr_str("NAME").unwrap_or(p.local_name()),
            "plateType": plate_type(p.attr_str("PURP")),
            "origin": o,
            "normal": n,
            "refDirection": x,
            "outline": outline,
            "thickness": t,
            "customAttributes": { "e3dDbAddress": p.db_address },
            "revision": Self::revision(p)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plate_frame_of_a_horizontal_rectangle() {
        let (o, n, x, out) =
            plate_frame(&[[0.0, 0.0, 3000.0], [2000.0, 0.0, 3000.0], [2000.0, 1000.0, 3000.0], [0.0, 1000.0, 3000.0]])
                .unwrap();
        assert_eq!((o, n, x), ([0.0, 0.0, 3000.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]));
        assert_eq!(out, [[0.0, 0.0], [2000.0, 0.0], [2000.0, 1000.0], [0.0, 1000.0]]);
        assert!(plate_frame(&[[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]).is_none());
    }

    #[test]
    fn profiles_shared_or_project() {
        assert_eq!(profile_id("/ESHUL-PRF/HEB300", "demo"), "EN:HEB300");
        assert_eq!(profile_id("/PRF/FB200x15", "demo"), "EN:FLAT200x15");
        assert_eq!(profile_id("/PRF/RB20", "demo"), "EN:RB20");
        assert_eq!(profile_id("/ESHUL-PRF/HP260x10", "huldra"), "HULDRA:HP260x10");
    }
}
