//! Equipment geometry from E3D design primitives (PML export).
//!
//! Replaces the former RVM binary parser (removed — see
//! `docs/design-decisions.md`, "E3D geometry via PML, no RVM parsing").
//! Geometry now comes exclusively from the official PML export: the
//! export script writes each equipment design primitive as a child element
//! of its `EQUI` with explicit, world-coordinate attributes.
//!
//! ## PML export contract (all lengths in mm, world coordinates)
//!
//! | Element | Attributes | PMEF primitive |
//! |---------|------------|----------------|
//! | `BOX`   | `POS` (centre), `XLEN` `YLEN` `ZLEN`, optional `ROTX` `ROTY` `ROTZ` (deg) | `BOX` |
//! | `CYLI`  | `POS` (centre), `AXIS`, `DIAM` `HEIG` | `CYLINDER` |
//! | `CONE`  | `POS` (centre), `AXIS` (bottom → top), `DBOT` `DTOP` `HEIG` | `CONE` |
//! | `SNOU`  | `POS` (centre), `AXIS` (bottom → top), `DBOT` `DTOP` `HEIG`, optional `XOFF` `YOFF` | `SNOUT` |
//! | `DISH`  | `POS` (centre of base circle), `AXIS` (outward), `DIAM` `HEIG`, optional `RADI` (knuckle) | `DISH` |
//!
//! `AXIS` is the primitive's height axis as a direction vector in world
//! coordinates (default `0 0 1`). The PML script obtains it from the
//! element's orientation; the adapter never derives geometry from binary files.

use crate::pml::{E3dElementType, PmlElement};
use crate::E3DCoordinateSystem;
use serde_json::{json, Value};

/// Tolerance [mm] used to classify dish heads.
const DISH_TOL_MM: f64 = 1.0;

/// Result of mapping the design primitives of one equipment element.
#[derive(Debug, Default)]
pub struct EquipmentGeometry {
    /// PMEF primitive (`COMPOSITE` if more than one part); `None` if no usable primitive.
    pub primitive: Option<Value>,
    /// Human-readable problems (skipped or approximated primitives).
    pub issues: Vec<String>,
}

/// Map all design-primitive children of an `EQUI` element to a PMEF primitive.
pub fn equipment_geometry(equi: &PmlElement, cs: E3DCoordinateSystem) -> EquipmentGeometry {
    let mut parts: Vec<(String, Value)> = Vec::new();
    let mut issues = Vec::new();

    for (n, child) in equi.children.iter().enumerate() {
        if !is_primitive(&child.element_type) {
            continue;
        }
        let part_name = child
            .attr_str("NAME")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{:?}-{}", child.element_type, n + 1));
        match map_primitive(child, cs) {
            Ok((prim, mut notes)) => {
                issues.extend(notes.drain(..).map(|m| format!("{}/{part_name}: {m}", equi.local_name())));
                parts.push((part_name, prim));
            }
            Err(msg) => issues.push(format!("{}/{part_name}: skipped — {msg}", equi.local_name())),
        }
    }

    let primitive = match parts.len() {
        0 => None,
        1 => Some(parts.pop().expect("one part").1),
        _ => Some(json!({
            "type": "COMPOSITE",
            "children": parts.into_iter()
                .map(|(name, p)| json!({ "partName": name, "primitive": p }))
                .collect::<Vec<_>>()
        })),
    };
    EquipmentGeometry { primitive, issues }
}

/// True for the E3D design primitive element types handled here.
pub fn is_primitive(t: &E3dElementType) -> bool {
    matches!(
        t,
        E3dElementType::PrimBox
            | E3dElementType::PrimCylinder
            | E3dElementType::PrimCone
            | E3dElementType::PrimSnout
            | E3dElementType::PrimDish
    )
}

type Mapped = Result<(Value, Vec<String>), String>;

