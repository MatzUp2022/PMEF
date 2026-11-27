//! `mesh_ref` geometry references into glTF 2.0 files (spec 04 §2.3).
//!
//! A `GeometryReference` of type `mesh_ref` carries `ref = <path>[#<fragment>]`. The fragment
//! selects one glTF node:
//!
//! ```text
//! fragment = selector *( ";" selector )
//! selector = "/nodes/" index          ; JSON pointer into the glTF JSON (index may move on re-export)
//!          | "node=" value            ; node "name"
//!          | "extras." key "=" value  ; node "extras" member, e.g. extras.RefNo=16791/39427
//! ```
//!
//! Values are percent-decoded (`%3B` for `;`). Name and extras selectors are *stable* and win:
//! all of them must match the same single node. The index is used alone only when no stable
//! selector is given; if it disagrees with the stable selectors, the stable match is returned
//! together with a warning.

use serde_json::Value;

/// One fragment selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// `/nodes/<index>`
    Index(usize),
    /// `node=<name>`
    Name(String),
    /// `extras.<key>=<value>`
    Extra(String, String),
}

/// A parsed `mesh_ref` reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshRef {
    /// File part (package-relative path or URI), percent-encoding kept.
    pub path: String,
    /// Fragment selectors; empty = the whole file.
    pub selectors: Vec<Selector>,
}

/// Result of resolving a reference against a glTF document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Index of the selected node (`None` when the reference has no fragment).
    pub node: Option<usize>,
    /// Non-fatal findings (e.g. a stale index).
    pub warnings: Vec<String>,
}

/// Parse `ref`. Errors name the offending selector.
pub fn parse(r: &str) -> Result<MeshRef, String> {
    let (path, frag) = match r.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (r, None),
    };
    if path.is_empty() {
        return Err("mesh_ref without a file part".into());
    }
    let mut selectors = Vec::new();
    for s in frag.into_iter().flat_map(|f| f.split(';')) {
        let sel = if let Some(i) = s.strip_prefix("/nodes/") {
            Selector::Index(i.parse().map_err(|_| format!("selector `{s}`: node index is not an integer"))?)
        } else if let Some(n) = s.strip_prefix("node=") {
            Selector::Name(decode(n)?)
        } else if let Some((k, v)) = s.strip_prefix("extras.").and_then(|kv| kv.split_once('=')) {
            if k.is_empty() {
                return Err(format!("selector `{s}`: empty extras key"));
            }
            Selector::Extra(decode(k)?, decode(v)?)
        } else {
            return Err(format!("selector `{s}`: expected /nodes/<i>, node=<name> or extras.<key>=<value>"));
        };
        selectors.push(sel);
    }
    Ok(MeshRef { path: path.to_owned(), selectors })
}

