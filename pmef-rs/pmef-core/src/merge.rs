//! Deterministic three-way merge of PMEF packages (spec 03 §10).
//!
//! `base` is the common ancestor, `ours` and `theirs` are two independent edits of it.
//! Objects are matched by `@id`; values are merged recursively with the comparison rules of
//! spec 03 §9 (keyed arrays, numeric tolerances). A change on one side only is taken; equal
//! changes on both sides are taken once; different changes on both sides are a conflict.

use crate::diff::{array_key_n, canonical, index, key_text, tolerance, DiffError};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Side that wins a conflict in the written output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Prefer {
    /// Keep `ours` (default).
    #[default]
    Ours,
    /// Keep `theirs`.
    Theirs,
}

/// Merge options.
#[derive(Debug, Clone, Default)]
pub struct MergeOptions {
    /// Compare numbers exactly instead of with the spec 06 §6.2 tolerances.
    pub exact: bool,
    /// Side written for conflicting values.
    pub prefer: Prefer,
}

/// Conflict kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    /// Both sides changed the value differently.
    ModifyModify,
    /// One side changed, the other deleted.
    ModifyDelete,
    /// Both sides added different values where base had none.
    AddAdd,
}

/// One conflict.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Conflict {
    /// Object id.
    #[serde(rename = "@id")]
    pub id: String,
    /// Field path within the object (spec 03 §9.3); empty for whole-object conflicts.
    pub path: String,
    /// Kind.
    pub kind: ConflictKind,
    /// Base value (absent if none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<Value>,
    /// Our value (absent if deleted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ours: Option<Value>,
    /// Their value (absent if deleted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theirs: Option<Value>,
}

/// Merge result.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeResult {
    /// Merged objects: FileHeader first, then sorted by `@id`, keys sorted.
    pub objects: Vec<Value>,
    /// Conflicts, sorted by `@id` and path.
    pub conflicts: Vec<Conflict>,
}

fn same(a: Option<&Value>, b: Option<&Value>, field: &str, opts: &MergeOptions) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => equal(x, y, field, opts),
        _ => false,
    }
}

fn equal(a: &Value, b: &Value, field: &str, opts: &MergeOptions) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap_or(f64::NAN), y.as_f64().unwrap_or(f64::NAN));
            if opts.exact { x == y } else { (x - y).abs() <= tolerance(field) }
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| equal(v, w, k, opts)))
        }
        (Value::Array(x), Value::Array(y)) => {
            if let Some(k) = array_key_n(&[x, y]) {
                let ym: BTreeMap<String, &Value> = y.iter().map(|v| (key_text(&v[k]), v)).collect();
                x.len() == y.len() && x.iter().all(|v| ym.get(&key_text(&v[k])).is_some_and(|w| equal(v, w, field, opts)))
            } else {
                x.len() == y.len() && x.iter().zip(y).all(|(v, w)| equal(v, w, field, opts))
            }
        }
        _ => a == b,
    }
}

struct Ctx<'a> {
    id: &'a str,
    opts: &'a MergeOptions,
    conflicts: &'a mut Vec<Conflict>,
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() { key.to_owned() } else { format!("{path}.{key}") }
}

