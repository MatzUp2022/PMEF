//! Deterministic object-level comparison of two PMEF packages (spec 03 §9).
//!
//! Objects are matched by `@id`; differences are reported per field with stable paths.
//! The result does not depend on the order of objects in either file, on JSON key order, or on
//! the order of keyed array elements (ports, nozzles, residuals, composite parts), so the same
//! two models always produce byte-identical output.

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Array elements carrying one of these fields are matched by its value, not by position.
pub const KEY_FIELDS: [&str; 6] = ["@id", "portId", "nozzleId", "parameter", "partName", "connectionId"];

/// Comparison options.
#[derive(Debug, Clone, Default)]
pub struct DiffOptions {
    /// Compare numbers exactly instead of with the spec 06 §6.2 tolerances.
    pub exact: bool,
    /// Field paths (or path prefixes) to ignore, e.g. `revision`, `provenance.createdAt`.
    pub ignore: Vec<String>,
}

/// Status of one object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ObjectStatus {
    /// Only in head.
    Added,
    /// Only in base.
    Removed,
    /// In both, with differences.
    Changed,
}

/// Kind of a field difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldChangeKind {
    /// Field only in head.
    Added,
    /// Field only in base.
    Removed,
    /// Field in both with different values.
    Modified,
}

/// One field difference.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldChange {
    /// Path, e.g. `ports[portId=P2].coordinate[0]`.
    pub path: String,
    /// Kind of change.
    pub kind: FieldChangeKind,
    /// Base value (absent for `added`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Value>,
    /// Head value (absent for `removed`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<Value>,
    /// `new − old` for numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

/// Differences of one object.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObjectDiff {
    /// Object id.
    #[serde(rename = "@id")]
    pub id: String,
    /// `@type` (head type for added/changed, base type for removed).
    #[serde(rename = "@type")]
    pub entity_type: String,
    /// Status.
    pub status: ObjectStatus,
    /// `@id` of the matched head object, if it differs (matching mode, spec 03 §9.5).
    #[serde(rename = "headId", skip_serializing_if = "Option::is_none")]
    pub head_id: Option<String>,
    /// How the head object was matched, if its `@id` differs.
    #[serde(rename = "matchedBy", skip_serializing_if = "Option::is_none")]
    pub matched_by: Option<crate::matching::MatchMethod>,
    /// Field changes (only for `changed`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<FieldChange>,
}

/// Counts per status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DiffSummary {
    /// Objects only in head.
    pub added: usize,
    /// Objects only in base.
    pub removed: usize,
    /// Objects in both with differences.
    pub changed: usize,
    /// Objects in both without differences.
    pub unchanged: usize,
}

/// Result of a comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiffReport {
    /// Counts.
    pub summary: DiffSummary,
    /// Added, removed and changed objects, sorted by `@id`.
    pub objects: Vec<ObjectDiff>,
    /// Correspondences between differing `@id`s (matching mode only), sorted by base `@id`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<crate::matching::ObjectMatch>,
}

impl DiffReport {
    /// True if both packages are equal under the options used.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

/// Invalid input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiffError {
    /// An object has no string `@id`.
    #[error("{side}: object #{index} has no @id")]
    MissingId {
        /// `base` or `head`.
        side: &'static str,
        /// 1-based object index.
        index: usize,
    },
    /// The same `@id` occurs twice in one package.
    #[error("{side}: duplicate @id {id}")]
    DuplicateId {
        /// `base` or `head`.
        side: &'static str,
        /// Duplicated id.
        id: String,
    },
}

/// Numeric tolerance for a field name (spec 06 §6.2).
pub fn tolerance(field: &str) -> f64 {
    let f = field.to_ascii_lowercase();
    if f.contains("pressure") {
        1.0
    } else if f.contains("temperature") {
        0.01
    } else if f.contains("flow") {
        0.001
    } else if f.contains("efficiency") {
        0.0001
    } else if f.contains("angle") || f.contains("direction") || f.contains("axis") {
        0.001
    } else {
        0.01
    }
}

pub(crate) fn index<'a>(objs: &'a [Value], side: &'static str) -> Result<BTreeMap<&'a str, &'a Value>, DiffError> {
    let mut map = BTreeMap::new();
    for (i, o) in objs.iter().enumerate() {
        let id = o.get("@id").and_then(Value::as_str).ok_or(DiffError::MissingId { side, index: i + 1 })?;
        if map.insert(id, o).is_some() {
            return Err(DiffError::DuplicateId { side, id: id.to_owned() });
        }
    }
    Ok(map)
}

fn type_of(v: &Value) -> String {
    v.get("@type").and_then(Value::as_str).unwrap_or("").to_owned()
}

