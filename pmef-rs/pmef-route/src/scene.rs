//! Scene: obstacles and nozzles, from explicit boxes, OBJ meshes and PMEF packages.

use crate::geom::{Aabb, Mesh, Tri, P3};
use crate::problem::{ObstacleKind, RouteProblem};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

/// Shape of an obstacle.
#[derive(Debug, Clone)]
pub enum Shape {
    Box(Aabb),
    /// Shared, so a [`SceneCache`] hands the same parsed mesh to every run without copying it.
    Mesh(Arc<Mesh>),
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
    /// Structural steel that pipe supports may rest on (PMEF members, or `structural` in the
    /// problem). Equipment, existing piping and other hard obstacles are not.
    pub structural: bool,
}

/// Obstacles plus named nozzles (`<@id>#<mark>` → point, outward direction).
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub obstacles: Vec<Obstacle>,
    pub nozzles: BTreeMap<String, (P3, P3)>,
    /// Other names of objects with nozzles → their `@id`: `isDerivedFrom` (functional URI of
    /// the P&ID object) and the tag number (`tagNumber`, `equipmentBasic.tagNumber`). Used to match P&ID line lists to the 3D scene.
    pub aliases: BTreeMap<String, String>,
}

/// Scene loading errors.
#[derive(Debug, thiserror::Error)]
pub enum SceneError {
    #[error("{0}: {1}")]
    Io(String, String),
    #[error("{0}:{1}: {2}")]
    Parse(String, usize, String),
    #[error("obstacle {0}: give exactly one of aabb / obj")]
    Shape(String),
}

/// Reads a file named in the problem: name → content, or an error message.
pub type Reader<'a> = dyn Fn(&str) -> Result<String, String> + 'a;

/// Reader for files relative to `base`.
pub fn fs_reader(base: &Path) -> impl Fn(&str) -> Result<String, String> + '_ {
    move |f: &str| std::fs::read_to_string(base.join(f)).map_err(|e| e.to_string())
}

/// Parsed scene files, kept across runs: OBJ meshes (with their search tree) and PMEF NDJSON
/// packages, by file name as written in the problem. A viewer that re-routes interactively
/// parses a large building model once instead of on every run (see `pmef-route-wasm`
/// `RouteSession`). Which objects of a package become obstacles (`pmefIgnore`) is still
/// decided per run, so the cache only holds parsed files.
#[derive(Debug, Clone, Default)]
pub struct SceneCache {
    meshes: HashMap<String, Arc<Mesh>>,
    ndjson: HashMap<String, Arc<Vec<Value>>>,
}

impl SceneCache {
    /// Forget the parsed content of `file` (it changed or was removed).
    pub fn invalidate(&mut self, file: &str) {
        self.meshes.remove(file);
        self.ndjson.remove(file);
    }

    /// Whether `file` is held parsed.
    pub fn contains(&self, file: &str) -> bool {
        self.meshes.contains_key(file) || self.ndjson.contains_key(file)
    }

    /// The OBJ mesh `file`, parsed on first use.
    pub fn mesh(&mut self, file: &str, read: &Reader<'_>) -> Result<Arc<Mesh>, SceneError> {
        if let Some(m) = self.meshes.get(file) {
            return Ok(m.clone());
        }
        let text = read(file).map_err(|e| SceneError::Io(file.to_string(), e))?;
        let tris = parse_obj(&text).map_err(|(l, m)| SceneError::Parse(file.to_string(), l, m))?;
        let m = Arc::new(Mesh::new(tris));
        self.meshes.insert(file.to_string(), m.clone());
        Ok(m)
    }

    /// The PMEF NDJSON package `file`, parsed on first use.
    pub fn ndjson(&mut self, file: &str, read: &Reader<'_>) -> Result<Arc<Vec<Value>>, SceneError> {
        if let Some(v) = self.ndjson.get(file) {
            return Ok(v.clone());
        }
        let v = Arc::new(read_ndjson(file, read)?);
        self.ndjson.insert(file.to_string(), v.clone());
        Ok(v)
    }
}

