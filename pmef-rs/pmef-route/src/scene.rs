//! Scene: obstacles and nozzles, from explicit boxes, OBJ meshes and PMEF packages.

use crate::geom::{tri_aabb, Aabb, Tri, P3};
use crate::problem::{ObstacleKind, RouteProblem};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// Shape of an obstacle.
#[derive(Debug, Clone)]
pub enum Shape {
    Box(Aabb),
    Mesh(Vec<Tri>),
}

/// One obstacle of the scene.
#[derive(Debug, Clone)]
pub struct Obstacle {
    pub id: String,
    pub kind: ObstacleKind,
    /// Penetration penalty (soft obstacles).
    pub penalty: f64,
    pub shape: Shape,
    pub bounds: Aabb,
}

/// Obstacles plus named nozzles (`<@id>#<mark>` → point, outward direction).
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub obstacles: Vec<Obstacle>,
    pub nozzles: BTreeMap<String, (P3, P3)>,
}

/// Scene loading errors.
#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    #[error("{0}: {1}")]
    Io(String, std::io::Error),
    #[error("{0}:{1}: {2}")]
    Parse(String, usize, String),
    #[error("obstacle {0}: give exactly one of aabb / obj")]
    Shape(String),
}

impl Scene {
    pub fn push(&mut self, id: String, kind: ObstacleKind, penalty: f64, shape: Shape) {
        let bounds = match &shape {
            Shape::Box(b) => *b,
            Shape::Mesh(t) => t
                .iter()
                .map(tri_aabb)
                .reduce(|a, b| a.union(&b))
                .unwrap_or(Aabb {
                    min: [0.0; 3],
                    max: [0.0; 3],
                }),
        };
        self.obstacles.push(Obstacle {
            id,
            kind,
            penalty,
            shape,
            bounds,
        });
    }

    /// Build the scene of a problem; relative paths resolve against `base`.
    pub fn load(problem: &RouteProblem, base: &Path) -> Result<Scene, SceneError> {
        let mut s = Scene::default();
        for f in &problem.pmef_scene {
            let p = base.join(f);
            let text = std::fs::read_to_string(&p)
                .map_err(|e| SceneError::Io(p.display().to_string(), e))?;
            let mut objs = Vec::new();
            for (i, line) in text
                .lines()
                .enumerate()
                .filter(|(_, l)| !l.trim().is_empty())
            {
                let v: Value = serde_json::from_str(line).map_err(|e| {
                    SceneError::Parse(p.display().to_string(), i + 1, e.to_string())
                })?;
                objs.push(v);
            }
            s.add_pmef(&objs, &problem.pmef_ignore);
        }
        for o in &problem.obstacles {
            let pen = o.penalty.unwrap_or(problem.settings.penetration_penalty);
            match (&o.aabb, &o.obj) {
                (Some(b), None) => s.push(o.id.clone(), o.kind, pen, Shape::Box(*b)),
                (None, Some(f)) => {
                    let p = base.join(f);
                    let text = std::fs::read_to_string(&p)
                        .map_err(|e| SceneError::Io(p.display().to_string(), e))?;
                    let tris = parse_obj(&text)
                        .map_err(|(l, m)| SceneError::Parse(p.display().to_string(), l, m))?;
                    s.push(o.id.clone(), o.kind, pen, Shape::Mesh(tris));
                }
                _ => return Err(SceneError::Shape(o.id.clone())),
            }
        }
        Ok(s)
    }

    /// Add PMEF objects: `geometry.boundingBox` → box, piping components → box over their
    /// ports grown by the outside radius, `nozzles` → terminals.
    pub fn add_pmef(&mut self, objs: &[Value], ignore: &[String]) {
        let skip = |id: &str| {
            ignore.iter().any(|g| {
                if g.ends_with('-') || g.ends_with(':') {
                    id.starts_with(g.as_str())
                } else {
                    id == g
                }
            })
        };
        let p3 = |v: &Value| -> Option<P3> {
            let a = v.as_array()?;
            Some([
                a.first()?.as_f64()?,
                a.get(1)?.as_f64()?,
                a.get(2)?.as_f64()?,
            ])
        };
        for o in objs {
            let Some(id) = o["@id"].as_str() else {
                continue;
            };
            for n in o["nozzles"].as_array().into_iter().flatten() {
                if let (Some(c), Some(d)) = (p3(&n["coordinate"]), p3(&n["direction"])) {
                    for key in ["nozzleMark", "nozzleId"] {
                        if let Some(m) = n[key].as_str() {
                            self.nozzles.insert(format!("{id}#{m}"), (c, d));
                        }
                    }
                }
            }
            let owner = o["ownerRef"].as_str().unwrap_or(id);
            if skip(id) || skip(owner) {
                continue;
            }
            if o["@type"] == "pmef:ParametricGeometry" {
                if let Some(b) = primitive_box(&o["primitive"]) {
                    self.push(owner.into(), ObstacleKind::Hard, 0.0, Shape::Box(b));
                }
                continue;
            }
            if let (Some(a), Some(b)) = (p3(&o["startPoint"]), p3(&o["endPoint"])) {
                // Steel member: centreline grown by half the larger profile dimension.
                let half = o["profileId"]
                    .as_str()
                    .and_then(profile_half)
                    .unwrap_or(250.0);
                self.push(
                    id.into(),
                    ObstacleKind::Hard,
                    0.0,
                    Shape::Box(Aabb::from_points(a, b).inflate(half)),
                );
                continue;
            }
            if let Some(b) = o["geometry"]["boundingBox"].as_object() {
                let f = |k: &str| b.get(k).and_then(Value::as_f64);
                if let (Some(x0), Some(x1), Some(y0), Some(y1), Some(z0), Some(z1)) = (
                    f("xMin"),
                    f("xMax"),
                    f("yMin"),
                    f("yMax"),
                    f("zMin"),
                    f("zMax"),
                ) {
                    let bx = Aabb {
                        min: [x0, y0, z0],
                        max: [x1, y1, z1],
                    };
                    self.push(id.into(), ObstacleKind::Hard, 0.0, Shape::Box(bx));
                    continue;
                }
            }
            let ports: Vec<(P3, Option<f64>)> = o["ports"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| Some((p3(&p["coordinate"])?, p["nominalDiameter"].as_f64())))
                .collect();
            if ports.len() >= 2 {
                let mut b = Aabb::from_points(ports[0].0, ports[1].0);
                for (c, _) in &ports[2..] {
                    b = b.union(&Aabb::from_points(*c, *c));
                }
                // Elbow: include the bend centre (intersection of the orthogonal port axes).
                let dn = ports.iter().filter_map(|p| p.1).fold(0.0, f64::max);
                let od = pmef_catalogs::pipe_sizes()
                    .into_iter()
                    .find(|s| s.dn as f64 == dn)
                    .map(|s| s.od_mm)
                    .unwrap_or(dn * 1.1);
                self.push(
                    id.into(),
                    ObstacleKind::Hard,
                    0.0,
                    Shape::Box(b.inflate(od / 2.0)),
                );
            }
        }
    }
}