/// Compare two packages (objects in any order).
pub fn diff(base: &[Value], head: &[Value], opts: &DiffOptions) -> Result<DiffReport, DiffError> {
    let b = index(base, "base")?;
    let h = index(head, "head")?;
    let ids: BTreeSet<&str> = b.keys().chain(h.keys()).copied().collect();
    let mut summary = DiffSummary::default();
    let mut objects = Vec::new();
    for id in ids {
        match (b.get(id), h.get(id)) {
            (None, Some(o)) => {
                summary.added += 1;
                objects.push(ObjectDiff { id: id.into(), entity_type: type_of(o), status: ObjectStatus::Added, head_id: None, matched_by: None, changes: vec![] });
            }
            (Some(o), None) => {
                summary.removed += 1;
                objects.push(ObjectDiff { id: id.into(), entity_type: type_of(o), status: ObjectStatus::Removed, head_id: None, matched_by: None, changes: vec![] });
            }
            (Some(old), Some(new)) => {
                let mut changes = Vec::new();
                compare(old, new, "", "", opts, &mut changes);
                if changes.is_empty() {
                    summary.unchanged += 1;
                } else {
                    summary.changed += 1;
                    objects.push(ObjectDiff { id: id.into(), entity_type: type_of(new), status: ObjectStatus::Changed, head_id: None, matched_by: None, changes });
                }
            }
            (None, None) => unreachable!(),
        }
    }
    Ok(DiffReport { summary, objects, matches: vec![] })
}

/// Compare two packages whose objects may carry different `@id`s (spec 03 §9.5).
///
/// Objects are first matched with [`crate::matching::match_objects`]; references in the head
/// package are rewritten to the matched base ids, then the packages are compared as usual.
pub fn diff_matched(
    base: &[Value],
    head: &[Value],
    opts: &DiffOptions,
    mopts: &crate::matching::MatchOptions,
) -> Result<DiffReport, DiffError> {
    use crate::matching::{match_objects, rewrite_ids, MatchMethod};
    let matches = match_objects(base, head, mopts)?;
    let h2b: BTreeMap<&str, &str> =
        matches.iter().filter(|m| m.base_id != m.head_id).map(|m| (m.head_id.as_str(), m.base_id.as_str())).collect();
    let rewritten: Vec<Value> = head.iter().map(|o| rewrite_ids(o, &h2b)).collect();
    let mut report = diff(base, &rewritten, opts)?;
    let by_base: BTreeMap<&str, &crate::matching::ObjectMatch> =
        matches.iter().filter(|m| m.method != MatchMethod::Id).map(|m| (m.base_id.as_str(), m)).collect();
    for o in &mut report.objects {
        if let Some(m) = by_base.get(o.id.as_str()) {
            o.head_id = Some(m.head_id.clone());
            o.matched_by = Some(m.method);
        }
    }
    report.matches = matches.into_iter().filter(|m| m.method != MatchMethod::Id).collect();
    Ok(report)
}

fn ignored(path: &str, opts: &DiffOptions) -> bool {
    opts.ignore.iter().any(|p| {
        path == p || path.strip_prefix(p.as_str()).is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
    })
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() { key.to_owned() } else { format!("{path}.{key}") }
}

fn push(out: &mut Vec<FieldChange>, path: String, kind: FieldChangeKind, old: Option<&Value>, new: Option<&Value>) {
    let delta = match (old.and_then(Value::as_f64), new.and_then(Value::as_f64)) {
        (Some(a), Some(b)) if kind == FieldChangeKind::Modified => Some(((b - a) * 1e6).round() / 1e6),
        _ => None,
    };
    out.push(FieldChange { path, kind, old: old.map(canonical), new: new.map(canonical), delta });
}

/// Copy of `v` with object keys sorted (spec 03 §2.5), so output never depends on input key order.
pub fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let sorted: BTreeMap<&String, Value> = m.iter().map(|(k, x)| (k, canonical(x))).collect();
            Value::Object(sorted.into_iter().map(|(k, x)| (k.clone(), x)).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// Key field shared by all elements of both arrays, if any.
fn array_key(a: &[Value], b: &[Value]) -> Option<&'static str> {
    KEY_FIELDS.into_iter().find(|k| {
        let ok = |v: &Value| v.get(*k).is_some_and(|x| x.is_string() || x.is_number());
        let unique = |arr: &[Value]| {
            let keys: BTreeSet<String> = arr.iter().filter_map(|v| v.get(*k)).map(key_text).collect();
            keys.len() == arr.len()
        };
        !(a.is_empty() && b.is_empty()) && a.iter().chain(b).all(ok) && unique(a) && unique(b)
    })
}

