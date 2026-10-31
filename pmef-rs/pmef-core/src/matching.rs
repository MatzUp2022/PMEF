//! Correspondence of objects whose `@id`s differ (spec 03 §9.5).
//!
//! Typical case: an as-built model produced by a recogniser (own `@id`s, no tags) is compared
//! with the design model. Objects are matched in fixed passes, strongest evidence first; every
//! pass only considers objects that are still unmatched, and every result is deterministic.
//!
//! 1. `id`         — same `@id`
//! 2. `tag`        — same `@type` and same engineering identifier, unique on both sides
//! 3. `equivalent` — same native tool object (`pmef:HasEquivalentIn` targetSystem + targetSystemId)
//! 4. `geometry`   — same `@type`, mean distance of anchor points ≤ `max_distance_mm`
//! 5. `reference`  — geometry objects follow their owner, relationships their endpoints

use crate::diff::DiffError;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Matching options.
#[derive(Debug, Clone, Copy)]
pub struct MatchOptions {
    /// Largest mean anchor distance accepted for a geometric match [mm].
    pub max_distance_mm: f64,
    /// A competing candidate closer than this to the chosen one makes a match ambiguous [mm].
    pub ambiguity_margin_mm: f64,
}

impl Default for MatchOptions {
    fn default() -> Self {
        Self { max_distance_mm: 100.0, ambiguity_margin_mm: 10.0 }
    }
}

/// How a pair was matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchMethod {
    /// Same `@id`.
    Id,
    /// Same engineering identifier (tag, line number, mark …).
    Tag,
    /// Same native tool object via `pmef:HasEquivalentIn`.
    Equivalent,
    /// Geometric proximity of anchor points.
    Geometry,
    /// Derived from matched owner / relationship endpoints.
    Reference,
}

/// One base ↔ head correspondence.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMatch {
    /// `@id` in the base package.
    pub base_id: String,
    /// `@id` in the head package.
    pub head_id: String,
    /// Evidence used.
    pub method: MatchMethod,
    /// Mean anchor distance [mm] (geometry matches only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_mm: Option<f64>,
    /// A competing candidate was nearly as close (geometry matches only).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ambiguous: bool,
}

/// Engineering identifier fields, checked in this order.
const TAG_PATHS: [&[&str]; 8] = [
    &["equipmentBasic", "tagNumber"],
    &["lineNumber"],
    &["tagNumber"],
    &["memberMark"],
    &["supportsMark"],
    &["spoolMark"],
    &["weldSpec", "weldNumber"],
    &["loopNumber"],
];

fn get<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(v, |cur, k| cur.get(*k))
}

fn ty(v: &Value) -> &str {
    v.get("@type").and_then(Value::as_str).unwrap_or("")
}

fn id(v: &Value) -> &str {
    v.get("@id").and_then(Value::as_str).unwrap_or("")
}

fn tag(v: &Value) -> Option<String> {
    TAG_PATHS.iter().find_map(|p| get(v, p).and_then(Value::as_str)).map(|t| format!("{}|{t}", ty(v)))
}

fn point(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    (a.len() == 3).then(|| [a[0].as_f64(), a[1].as_f64(), a[2].as_f64()]).and_then(|[x, y, z]| Some([x?, y?, z?]))
}

/// Anchor points: port and nozzle coordinates, member end points, node coordinate.
pub fn anchors(v: &Value) -> Vec<[f64; 3]> {
    let mut pts = Vec::new();
    for list in ["ports", "nozzles"] {
        for e in v.get(list).and_then(Value::as_array).into_iter().flatten() {
            pts.extend(e.get("coordinate").and_then(point));
        }
    }
    for k in ["startPoint", "endPoint", "coordinate"] {
        pts.extend(v.get(k).and_then(point));
    }
    pts
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Symmetric mean nearest-neighbour distance of two anchor sets (order- and direction-free).
pub fn anchor_distance(a: &[[f64; 3]], b: &[[f64; 3]]) -> Option<f64> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let one_way = |x: &[[f64; 3]], y: &[[f64; 3]]| {
        x.iter().map(|p| y.iter().map(|q| dist(*p, *q)).fold(f64::INFINITY, f64::min)).sum::<f64>() / x.len() as f64
    };
    Some(((one_way(a, b) + one_way(b, a)) / 2.0 * 1e6).round() / 1e6)
}

struct State<'a> {
    base: BTreeMap<&'a str, &'a Value>,
    head: BTreeMap<&'a str, &'a Value>,
    b2h: BTreeMap<String, String>,
    h2b: BTreeMap<String, String>,
    out: Vec<ObjectMatch>,
}