/// Three-way merge of optional values; `None` = absent / deleted.
fn merge3(b: Option<&Value>, o: Option<&Value>, t: Option<&Value>, path: &str, field: &str, cx: &mut Ctx) -> Option<Value> {
    let opts = cx.opts;
    if same(o, t, field, opts) {
        return o.cloned();
    }
    if same(b, o, field, opts) {
        return t.cloned();
    }
    if same(b, t, field, opts) {
        return o.cloned();
    }
    // both sides changed differently: descend where structure allows
    match (b, o, t) {
        (bv, Some(Value::Object(om)), Some(Value::Object(tm))) if bv.is_none() || bv.is_some_and(Value::is_object) => {
            let empty = Map::new();
            let bm = bv.and_then(Value::as_object).unwrap_or(&empty);
            let keys: BTreeSet<&String> = bm.keys().chain(om.keys()).chain(tm.keys()).collect();
            let mut out = Map::new();
            for k in keys {
                if let Some(v) = merge3(bm.get(k), om.get(k), tm.get(k), &join(path, k), k, cx) {
                    out.insert(k.clone(), v);
                }
            }
            return Some(Value::Object(out));
        }
        (Some(Value::Array(ba)), Some(Value::Array(oa)), Some(Value::Array(ta))) => {
            if let Some(k) = array_key_n(&[ba, oa, ta]) {
                let m = |a: &Vec<Value>| -> BTreeMap<String, Value> { a.iter().map(|v| (key_text(&v[k]), v.clone())).collect() };
                let (bm, om, tm) = (m(ba), m(oa), m(ta));
                // order: ours' order, then keys new in theirs (sorted)
                let mut order: Vec<String> = oa.iter().map(|v| key_text(&v[k])).collect();
                let extra: BTreeSet<&String> = tm.keys().filter(|x| !om.contains_key(*x)).collect();
                order.extend(extra.into_iter().cloned());
                let mut out = Vec::new();
                for key in order {
                    let p = format!("{path}[{k}={key}]");
                    if let Some(v) = merge3(bm.get(&key), om.get(&key), tm.get(&key), &p, field, cx) {
                        out.push(v);
                    }
                }
                return Some(Value::Array(out));
            }
            if ba.len() == oa.len() && oa.len() == ta.len() {
                let out = (0..ba.len())
                    .filter_map(|i| merge3(Some(&ba[i]), Some(&oa[i]), Some(&ta[i]), &format!("{path}[{i}]"), field, cx))
                    .collect();
                return Some(Value::Array(out));
            }
        }
        _ => {}
    }
    let kind = match (b, o, t) {
        (None, _, _) => ConflictKind::AddAdd,
        (Some(_), None, _) | (Some(_), _, None) => ConflictKind::ModifyDelete,
        _ => ConflictKind::ModifyModify,
    };
    cx.conflicts.push(Conflict {
        id: cx.id.to_owned(),
        path: path.to_owned(),
        kind,
        base: b.map(canonical),
        ours: o.map(canonical),
        theirs: t.map(canonical),
    });
    match opts.prefer {
        Prefer::Ours => o.cloned(),
        Prefer::Theirs => t.cloned(),
    }
}