fn key_text(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

fn compare(old: &Value, new: &Value, path: &str, field: &str, opts: &DiffOptions, out: &mut Vec<FieldChange>) {
    if !path.is_empty() && ignored(path, opts) {
        return;
    }
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => compare_objects(a, b, path, opts, out),
        (Value::Array(a), Value::Array(b)) => {
            if let Some(k) = array_key(a, b) {
                let am: BTreeMap<String, &Value> = a.iter().map(|v| (key_text(&v[k]), v)).collect();
                let bm: BTreeMap<String, &Value> = b.iter().map(|v| (key_text(&v[k]), v)).collect();
                let keys: BTreeSet<&String> = am.keys().chain(bm.keys()).collect();
                for key in keys {
                    let p = format!("{path}[{k}={key}]");
                    match (am.get(key), bm.get(key)) {
                        (Some(x), Some(y)) => compare(x, y, &p, field, opts, out),
                        (Some(x), None) => push(out, p, FieldChangeKind::Removed, Some(x), None),
                        (None, Some(y)) => push(out, p, FieldChangeKind::Added, None, Some(y)),
                        (None, None) => unreachable!(),
                    }
                }
            } else {
                for i in 0..a.len().max(b.len()) {
                    let p = format!("{path}[{i}]");
                    match (a.get(i), b.get(i)) {
                        (Some(x), Some(y)) => compare(x, y, &p, field, opts, out),
                        (Some(x), None) => push(out, p, FieldChangeKind::Removed, Some(x), None),
                        (None, Some(y)) => push(out, p, FieldChangeKind::Added, None, Some(y)),
                        (None, None) => unreachable!(),
                    }
                }
            }
        }
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap_or(f64::NAN), y.as_f64().unwrap_or(f64::NAN));
            let equal = if opts.exact { x == y } else { (x - y).abs() <= tolerance(field) };
            if !equal {
                push(out, path.to_owned(), FieldChangeKind::Modified, Some(old), Some(new));
            }
        }
        _ if old == new => {}
        _ => push(out, path.to_owned(), FieldChangeKind::Modified, Some(old), Some(new)),
    }
}

fn compare_objects(a: &Map<String, Value>, b: &Map<String, Value>, path: &str, opts: &DiffOptions, out: &mut Vec<FieldChange>) {
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for k in keys {
        let p = join(path, k);
        if ignored(&p, opts) {
            continue;
        }
        match (a.get(k), b.get(k)) {
            (Some(x), Some(y)) => compare(x, y, &p, k, opts, out),
            (Some(x), None) => push(out, p, FieldChangeKind::Removed, Some(x), None),
            (None, Some(y)) => push(out, p, FieldChangeKind::Added, None, Some(y)),
            (None, None) => unreachable!(),
        }
    }
}