impl State<'_> {
    fn free_base(&self) -> Vec<&str> {
        self.base.keys().copied().filter(|k| !self.b2h.contains_key(*k)).collect()
    }
    fn free_head(&self) -> Vec<&str> {
        self.head.keys().copied().filter(|k| !self.h2b.contains_key(*k)).collect()
    }
    fn add(&mut self, b: &str, h: &str, method: MatchMethod, distance_mm: Option<f64>, ambiguous: bool) {
        self.b2h.insert(b.to_owned(), h.to_owned());
        self.h2b.insert(h.to_owned(), b.to_owned());
        self.out.push(ObjectMatch { base_id: b.into(), head_id: h.into(), method, distance_mm, ambiguous });
    }
}

/// Unique key → id, dropping keys that occur more than once.
fn unique<'a>(ids: &[&'a str], objs: &BTreeMap<&'a str, &'a Value>, key: impl Fn(&Value) -> Option<String>) -> BTreeMap<String, &'a str> {
    let mut seen: BTreeMap<String, Option<&str>> = BTreeMap::new();
    for i in ids {
        if let Some(k) = key(objs[i]) {
            seen.entry(k).and_modify(|e| *e = None).or_insert(Some(*i));
        }
    }
    seen.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).collect()
}

fn equivalents<'a>(objs: &BTreeMap<&'a str, &'a Value>) -> BTreeMap<&'a str, String> {
    let mut m = BTreeMap::new();
    for o in objs.values().filter(|o| ty(o) == "pmef:HasEquivalentIn") {
        if let (Some(src), Some(sys), Some(nid)) = (
            o.get("sourceId").and_then(Value::as_str),
            o.get("targetSystem").and_then(Value::as_str),
            o.get("targetSystemId").and_then(Value::as_str),
        ) {
            m.insert(src, format!("{sys}|{nid}"));
        }
    }
    m
}

/// Compute base ↔ head correspondences.
pub fn match_objects(base: &[Value], head: &[Value], opts: &MatchOptions) -> Result<Vec<ObjectMatch>, DiffError> {
    let mut st = State { base: crate::diff::index(base, "base")?, head: crate::diff::index(head, "head")?, b2h: BTreeMap::new(), h2b: BTreeMap::new(), out: vec![] };

    // 1. id
    let common: Vec<String> = st.base.keys().filter(|k| st.head.contains_key(*k)).map(|k| (*k).to_owned()).collect();
    for k in common {
        st.add(&k, &k, MatchMethod::Id, None, false);
    }
    // 2. tag
    let (fb, fh) = (st.free_base(), st.free_head());
    let (tb, th) = (unique(&fb, &st.base, tag), unique(&fh, &st.head, tag));
    let pairs: Vec<(String, String)> = tb.iter().filter_map(|(k, b)| th.get(k).map(|h| ((*b).to_owned(), (*h).to_owned()))).collect();
    for (b, h) in pairs {
        st.add(&b, &h, MatchMethod::Tag, None, false);
    }
    // 3. equivalent
    let (eb, eh) = (equivalents(&st.base), equivalents(&st.head));
    let (fb, fh) = (st.free_base(), st.free_head());
    let tb = unique(&fb, &st.base, |o| eb.get(id(o)).map(|e| format!("{}|{e}", ty(o))));
    let th = unique(&fh, &st.head, |o| eh.get(id(o)).map(|e| format!("{}|{e}", ty(o))));
    let pairs: Vec<(String, String)> = tb.iter().filter_map(|(k, b)| th.get(k).map(|h| ((*b).to_owned(), (*h).to_owned()))).collect();
    for (b, h) in pairs {
        st.add(&b, &h, MatchMethod::Equivalent, None, false);
    }
    // 4. geometry — candidate pairs ordered by (distance, base id, head id), assigned greedily 1:1
    let (fb, fh) = (st.free_base(), st.free_head());
    let mut cands: Vec<(f64, String, String)> = Vec::new();
    for b in &fb {
        let ab = anchors(st.base[b]);
        for h in &fh {
            if ty(st.base[b]) != ty(st.head[h]) {
                continue;
            }
            if let Some(d) = anchor_distance(&ab, &anchors(st.head[h])) {
                if d <= opts.max_distance_mm {
                    cands.push((d, (*b).to_owned(), (*h).to_owned()));
                }
            }
        }
    }
    cands.sort_by(|x, y| x.0.total_cmp(&y.0).then_with(|| x.1.cmp(&y.1)).then_with(|| x.2.cmp(&y.2)));
    let all = cands.clone();
    for (d, b, h) in cands {
        if st.b2h.contains_key(&b) || st.h2b.contains_key(&h) {
            continue;
        }
        let ambiguous = all.iter().any(|(d2, b2, h2)| (*b2 == b) != (*h2 == h) && *d2 - d < opts.ambiguity_margin_mm);
        st.add(&b, &h, MatchMethod::Geometry, Some(d), ambiguous);
    }
    // 5. reference — repeat until stable (owners and relationship endpoints may chain)
    loop {
        let (fb, fh) = (st.free_base(), st.free_head());
        let refkey = |o: &Value, map: Option<&BTreeMap<String, String>>| -> Option<String> {
            let tr = |s: &str| map.map_or(Some(s.to_owned()), |m| m.get(s).cloned());
            if let Some(owner) = o.get("ownerRef").and_then(Value::as_str) {
                return Some(format!("{}|{}", ty(o), tr(owner)?));
            }
            let (s, t) = (o.get("sourceId")?.as_str()?, o.get("targetId")?.as_str()?);
            Some(format!("{}|{}|{}", ty(o), tr(s)?, tr(t)?))
        };
        let h2b = st.h2b.clone();
        let tb = unique(&fb, &st.base, |o| refkey(o, None));
        let th = unique(&fh, &st.head, |o| refkey(o, Some(&h2b)));
        let pairs: Vec<(String, String)> = tb.iter().filter_map(|(k, b)| th.get(k).map(|h| ((*b).to_owned(), (*h).to_owned()))).collect();
        if pairs.is_empty() {
            break;
        }
        for (b, h) in pairs {
            st.add(&b, &h, MatchMethod::Reference, None, false);
        }
    }
    st.out.sort_by(|a, b| a.base_id.cmp(&b.base_id));
    Ok(st.out)
}