fn map_primitive(p: &PmlElement, cs: E3DCoordinateSystem) -> Mapped {
    let pos = p.attr("POS").and_then(|v| v.as_vec3()).ok_or("missing POS")?;
    let pos = cs.to_pmef(pos.0, pos.1, pos.2);
    let axis = p.attr("AXIS").and_then(|v| v.as_vec3()).unwrap_or((0.0, 0.0, 1.0));
    let axis = normalise(cs.to_pmef(axis.0, axis.1, axis.2)).ok_or("AXIS is a zero vector")?;
    let mut notes = Vec::new();

    let prim = match p.element_type {
        E3dElementType::PrimBox => {
            let (x, y, z) = (pos_dim(p, "XLEN")?, pos_dim(p, "YLEN")?, pos_dim(p, "ZLEN")?);
            let rot = |k| p.attr_f64(k).unwrap_or(0.0);
            let (rx, ry, rz) = (rot("ROTX"), rot("ROTY"), rot("ROTZ"));
            if cs != E3DCoordinateSystem::XEastYNorthZUp && (rx, ry, rz) != (0.0, 0.0, 0.0) {
                return Err("box rotations are only supported for the Z-up coordinate system".into());
            }
            json!({ "type": "BOX", "center": arr(pos), "xLen": x, "yLen": y, "zLen": z,
                    "rotationX": rx, "rotationY": ry, "rotationZ": rz })
        }
        E3dElementType::PrimCylinder => {
            let (d, h) = (pos_dim(p, "DIAM")?, pos_dim(p, "HEIG")?);
            // PMEF: centre of the start face; E3D POS is the geometric centre.
            json!({ "type": "CYLINDER", "center": arr(offset(pos, axis, -h / 2.0)),
                    "axis": arr(axis), "radius": d / 2.0, "length": h })
        }
        E3dElementType::PrimCone | E3dElementType::PrimSnout => {
            let h = pos_dim(p, "HEIG")?;
            let (dbot, dtop) = (non_neg(p, "DBOT")?, non_neg(p, "DTOP")?);
            if dbot == 0.0 && dtop == 0.0 {
                return Err("DBOT and DTOP are both 0".into());
            }
            let start = arr(offset(pos, axis, -h / 2.0));
            if p.element_type == E3dElementType::PrimCone {
                json!({ "type": "CONE", "center": start, "axis": arr(axis),
                        "r1": dbot / 2.0, "r2": dtop / 2.0, "length": h })
            } else {
                json!({ "type": "SNOUT", "center": start, "axis": arr(axis),
                        "r1": dbot / 2.0, "r2": dtop / 2.0, "length": h,
                        "offsetX": p.attr_f64("XOFF").unwrap_or(0.0),
                        "offsetY": p.attr_f64("YOFF").unwrap_or(0.0) })
            }
        }
        E3dElementType::PrimDish => {
            let (d, h) = (pos_dim(p, "DIAM")?, pos_dim(p, "HEIG")?);
            let knuckle = p.attr_f64("RADI").filter(|r| *r > 0.0);
            let r = d / 2.0;
            let dish_type = if knuckle.is_some() {
                "TORISPHERICAL"
            } else if (h - r).abs() <= DISH_TOL_MM {
                "HEMISPHERICAL"
            } else {
                if (h - r / 2.0).abs() > DISH_TOL_MM {
                    notes.push(format!(
                        "ellipsoidal head with depth {h} mm is not 2:1; mapped to ELLIPTICAL_2:1, depth preserved"
                    ));
                }
                "ELLIPTICAL_2:1"
            };
            let mut v = json!({ "type": "DISH", "center": arr(pos), "axis": arr(axis),
                                "shellRadius": r, "dishType": dish_type, "depth": h });
            if let Some(k) = knuckle {
                v["knuckleRadius"] = json!(k);
            }
            v
        }
        _ => return Err(format!("not a design primitive: {:?}", p.element_type)),
    };
    Ok((prim, notes))
}

fn pos_dim(p: &PmlElement, key: &'static str) -> Result<f64, String> {
    match p.attr_f64(key) {
        Some(v) if v > 0.0 => Ok(v),
        Some(v) => Err(format!("{key} must be > 0, got {v}")),
        None => Err(format!("missing {key}")),
    }
}

fn non_neg(p: &PmlElement, key: &'static str) -> Result<f64, String> {
    match p.attr_f64(key) {
        Some(v) if v >= 0.0 => Ok(v),
        Some(v) => Err(format!("{key} must be >= 0, got {v}")),
        None => Err(format!("missing {key}")),
    }
}

fn normalise(v: (f64, f64, f64)) -> Option<(f64, f64, f64)> {
    let len = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt();
    (len > 1e-9).then(|| (v.0 / len, v.1 / len, v.2 / len))
}

fn offset(p: (f64, f64, f64), dir: (f64, f64, f64), d: f64) -> (f64, f64, f64) {
    (p.0 + dir.0 * d, p.1 + dir.1 * d, p.2 + dir.2 * d)
}