/// Read a PMEF NDJSON file through `read`.
pub fn read_ndjson(f: &str, read: &Reader<'_>) -> Result<Vec<Value>, SceneError> {
    let text = read(f).map_err(|e| SceneError::Io(f.to_string(), e))?;
    let mut objs = Vec::new();
    for (i, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let v: Value = serde_json::from_str(line)
            .map_err(|e| SceneError::Parse(f.to_string(), i + 1, e.to_string()))?;
        objs.push(v);
    }
    Ok(objs)
}

impl Scene {
    pub fn push(&mut self, id: String, kind: ObstacleKind, penalty: f64, shape: Shape) {
        let bounds = match &shape {
            Shape::Box(b) => *b,
            Shape::Mesh(m) => m.bounds().unwrap_or(Aabb {
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
            structural: false,
        });
    }

    /// Build the scene of a problem; relative paths resolve against `base`.
    pub fn load(problem: &RouteProblem, base: &Path) -> Result<Scene, SceneError> {
        Scene::load_with(problem, &fs_reader(base))
    }

    /// Build the scene of a problem, reading the files it names through `read` (file name as
    /// written in the problem → content). Used where there is no file system (WASM).
    pub fn load_with(problem: &RouteProblem, read: &Reader<'_>) -> Result<Scene, SceneError> {
        Scene::load_cached(problem, read, &mut SceneCache::default())
    }

    /// As [`Scene::load_with`], taking parsed files from `cache` and adding those it lacks.
    pub fn load_cached(
        problem: &RouteProblem,
        read: &Reader<'_>,
        cache: &mut SceneCache,
    ) -> Result<Scene, SceneError> {
        let mut s = Scene::default();
        for f in &problem.pmef_scene {
            let objs = cache.ndjson(f, read)?;
            s.add_pmef(&objs, &problem.pmef_ignore);
        }
        for o in &problem.obstacles {
            let pen = o.penalty.unwrap_or(problem.settings.penetration_penalty);
            match (&o.aabb, &o.obj) {
                (Some(b), None) => s.push(o.id.clone(), o.kind, pen, Shape::Box(*b)),
                (None, Some(f)) => {
                    let m = cache.mesh(f, read)?;
                    s.push(o.id.clone(), o.kind, pen, Shape::Mesh(m));
                }
                _ => return Err(SceneError::Shape(o.id.clone())),
            }
            if let Some(last) = s.obstacles.last_mut() {
                last.structural = o.structural;
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
            if o["nozzles"].as_array().is_some_and(|a| !a.is_empty()) {
                for a in [
                    &o["isDerivedFrom"],
                    &o["tagNumber"],
                    &o["equipmentBasic"]["tagNumber"],
                ]
                .into_iter()
                .filter_map(Value::as_str)
                {
                    self.aliases.entry(a.to_string()).or_insert(id.to_string());
                }
            }
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
                // Steel member: centreline grown by the rolled section (pmef_geom::member_aabb);
                // unknown profiles get 250 mm.
                let s = o["profileId"].as_str().and_then(profile_section).unwrap_or(
                    pmef_geom::SectionSize {
                        half_h: 250.0,
                        half_b: 250.0,
                        round: true,
                    },
                );
                let v = |p: P3| pmef_geom::Vec3::new(p[0], p[1], p[2]);
                let m =
                    pmef_geom::member_aabb(v(a), v(b), s, o["rollAngle"].as_f64().unwrap_or(0.0));
                let bx = Aabb {
                    min: [m.min.x, m.min.y, m.min.z],
                    max: [m.max.x, m.max.y, m.max.z],
                };
                self.push(id.into(), ObstacleKind::Hard, 0.0, Shape::Box(bx));
                if let Some(last) = self.obstacles.last_mut() {
                    last.structural = true;
                }
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

/// Conservative box of a PMEF parametric primitive (spec 04); `None` for unsupported types
/// and `MESH_REF`. Composite primitives are the union of their children. Delegates to
/// [`pmef_geom::schema_primitive_aabb`], the single implementation for schema primitives.
pub fn primitive_box(p: &Value) -> Option<Aabb> {
    let b = pmef_geom::schema_primitive_aabb(p, &profile_section)?;
    Some(Aabb {
        min: [b.min.x, b.min.y, b.min.z],
        max: [b.max.x, b.max.y, b.max.z],
    })
}

/// Cross-section size of a catalog steel profile (`EN:HEA200`) for member bounds.
fn profile_section(id: &str) -> Option<pmef_geom::SectionSize> {
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
            let round = g("OD_mm").max(g("D_mm"));
            if round > 0.0 {
                return Some(pmef_geom::SectionSize {
                    half_h: round / 2.0,
                    half_b: round / 2.0,
                    round: true,
                });
            }
            let h = g("h_mm").max(g("d_mm")).max(g("b1_mm"));
            let h = if h > 0.0 { h } else { g("t_mm") }; // flat bar: b × t
            let b = g("b_mm").max(g("bf_mm")).max(g("b2_mm"));
            if h.max(b) > 0.0 {
                return Some(pmef_geom::SectionSize {
                    half_h: h / 2.0,
                    half_b: b / 2.0,
                    round: false,
                });
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

    #[test]
    fn sheared_snout_box_covers_the_tilted_face() {
        let b = primitive_box(
            &serde_json::json!({"type": "SNOUT", "center": [0.0, 0.0, 0.0],
            "axis": [0.0, 0.0, 1.0], "r1": 100.0, "r2": 100.0, "length": 300.0,
            "bottomShearX": 45.0}),
        )
        .unwrap();
        assert!(b.min[2] <= -100.0 + 1e-9, "{b:?}");
    }

    #[test]
    fn compound_schema_primitives_are_obstacles() {
        // via pmef-geom; the member size comes from the profile catalog
        let m = serde_json::json!({"type": "STEEL_PROFILE", "startPt": [0, 0, 0],
            "endPt": [6000, 0, 0], "profileId": "EN:HEB300"});
        assert_eq!(primitive_box(&m).unwrap().max[1], 150.0);
        // rolled 45°: flange corners at the half-diagonal (Codex review on #62)
        let mut r = m.clone();
        r["rollDeg"] = 45.0.into();
        assert!((primitive_box(&r).unwrap().max[1] - 300f64.hypot(300.0) / 2.0).abs() < 1e-9);
        let v = serde_json::json!({"type": "VALVE_BODY", "valveBodyType": "BALL", "center": [0, 0, 0],
            "direction": [1, 0, 0], "boreD": 50, "faceToFace": 200});
        assert!(primitive_box(&v).is_some());
        assert!(
            primitive_box(&serde_json::json!({"type": "MESH_REF", "meshUri": "a.glb"})).is_none()
        );
    }

    #[test]
    fn pyramid_and_rect_torus_boxes() {
        let pyr = serde_json::json!({"type": "PYRAMID", "center": [0, 0, 0], "axis": [0, 0, 1],
            "bottomX": 600, "bottomY": 800, "topX": 0, "topY": 0, "height": 300});
        let b = primitive_box(&pyr).unwrap();
        assert_eq!(
            (b.min[0], b.max[0], b.min[2], b.max[2]),
            (-500.0, 500.0, 0.0, 300.0)
        );
        let tor = serde_json::json!({"type": "RECT_TORUS", "center": [0, 0, 0], "axis": [0, 0, 1],
            "startDir": [1, 0, 0], "innerRadius": 300, "outerRadius": 400, "height": 600, "angleDeg": 90});
        let b = primitive_box(&tor).unwrap();
        assert_eq!(b.max[0], 500.0);
    }
}