/// Resolve the selectors against the glTF JSON document.
pub fn resolve(gltf: &Value, r: &MeshRef) -> Result<Resolved, String> {
    let nodes = gltf.get("nodes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let index = r.selectors.iter().find_map(|s| match s { Selector::Index(i) => Some(*i), _ => None });
    let stable: Vec<&Selector> = r.selectors.iter().filter(|s| !matches!(s, Selector::Index(_))).collect();
    if r.selectors.is_empty() {
        return Ok(Resolved { node: None, warnings: vec![] });
    }
    if stable.is_empty() {
        let i = index.expect("non-empty, no stable selector");
        return if i < nodes.len() {
            Ok(Resolved { node: Some(i), warnings: vec![] })
        } else {
            Err(format!("/nodes/{i}: file has {} node(s)", nodes.len()))
        };
    }
    let hits: Vec<usize> = (0..nodes.len()).filter(|&i| stable.iter().all(|s| matches(&nodes[i], s))).collect();
    match hits.as_slice() {
        [] => Err(format!("no node matches {}", describe(&stable))),
        [h] => {
            let warnings = match index {
                Some(i) if i != *h => vec![format!("/nodes/{i} is stale: {} is node {h}", describe(&stable))],
                _ => vec![],
            };
            Ok(Resolved { node: Some(*h), warnings })
        }
        many => Err(format!("{} nodes match {} (ambiguous)", many.len(), describe(&stable))),
    }
}

/// JSON document of a `.gltf` (plain JSON) or `.glb` (binary container) file.
pub fn gltf_json(bytes: &[u8]) -> Result<Value, String> {
    if bytes.starts_with(b"glTF") {
        let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
        let len = u32_at(12).ok_or("GLB too short")?;
        if u32_at(16) != Some(0x4E4F_534A) {
            return Err("GLB: first chunk is not JSON".into());
        }
        let chunk = bytes.get(20..20 + len).ok_or("GLB: JSON chunk exceeds file")?;
        serde_json::from_slice(chunk).map_err(|e| format!("GLB JSON chunk: {e}"))
    } else {
        serde_json::from_slice(bytes).map_err(|e| format!("glTF JSON: {e}"))
    }
}

fn matches(node: &Value, s: &Selector) -> bool {
    match s {
        Selector::Name(n) => node.get("name").and_then(Value::as_str) == Some(n.as_str()),
        Selector::Extra(k, v) => match node.get("extras").and_then(|e| e.get(k)) {
            Some(Value::String(x)) => x == v,
            Some(x @ (Value::Number(_) | Value::Bool(_))) => x.to_string() == *v,
            _ => false,
        },
        Selector::Index(_) => true,
    }
}

fn describe(sel: &[&Selector]) -> String {
    sel.iter()
        .map(|s| match s {
            Selector::Name(n) => format!("node={n}"),
            Selector::Extra(k, v) => format!("extras.{k}={v}"),
            Selector::Index(i) => format!("/nodes/{i}"),
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn decode(s: &str) -> Result<String, String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3).ok_or_else(|| format!("bad percent escape in `{s}`"))?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| format!("bad percent escape in `{s}`"))?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| format!("`{s}` is not UTF-8 after decoding"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> Value {
        json!({"nodes": [
            {"name": "SITE"},
            {"name": "/HA-PIPE/V-101", "extras": {"RefNo": "16791/39427"}},
            {"name": "dup"}, {"name": "dup"}
        ]})
    }

    #[test]
    fn parses_all_selector_forms() {
        let r = parse("HA-PIPE.glb#/nodes/1;node=%2FHA-PIPE%2FV-101;extras.RefNo=16791/39427").unwrap();
        assert_eq!(r.path, "HA-PIPE.glb");
        assert_eq!(r.selectors, vec![
            Selector::Index(1),
            Selector::Name("/HA-PIPE/V-101".into()),
            Selector::Extra("RefNo".into(), "16791/39427".into()),
        ]);
        assert!(parse("#/nodes/1").is_err());
        assert!(parse("a.glb#/nodes/x").is_err());
        assert!(parse("a.glb#foo").is_err());
    }

    #[test]
    fn stable_selectors_win_over_a_stale_index() {
        let r = resolve(&doc(), &parse("a.glb#/nodes/0;extras.RefNo=16791/39427").unwrap()).unwrap();
        assert_eq!(r.node, Some(1));
        assert_eq!(r.warnings.len(), 1);
        let r = resolve(&doc(), &parse("a.glb#/nodes/1;extras.RefNo=16791/39427").unwrap()).unwrap();
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn unresolved_ambiguous_and_out_of_range() {
        assert!(resolve(&doc(), &parse("a.glb#node=nope").unwrap()).is_err());
        assert!(resolve(&doc(), &parse("a.glb#node=dup").unwrap()).unwrap_err().contains("ambiguous"));
        assert!(resolve(&doc(), &parse("a.glb#/nodes/9").unwrap()).is_err());
        assert_eq!(resolve(&doc(), &parse("a.glb#/nodes/3").unwrap()).unwrap().node, Some(3));
        assert_eq!(resolve(&doc(), &parse("a.glb").unwrap()).unwrap().node, None);
    }

    #[test]
    fn reads_glb_json_chunk() {
        let json = br#"{"nodes":[{"name":"x"}]}  "#; // padded to 4 bytes
        let mut glb = b"glTF".to_vec();
        glb.extend(2u32.to_le_bytes());
        glb.extend(((20 + json.len()) as u32).to_le_bytes());
        glb.extend((json.len() as u32).to_le_bytes());
        glb.extend(0x4E4F_534Au32.to_le_bytes());
        glb.extend_from_slice(json);
        assert_eq!(gltf_json(&glb).unwrap()["nodes"][0]["name"], "x");
        assert_eq!(gltf_json(br#"{"nodes":[]}"#).unwrap()["nodes"], json!([]));
    }
}