fn p3v(v: &Value) -> Option<P3> {
    let a = v.as_array()?;
    Some([
        a.first()?.as_f64()?,
        a.get(1)?.as_f64()?,
        a.get(2)?.as_f64()?,
    ])
}

/// Conservative box of a PMEF parametric primitive (spec 04); `None` for unsupported types.
/// Composite primitives are the union of their children.
pub fn primitive_box(p: &Value) -> Option<Aabb> {
    let f = |k: &str| p[k].as_f64();
    let t = p["type"].as_str()?;
    if t == "COMPOSITE" {
        return p["children"]
            .as_array()?
            .iter()
            .filter_map(|c| primitive_box(&c["primitive"]))
            .reduce(|a, b| a.union(&b));
    }
    let c = p3v(&p["center"]).or_else(|| p3v(&p["origin"]))?;
    let axis = p3v(&p["axis"]).unwrap_or([0.0, 0.0, 1.0]);
    // Disc of radius r perpendicular to the axis, swept along axis by `len`.
    let swept = |r: f64, len: f64| {
        let ext = axis.map(|a| r * (1.0 - a * a).max(0.0).sqrt());
        let end = [0, 1, 2].map(|i| c[i] + axis[i] * len);
        Aabb::from_points(c, end)
            .union(&Aabb::from_points(c, c))
            .inflate(0.0)
            .union(&Aabb {
                min: [0, 1, 2].map(|i| c[i].min(end[i]) - ext[i]),
                max: [0, 1, 2].map(|i| c[i].max(end[i]) + ext[i]),
            })
    };
    match t {
        "CYLINDER" | "PIPE" => Some(swept(f("radius")?, f("length").or(f("height"))?)),
        "DISH" => Some(swept(f("shellRadius").or(f("radius"))?, f("depth")?)),
        "CONE" => Some(swept(
            f("radius")?.max(f("radius2").unwrap_or(0.0)),
            f("length").or(f("height"))?,
        )),
        "SPHERE" => Some(Aabb::from_points(c, c).inflate(f("radius")?)),
        "BOX" => {
            let h = [f("length")?, f("width")?, f("height").or(f("depth"))?];
            let r = (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt() / 2.0;
            Some(Aabb::from_points(c, c).inflate(r))
        }
        _ => None,
    }
}

/// Half of the larger cross-section dimension of a catalog steel profile (`EN:HEA200`).
fn profile_half(id: &str) -> Option<f64> {
    for cat in [
        "profiles-en",
        "profiles-en-extended",
        "profiles-aisc",
        "profiles-aisc-extended",
    ] {
        let doc = pmef_catalogs::document(cat)?;
        if let Some(e) = doc["entries"]
            .as_array()?
            .iter()
            .find(|e| e["catalogId"] == id)
        {
            let g = |k: &str| e[k].as_f64().unwrap_or(0.0);
            let m = g("h_mm")
                .max(g("b_mm"))
                .max(g("d_mm"))
                .max(g("bf_mm"))
                .max(g("D_mm"));
            if m > 0.0 {
                return Some(m / 2.0);
            }
        }
    }
    None
}

/// Parse the triangles of a Wavefront OBJ (`v` and `f` records; polygons are fanned).
pub fn parse_obj(text: &str) -> Result<Vec<Tri>, (usize, String)> {
    let mut v: Vec<P3> = Vec::new();
    let mut tris = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let c: Vec<f64> = it.take(3).filter_map(|s| s.parse().ok()).collect();
                if c.len() != 3 {
                    return Err((i + 1, "vertex needs 3 coordinates".into()));
                }
                v.push([c[0], c[1], c[2]]);
            }
            Some("f") => {
                let mut idx = Vec::new();
                for s in it {
                    let n: i64 = s
                        .split('/')
                        .next()
                        .unwrap_or("")
                        .parse()
                        .map_err(|_| (i + 1, format!("bad index {s}")))?;
                    let k = if n < 0 { v.len() as i64 + n } else { n - 1 };
                    let p = *v
                        .get(k as usize)
                        .ok_or((i + 1, format!("index {n} out of range")))?;
                    idx.push(p);
                }
                for w in 1..idx.len().saturating_sub(1) {
                    tris.push([idx[0], idx[w], idx[w + 1]]);
                }
            }
            _ => {}
        }
    }
    Ok(tris)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_quad() {
        let t = parse_obj("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n").unwrap();
        assert_eq!(t.len(), 2);
    }
}
