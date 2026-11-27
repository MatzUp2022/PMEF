//! Conservative bounds of a PMEF parametric primitive given as schema JSON
//! (`schemas/pmef-geometry.schema.json`, spec 04). Used by `pmef-route` for obstacles.

use crate::{member_aabb, primitive_aabb, Aabb, PrimitiveParams, PrimitiveType, SectionSize, Vec3};
use serde_json::Value;

fn p3(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([
        a.first()?.as_f64()?,
        a.get(1)?.as_f64()?,
        a.get(2)?.as_f64()?,
    ])
}

fn v3(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

fn inflate(b: Aabb, d: f64) -> Aabb {
    Aabb::new(b.min - Vec3::new(d, d, d), b.max + Vec3::new(d, d, d))
}

fn point(c: [f64; 3]) -> Aabb {
    Aabb::from_point(v3(c))
}

/// Conservative AABB of a schema primitive; `None` for unsupported types, missing required
/// fields and `MESH_REF` (no extent in the schema). `COMPOSITE` is the union of its children.
/// `section` returns the [`SectionSize`] of a catalog profile (`STEEL_PROFILE.profileId`);
/// without it a member has no bounds. `rollDeg` is honoured (see [`member_aabb`]).
pub fn schema_primitive_aabb(
    p: &Value,
    section: &dyn Fn(&str) -> Option<SectionSize>,
) -> Option<Aabb> {
    let f = |k: &str| p[k].as_f64();
    let t = p["type"].as_str()?;
    match t {
        "COMPOSITE" => {
            return p["children"]
                .as_array()?
                .iter()
                .filter_map(|c| schema_primitive_aabb(&c["primitive"], section))
                .reduce(|a, b| a.union(&b));
        }
        "MESH_REF" => return None,
        "STEEL_PROFILE" => {
            let (a, b) = (p3(&p["startPt"])?, p3(&p["endPt"])?);
            let s = section(p["profileId"].as_str()?)?;
            return Some(member_aabb(v3(a), v3(b), s, f("rollDeg").unwrap_or(0.0)));
        }
        "CABLE_TRAY" => {
            let points: Vec<[f64; 3]> = p["centerline"]
                .as_array()?
                .iter()
                .map(p3)
                .collect::<Option<_>>()?;
            if points.len() < 2 {
                return None;
            }
            let params = PrimitiveParams {
                points: Some(points),
                width: Some(f("width")?),
                height: f("height"),
                ..Default::default()
            };
            return Some(primitive_aabb(
                &PrimitiveType::CableTray,
                Vec3::zero(),
                &params,
            ));
        }
        "VALVE_BODY" => {
            let params = PrimitiveParams {
                radius: Some(f("boreD")? / 2.0),
                length: Some(f("faceToFace")?),
                axis: Some(p3(&p["direction"])?),
                height: f("bodyHeight"),
                actuator_height: f("actuatorHeight"),
                actuator_axis: p3(&p["actuatorDir"]),
                ..Default::default()
            };
            return Some(primitive_aabb(
                &PrimitiveType::ValveBody,
                v3(p3(&p["center"])?),
                &params,
            ));
        }
        _ => {}
    }
    let c = p3(&p["center"]).or_else(|| p3(&p["origin"]))?;
    let axis = p3(&p["axis"]).unwrap_or([0.0, 0.0, 1.0]);
    // Disc of radius r perpendicular to the axis, swept along the axis by `len`.
    let swept = |r: f64, len: f64| {
        let ext = axis.map(|a| r * (1.0 - a * a).max(0.0).sqrt());
        let end = [0, 1, 2].map(|i| c[i] + axis[i] * len);
        Aabb::new(
            Vec3::new(
                c[0].min(end[0]) - ext[0],
                c[1].min(end[1]) - ext[1],
                c[2].min(end[2]) - ext[2],
            ),
            Vec3::new(
                c[0].max(end[0]) + ext[0],
                c[1].max(end[1]) + ext[1],
                c[2].max(end[2]) + ext[2],
            ),
        )
    };
    // Field names per schemas/pmef-geometry.schema.json; legacy aliases are accepted.
    match t {
        "CYLINDER" | "PIPE" => Some(swept(f("radius")?, f("length").or(f("height"))?)),
        // `depth` is optional; the shell radius bounds every head type.
        "DISH" => {
            let r = f("shellRadius").or(f("radius"))?;
            Some(swept(r, f("depth").unwrap_or(r)))
        }
        "CONE" | "SNOUT" => {
            let r1 = f("r1").or(f("radius"))?;
            let r2 = f("r2").or(f("radius2")).unwrap_or(0.0);
            let off = f("offsetX")
                .unwrap_or(0.0)
                .hypot(f("offsetY").unwrap_or(0.0));
            // Sheared end faces (spec 04 §4.3) move face points along the axis by up to
            // r·|(tan shearX, tan shearY)|; inflating by that bound stays conservative.
            let shear = |x: &str, y: &str, r: f64| {
                let t = |k: &str| f(k).unwrap_or(0.0).to_radians().tan();
                r * t(x).hypot(t(y))
            };
            let overhang =
                shear("bottomShearX", "bottomShearY", r1).max(shear("topShearX", "topShearY", r2));
            Some(inflate(
                swept(r1.max(r2) + off, f("length").or(f("height"))?),
                overhang,
            ))
        }
        "SPHERE" => Some(inflate(point(c), f("radius")?)),
        "CIRC_TORUS" => Some(inflate(point(c), f("torusRadius")? + f("tubeRadius")?)),
        "RECT_TORUS" => Some(inflate(
            point(c),
            f("outerRadius")?.hypot(f("height")? / 2.0),
        )),
        // Half-diagonal of the larger face plus the top offset (the local frame of
        // spec 04 §4.3 is not needed for a conservative box).
        "PYRAMID" => {
            let bottom = f("bottomX")?.hypot(f("bottomY")?) / 2.0;
            let top = f("topX")?.hypot(f("topY")?) / 2.0
                + f("offsetX")
                    .unwrap_or(0.0)
                    .hypot(f("offsetY").unwrap_or(0.0));
            Some(swept(bottom.max(top), f("height")?))
        }
        "BOX" => {
            let h = [
                f("xLen").or(f("length"))?,
                f("yLen").or(f("width"))?,
                f("zLen").or(f("height")).or(f("depth"))?,
            ];
            let rotated = ["rotationX", "rotationY", "rotationZ"]
                .iter()
                .any(|k| f(k).is_some_and(|v| v.abs() > 1e-9));
            if rotated {
                // Rotated box: bounded by its circumscribed sphere.
                Some(inflate(
                    point(c),
                    (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt() / 2.0,
                ))
            } else {
                Some(Aabb::new(
                    Vec3::new(c[0] - h[0] / 2.0, c[1] - h[1] / 2.0, c[2] - h[2] / 2.0),
                    Vec3::new(c[0] + h[0] / 2.0, c[1] + h[1] / 2.0, c[2] + h[2] / 2.0),
                ))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn no_profiles(_: &str) -> Option<SectionSize> {
        None
    }

    fn b(v: Value) -> Aabb {
        schema_primitive_aabb(&v, &no_profiles).unwrap()
    }

    #[test]
    fn sheared_snout_box_covers_the_tilted_face() {
        let x = b(
            json!({"type": "SNOUT", "center": [0.0, 0.0, 0.0], "axis": [0.0, 0.0, 1.0],
            "r1": 100.0, "r2": 100.0, "length": 300.0, "bottomShearX": 45.0}),
        );
        assert!(x.min.z <= -100.0 + 1e-9, "{x:?}");
    }

    #[test]
    fn pyramid_and_rect_torus_boxes() {
        let x = b(
            json!({"type": "PYRAMID", "center": [0, 0, 0], "axis": [0, 0, 1],
            "bottomX": 600, "bottomY": 800, "topX": 0, "topY": 0, "height": 300}),
        );
        assert_eq!(
            (x.min.x, x.max.x, x.min.z, x.max.z),
            (-500.0, 500.0, 0.0, 300.0)
        );
        let x = b(
            json!({"type": "RECT_TORUS", "center": [0, 0, 0], "axis": [0, 0, 1],
            "startDir": [1, 0, 0], "innerRadius": 300, "outerRadius": 400, "height": 600, "angleDeg": 90}),
        );
        assert_eq!(x.max.x, 500.0);
    }

    #[test]
    fn steel_profile_needs_the_profile_size() {
        let m = json!({"type": "STEEL_PROFILE", "startPt": [0, 0, 0], "endPt": [6000, 0, 0],
            "profileId": "EN:HEB300"});
        assert!(schema_primitive_aabb(&m, &no_profiles).is_none());
        let heb300 = |id: &str| {
            (id == "EN:HEB300").then_some(SectionSize {
                half_h: 150.0,
                half_b: 150.0,
                round: false,
            })
        };
        let x = schema_primitive_aabb(&m, &heb300).unwrap();
        assert_eq!(
            (x.min.x, x.max.x, x.min.y, x.max.z),
            (-150.0, 6150.0, -150.0, 150.0)
        );
        // rolled 45°: the flange corners reach the half-diagonal
        let mut rolled = m.clone();
        rolled["rollDeg"] = 45.0.into();
        let x = schema_primitive_aabb(&rolled, &heb300).unwrap();
        assert!((x.max.y - 300f64.hypot(300.0) / 2.0).abs() < 1e-9, "{x:?}");
    }

    #[test]
    fn valve_tray_composite_and_mesh() {
        let v = b(
            json!({"type": "VALVE_BODY", "valveBodyType": "GATE", "center": [0, 0, 0],
            "direction": [1, 0, 0], "boreD": 100, "faceToFace": 229, "actuatorHeight": 600,
            "actuatorDir": [0, 0, 1]}),
        );
        // no flange OD in the schema → heuristic envelope 3.5 × boreD around flow axis and actuator
        let env = 100.0 * crate::VALVE_ENVELOPE_FACTOR;
        assert_eq!((v.min.x, v.max.x, v.max.z), (-env, env, 600.0 + env));
        let t = b(
            json!({"type": "CABLE_TRAY", "centerline": [[0, 0, 0], [1000, 0, 0]],
            "width": 300, "height": 400}),
        );
        assert_eq!((t.min.x, t.max.x), (-250.0, 1250.0));
        let c = b(json!({"type": "COMPOSITE", "children": [
            {"partName": "a", "primitive": {"type": "SPHERE", "center": [0, 0, 0], "radius": 10}},
            {"partName": "m", "primitive": {"type": "MESH_REF", "meshUri": "x.glb"}},
            {"partName": "b", "primitive": {"type": "SPHERE", "center": [100, 0, 0], "radius": 10}}]}));
        assert_eq!((c.min.x, c.max.x), (-10.0, 110.0));
        assert!(schema_primitive_aabb(
            &json!({"type": "MESH_REF", "meshUri": "x.glb"}),
            &no_profiles
        )
        .is_none());
    }
}