/// Three-way merge of packages (objects in any order).
pub fn merge(base: &[Value], ours: &[Value], theirs: &[Value], opts: &MergeOptions) -> Result<MergeResult, DiffError> {
    let (b, o, t) = (index(base, "base")?, index(ours, "ours")?, index(theirs, "theirs")?);
    let ids: BTreeSet<&str> = b.keys().chain(o.keys()).chain(t.keys()).copied().collect();
    let mut conflicts = Vec::new();
    let mut objects = Vec::new();
    for id in ids {
        let mut cx = Ctx { id, opts, conflicts: &mut conflicts };
        let merged = merge3(b.get(id).copied(), o.get(id).copied(), t.get(id).copied(), "", "", &mut cx);
        if let Some(v) = merged {
            objects.push(canonical(&v));
        }
    }
    let is_header = |v: &Value| v.get("@type").and_then(Value::as_str) == Some("pmef:FileHeader");
    objects.sort_by_key(|v| !is_header(v)); // stable: headers first, @id order otherwise
    conflicts.sort_by(|x, y| x.id.cmp(&y.id).then_with(|| x.path.cmp(&y.path)));
    Ok(MergeResult { objects, conflicts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pipe(len: f64, x: f64, note: &str) -> Value {
        json!({"@type": "pmef:Pipe", "@id": "urn:pmef:obj:p:A", "pipeLength": len, "note": note,
               "ports": [{"portId": "P1", "coordinate": [0.0, 0.0, 0.0]}, {"portId": "P2", "coordinate": [x, 0.0, 0.0]}]})
    }

    #[test]
    fn non_overlapping_changes_merge_cleanly() {
        let base = vec![pipe(1000.0, 1000.0, "n")];
        let ours = vec![pipe(1012.0, 1000.0, "n")];
        let mut t = pipe(1000.0, 1000.0, "n");
        t["ports"] = json!([{"portId": "P2", "coordinate": [1000.0, 0.0, 5.0]}, {"portId": "P1", "coordinate": [0.0, 0.0, 0.0]}]);
        let r = merge(&base, &ours, &[t], &MergeOptions::default()).unwrap();
        assert!(r.conflicts.is_empty(), "{:?}", r.conflicts);
        assert_eq!(r.objects[0]["pipeLength"], 1012.0);
        let p2 = r.objects[0]["ports"].as_array().unwrap().iter().find(|p| p["portId"] == "P2").unwrap();
        assert_eq!(p2["coordinate"], json!([1000.0, 0.0, 5.0]));
    }

    #[test]
    fn conflicting_changes_are_reported_and_prefer_applies() {
        let base = vec![pipe(1000.0, 1000.0, "n")];
        let ours = vec![pipe(1012.0, 1000.0, "n")];
        let theirs = vec![pipe(990.0, 1000.0, "n")];
        let r = merge(&base, &ours, &theirs, &MergeOptions::default()).unwrap();
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(r.conflicts[0].path, "pipeLength");
        assert_eq!(r.conflicts[0].kind, ConflictKind::ModifyModify);
        assert_eq!(r.objects[0]["pipeLength"], 1012.0);
        let r = merge(&base, &ours, &theirs, &MergeOptions { prefer: Prefer::Theirs, ..Default::default() }).unwrap();
        assert_eq!(r.objects[0]["pipeLength"], 990.0);
    }

    #[test]
    fn equal_changes_within_tolerance_do_not_conflict() {
        let r = merge(&[pipe(1000.0, 1000.0, "n")], &[pipe(1012.0, 1000.0, "n")], &[pipe(1012.004, 1000.0, "n")], &MergeOptions::default()).unwrap();
        assert!(r.conflicts.is_empty());
    }

    #[test]
    fn object_level_add_delete() {
        let base = vec![pipe(1000.0, 1000.0, "n")];
        // ours deletes, theirs changes → modify/delete
        let r = merge(&base, &[], &[pipe(1001.0, 1000.0, "n")], &MergeOptions::default()).unwrap();
        assert_eq!(r.conflicts[0].kind, ConflictKind::ModifyDelete);
        assert!(r.objects.is_empty(), "prefer ours keeps the deletion");
        // ours deletes, theirs unchanged → deleted cleanly
        let r = merge(&base, &[], &base, &MergeOptions::default()).unwrap();
        assert!(r.conflicts.is_empty() && r.objects.is_empty());
        // both add the same new object → taken once
        let new = json!({"@type": "pmef:Plant", "@id": "urn:pmef:plant:p:X", "name": "X"});
        let r = merge(&[], std::slice::from_ref(&new), std::slice::from_ref(&new), &MergeOptions::default()).unwrap();
        assert!(r.conflicts.is_empty() && r.objects.len() == 1);
    }

    #[test]
    fn output_is_deterministic_and_header_first() {
        let h = json!({"@type": "pmef:FileHeader", "@id": "urn:pmef:pkg:p:z"});
        let a = json!({"@type": "pmef:Plant", "@id": "urn:pmef:plant:p:a", "name": "A"});
        let r1 = merge(&[], &[a.clone(), h.clone()], &[], &MergeOptions::default()).unwrap();
        let r2 = merge(&[], &[h, a], &[], &MergeOptions::default()).unwrap();
        assert_eq!(r1, r2);
        assert_eq!(r1.objects[0]["@type"], "pmef:FileHeader");
    }
}