fn arr(v: (f64, f64, f64)) -> Value {
    json!([v.0, v.1, v.2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pml::parse_pml_text;

    const Z_UP: E3DCoordinateSystem = E3DCoordinateSystem::XEastYNorthZUp;

    fn equi(body: &str) -> PmlElement {
        let text = format!("EQUI /S/Z/EQUI-V-1\n  TAG 'V-1'\n{body}");
        parse_pml_text(&text).unwrap().remove(0)
    }

    #[test]
    fn cylinder_centre_moves_to_start_face() {
        let e = equi("  CYLI\n    POS 1000.0 2000.0 1500.0\n    AXIS 0.0 0.0 1.0\n    DIAM 1200\n    HEIG 2000\n");
        let g = equipment_geometry(&e, Z_UP);
        let p = g.primitive.unwrap();
        assert_eq!(p["type"], "CYLINDER");
        assert_eq!(p["center"], json!([1000.0, 2000.0, 500.0]));
        assert_eq!(p["radius"], 600.0);
        assert_eq!(p["length"], 2000.0);
        assert!(g.issues.is_empty());
    }

    #[test]
    fn vessel_is_composite_with_heads() {
        let e = equi(
            "  CYLI\n    NAME shell\n    POS 0.0 0.0 1100.0\n    DIAM 1200\n    HEIG 2200\n\
             \x20 DISH\n    NAME top\n    POS 0.0 0.0 2200.0\n    DIAM 1200\n    HEIG 300\n\
             \x20 DISH\n    NAME bottom\n    POS 0.0 0.0 0.0\n    AXIS 0.0 0.0 -1.0\n    DIAM 1200\n    HEIG 600\n",
        );
        let g = equipment_geometry(&e, Z_UP);
        let p = g.primitive.unwrap();
        assert_eq!(p["type"], "COMPOSITE");
        let c = p["children"].as_array().unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!(c[1]["partName"], "top");
        assert_eq!(c[1]["primitive"]["dishType"], "ELLIPTICAL_2:1");
        assert_eq!(c[2]["primitive"]["dishType"], "HEMISPHERICAL");
        assert_eq!(c[2]["primitive"]["axis"], json!([0.0, 0.0, -1.0]));
    }

    #[test]
    fn torispherical_dish_keeps_knuckle() {
        let e = equi("  DISH\n    POS 0.0 0.0 0.0\n    DIAM 1000\n    HEIG 200\n    RADI 100\n");
        let p = equipment_geometry(&e, Z_UP).primitive.unwrap();
        assert_eq!(p["dishType"], "TORISPHERICAL");
        assert_eq!(p["knuckleRadius"], 100.0);
    }

    #[test]
    fn snout_and_box() {
        let e = equi(
            "  SNOU\n    POS 0.0 0.0 50.0\n    DBOT 300\n    DTOP 200\n    HEIG 100\n    XOFF 25\n\
             \x20 BOX\n    POS 0.0 0.0 0.0\n    XLEN 100\n    YLEN 200\n    ZLEN 50\n    ROTZ 90\n",
        );
        let p = equipment_geometry(&e, Z_UP).primitive.unwrap();
        let c = p["children"].as_array().unwrap();
        assert_eq!(c[0]["primitive"]["type"], "SNOUT");
        assert_eq!(c[0]["primitive"]["r1"], 150.0);
        assert_eq!(c[0]["primitive"]["offsetX"], 25.0);
        assert_eq!(c[1]["primitive"]["type"], "BOX");
        assert_eq!(c[1]["primitive"]["rotationZ"], 90.0);
    }

    #[test]
    fn invalid_primitives_are_reported_not_silently_dropped() {
        let e = equi("  CYLI\n    POS 0.0 0.0 0.0\n    HEIG 100\n  CONE\n    POS 0.0 0.0 0.0\n    DBOT 0\n    DTOP 0\n    HEIG 10\n");
        let g = equipment_geometry(&e, Z_UP);
        assert!(g.primitive.is_none());
        assert_eq!(g.issues.len(), 2);
        assert!(g.issues[0].contains("missing DIAM"));
        assert!(g.issues[1].contains("both 0"));
    }

    #[test]
    fn legacy_pdms_axes_are_converted() {
        // PDMS Y-up: axis (0,1,0) is vertical → PMEF (0,0,1)
        let e = equi("  CYLI\n    POS 0.0 500.0 0.0\n    AXIS 0.0 1.0 0.0\n    DIAM 100\n    HEIG 1000\n");
        let p = equipment_geometry(&e, E3DCoordinateSystem::XEastYUpZSouth).primitive.unwrap();
        assert_eq!(p["axis"], json!([0.0, 0.0, 1.0]));
        assert_eq!(p["center"], json!([0.0, 0.0, 0.0]));
    }

    #[test]
    fn no_primitives_means_no_geometry() {
        let e = equi("  DESC 'no geometry'\n");
        let g = equipment_geometry(&e, Z_UP);
        assert!(g.primitive.is_none() && g.issues.is_empty());
    }
}