/// Human-readable, deterministic text rendering.
pub fn render_text(r: &DiffReport) -> String {
    let mut s = String::new();
    for o in &r.objects {
        let sign = match o.status {
            ObjectStatus::Added => '+',
            ObjectStatus::Removed => '-',
            ObjectStatus::Changed => '~',
        };
        match (&o.head_id, o.matched_by) {
            (Some(h), Some(m)) => {
                let detail = r.matches.iter().find(|x| x.base_id == o.id).map_or_else(String::new, |x| {
                    let d = x.distance_mm.map_or_else(String::new, |d| format!(" {d} mm"));
                    format!(" [{}{d}{}]", serde_json::to_value(m).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default(),
                        if x.ambiguous { ", ambiguous" } else { "" })
                });
                let _ = writeln!(s, "{sign} {} ⇐ {h} ({}){detail}", o.id, o.entity_type);
            }
            _ => {
                let _ = writeln!(s, "{sign} {} ({})", o.id, o.entity_type);
            }
        }
        for c in &o.changes {
            let v = |x: &Option<Value>| x.as_ref().map_or_else(String::new, Value::to_string);
            let _ = match c.kind {
                FieldChangeKind::Added => writeln!(s, "    + {}: {}", c.path, v(&c.new)),
                FieldChangeKind::Removed => writeln!(s, "    - {}: {}", c.path, v(&c.old)),
                FieldChangeKind::Modified => match c.delta {
                    Some(d) => writeln!(s, "    ~ {}: {} → {} (Δ {:+})", c.path, v(&c.old), v(&c.new), d),
                    None => writeln!(s, "    ~ {}: {} → {}", c.path, v(&c.old), v(&c.new)),
                },
            };
        }
    }
    if !r.matches.is_empty() {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for x in &r.matches {
            let k = serde_json::to_value(x.method).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            *counts.entry(k).or_default() += 1;
        }
        let list: Vec<String> = counts.iter().map(|(k, n)| format!("{k} {n}")).collect();
        let amb = r.matches.iter().filter(|x| x.ambiguous).count();
        let _ = writeln!(s, "Matched with different @id: {} ({}), ambiguous {amb}", r.matches.len(), list.join(", "));
    }
    let m = &r.summary;
    let _ = writeln!(
        s,
        "Summary: {} added, {} removed, {} changed, {} unchanged",
        m.added, m.removed, m.changed, m.unchanged
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pipe(id: &str, x: f64, ports_rev: bool) -> Value {
        let mut ports = vec![
            json!({"portId": "P1", "coordinate": [0.0, 0.0, 0.0]}),
            json!({"portId": "P2", "coordinate": [x, 0.0, 0.0]}),
        ];
        if ports_rev {
            ports.reverse();
        }
        json!({"@type": "pmef:Pipe", "@id": id, "pipeLength": x, "ports": ports,
               "revision": {"revisionId": "r2026-01-01-001", "changeState": "WIP"}})
    }

    #[test]
    fn order_of_objects_and_keyed_arrays_does_not_matter() {
        let base = vec![pipe("urn:pmef:obj:p:A", 1000.0, false), pipe("urn:pmef:obj:p:B", 2000.0, false)];
        let head = vec![pipe("urn:pmef:obj:p:B", 2000.0, true), pipe("urn:pmef:obj:p:A", 1000.0, true)];
        let r = diff(&base, &head, &DiffOptions::default()).unwrap();
        assert!(r.is_empty());
        assert_eq!(r.summary.unchanged, 2);
    }

    #[test]
    fn field_level_changes_with_keyed_paths_and_tolerance() {
        let base = vec![pipe("urn:pmef:obj:p:A", 1000.0, false)];
        let head = vec![pipe("urn:pmef:obj:p:A", 1003.5, true)];
        let r = diff(&base, &head, &DiffOptions::default()).unwrap();
        let paths: Vec<&str> = r.objects[0].changes.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, ["pipeLength", "ports[portId=P2].coordinate[0]"]);
        assert_eq!(r.objects[0].changes[0].delta, Some(3.5));

        let within = vec![pipe("urn:pmef:obj:p:A", 1000.004, false)];
        assert!(diff(&base, &within, &DiffOptions::default()).unwrap().is_empty());
        assert!(!diff(&base, &within, &DiffOptions { exact: true, ..Default::default() }).unwrap().is_empty());
    }

    #[test]
    fn added_removed_ignored_and_duplicates() {
        let base = vec![pipe("urn:pmef:obj:p:A", 1.0, false), pipe("urn:pmef:obj:p:B", 1.0, false)];
        let mut changed = pipe("urn:pmef:obj:p:A", 1.0, false);
        changed["revision"]["changeState"] = json!("SHARED");
        let head = vec![changed, pipe("urn:pmef:obj:p:C", 1.0, false)];
        let r = diff(&base, &head, &DiffOptions::default()).unwrap();
        assert_eq!(r.summary, DiffSummary { added: 1, removed: 1, changed: 1, unchanged: 0 });
        let opts = DiffOptions { ignore: vec!["revision".into()], ..Default::default() };
        assert_eq!(diff(&base, &head, &opts).unwrap().summary.changed, 0);

        let dup = vec![pipe("urn:pmef:obj:p:A", 1.0, false), pipe("urn:pmef:obj:p:A", 2.0, false)];
        assert!(matches!(diff(&dup, &base, &DiffOptions::default()), Err(DiffError::DuplicateId { side: "base", .. })));
    }

    #[test]
    fn output_does_not_depend_on_json_key_order() {
        let base = vec![json!({"@id": "urn:pmef:obj:p:A", "@type": "pmef:Pipe"})];
        let h1 = vec![json!({"@id": "urn:pmef:obj:p:A", "@type": "pmef:Pipe", "review": {"status": "approved", "reviewedBy": "x"}})];
        let h2 = vec![json!({"review": {"reviewedBy": "x", "status": "approved"}, "@type": "pmef:Pipe", "@id": "urn:pmef:obj:p:A"})];
        let r1 = serde_json::to_string(&diff(&base, &h1, &DiffOptions::default()).unwrap()).unwrap();
        let r2 = serde_json::to_string(&diff(&base, &h2, &DiffOptions::default()).unwrap()).unwrap();
        assert_eq!(r1, r2);
        assert!(r1.contains(r#"{"reviewedBy":"x","status":"approved"}"#), "{r1}");
    }

    #[test]
    fn ignore_matches_path_prefix_only_at_segment_boundary() {
        let o = DiffOptions { ignore: vec!["rev".into()], ..Default::default() };
        assert!(!ignored("revision", &o));
        assert!(ignored("rev.x", &o));
        assert!(ignored("rev[0]", &o));
    }
}