/// Rewrite every string in `v` that equals a mapped head id to its base id.
pub fn rewrite_ids(v: &Value, h2b: &BTreeMap<&str, &str>) -> Value {
    match v {
        Value::String(s) => h2b.get(s.as_str()).map_or_else(|| v.clone(), |b| Value::String((*b).to_owned())),
        Value::Array(a) => Value::Array(a.iter().map(|x| rewrite_ids(x, h2b)).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), rewrite_ids(x, h2b))).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pipe(id: &str, x0: f64, x1: f64) -> Value {
        json!({"@type": "pmef:Pipe", "@id": id, "ports": [
            {"portId": "P1", "coordinate": [x0, 0.0, 0.0]}, {"portId": "P2", "coordinate": [x1, 0.0, 0.0]}]})
    }

    #[test]
    fn anchor_distance_ignores_direction() {
        let a = anchors(&pipe("a", 0.0, 1000.0));
        let b = anchors(&pipe("b", 1006.0, 6.0));
        assert_eq!(anchor_distance(&a, &b), Some(6.0));
    }

    #[test]
    fn passes_in_order_and_one_to_one() {
        let base = vec![
            json!({"@type": "pmef:Pump", "@id": "urn:pmef:obj:d:P-1", "equipmentBasic": {"tagNumber": "P-1"}}),
            pipe("urn:pmef:obj:d:PIPE-1", 0.0, 1000.0),
            pipe("urn:pmef:obj:d:PIPE-2", 0.0, 5000.0),
            json!({"@type": "pmef:ParametricGeometry", "@id": "urn:pmef:geom:d:G1", "ownerRef": "urn:pmef:obj:d:PIPE-1"}),
        ];
        let head = vec![
            json!({"@type": "pmef:Pump", "@id": "urn:pmef:obj:s:x1", "equipmentBasic": {"tagNumber": "P-1"}}),
            pipe("urn:pmef:obj:s:x2", 1004.0, 2.0),
            pipe("urn:pmef:obj:s:x3", 3000.0, 9000.0),
            json!({"@type": "pmef:ParametricGeometry", "@id": "urn:pmef:geom:s:g", "ownerRef": "urn:pmef:obj:s:x2"}),
        ];
        let m = match_objects(&base, &head, &MatchOptions::default()).unwrap();
        let got: Vec<(&str, &str, MatchMethod)> = m.iter().map(|m| (m.base_id.as_str(), m.head_id.as_str(), m.method)).collect();
        assert_eq!(got, [
            ("urn:pmef:geom:d:G1", "urn:pmef:geom:s:g", MatchMethod::Reference),
            ("urn:pmef:obj:d:P-1", "urn:pmef:obj:s:x1", MatchMethod::Tag),
            ("urn:pmef:obj:d:PIPE-1", "urn:pmef:obj:s:x2", MatchMethod::Geometry),
        ]);
        assert_eq!(m[2].distance_mm, Some(3.0));
    }

    #[test]
    fn near_tie_is_ambiguous_and_duplicate_tags_are_not_used() {
        let base = vec![pipe("urn:pmef:obj:d:A", 0.0, 1000.0)];
        let head = vec![pipe("urn:pmef:obj:s:1", 0.0, 1002.0), pipe("urn:pmef:obj:s:2", 0.0, 1004.0)];
        let m = match_objects(&base, &head, &MatchOptions::default()).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].head_id, "urn:pmef:obj:s:1");
        assert!(m[0].ambiguous);

        let tagged = |id: &str| json!({"@type": "pmef:Valve", "@id": id, "tagNumber": "XV-1"});
        let m = match_objects(&[tagged("urn:pmef:obj:d:V")], &[tagged("urn:pmef:obj:s:1"), tagged("urn:pmef:obj:s:2")], &MatchOptions::default()).unwrap();
        assert!(m.is_empty(), "a tag occurring twice must not be used for matching");
    }
}
