//! CAEX XML secondary serialisation (spec 03 §4, AutomationML 2.10 / IEC 62714, CAEX 3.0).
//!
//! [`write_caex`] turns PMEF records into a CAEX document, [`read_caex`] reads one back. The
//! round trip PMEF → CAEX → PMEF is lossless: every field of every record comes back with its
//! JSON type and in its order.
//!
//! Layout of the document:
//!
//! - `AdditionalInformation/PMEFHeader`: the `pmef:FileHeader` record.
//! - `InstanceHierarchy "PlantModel"`: one `InternalElement` per object, nested along `isPartOf`
//!   where the parent is in the same document (otherwise at the top level). `@ID` is a GUID
//!   derived from the PMEF `@id`, `@RefBaseSystemUnitPath` is `PMEF_SystemUnitClasses/<type>`,
//!   the PMEF `@id` sits in the `ExternalInterface "pmefId"` (spec 03 §4.3).
//! - `InstanceHierarchy "Relationships"`: relationship records (`relationType` or
//!   `sourceId`/`targetId`) as `InternalElement`s, plus one `InternalLink` between the
//!   `pmefId` interfaces of source and target when both are in the document.
//! - `SystemUnitClassLib "PMEF_SystemUnitClasses"`: one class per PMEF type used.
//!
//! Field encoding: a scalar becomes `Attribute` with `AttributeDataType` `xs:string`,
//! `xs:boolean`, `xs:long` (integers) or `xs:double`; an object becomes an `Attribute` with
//! nested `Attribute`s (`RefAttributeType="PMEF_AttributeTypeLib/Object"`), an array the same
//! with `…/Array` and children named `0`, `1`, …, `null` an empty `Attribute` of type `…/Null`.
//! Field names are the PMEF names (SI units, spec 01 §7), and `isPartOf` is kept as an attribute
//! as well as expressed by the nesting.

use serde_json::{Map, Number, Value};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use thiserror::Error;

const SUC_LIB: &str = "PMEF_SystemUnitClasses";
const ATTR_LIB: &str = "PMEF_AttributeTypeLib";
const RELATION_CONTAINER: &str = "PMEF_SystemUnitClasses/RelationshipContainer";

/// Errors reading a CAEX document.
#[derive(Debug, Error)]
pub enum CaexError {
    #[error("XML: {0}")]
    Xml(String),
    #[error("kein CAEXFile-Element")]
    NotCaex,
    #[error("{0}")]
    Invalid(String),
}

// ── writing ─────────────────────────────────────────────────────────────────────────────────

/// Serialise PMEF records (the `pmef:FileHeader` included, if present) as a CAEX 3.0 document.
pub fn write_caex(records: &[Value]) -> String {
    let header = records.iter().find(|r| type_of(r) == Some("pmef:FileHeader"));
    let objects: Vec<&Value> = records
        .iter()
        .filter(|r| type_of(r) != Some("pmef:FileHeader"))
        .collect();
    let (plant, relations): (Vec<&Value>, Vec<&Value>) =
        objects.iter().partition(|r| !is_relationship(r));

    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let file_name = header
        .and_then(|h| h.get("projectCode").or_else(|| h.get("plantId")))
        .and_then(Value::as_str)
        .unwrap_or("pmef");
    let _ = writeln!(
        x,
        "<CAEXFile xmlns=\"http://www.dke.de/CAEX\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" SchemaVersion=\"3.0\" FileName=\"{}.aml\">",
        esc(file_name)
    );

    // Header
    if let Some(h) = header {
        x.push_str("  <AdditionalInformation>\n");
        let mut tag = String::from("    <PMEFHeader");
        for key in ["pmefVersion", "plantId", "projectCode"] {
            if let Some(v) = h.get(key).and_then(Value::as_str) {
                let _ = write!(tag, " {key}=\"{}\"", esc(v));
            }
        }
        tag.push_str(">\n");
        x.push_str(&tag);
        if let Some(map) = h.as_object() {
            for (k, v) in map {
                write_attribute(&mut x, k, v, 6);
            }
        }
        x.push_str("    </PMEFHeader>\n  </AdditionalInformation>\n");
    }
    x.push_str("  <SuperiorStandardVersion>AutomationML 2.10</SuperiorStandardVersion>\n");
    let version = header
        .and_then(|h| h.get("pmefVersion"))
        .and_then(Value::as_str)
        .unwrap_or("0.9.0");
    let written = header
        .and_then(|h| h.get("createdAt").or_else(|| h.get("exportDate")))
        .and_then(Value::as_str)
        .unwrap_or("1970-01-01T00:00:00Z");
    let _ = writeln!(
        x,
        "  <SourceDocumentInformation OriginName=\"PMEF\" OriginID=\"urn:pmef\" OriginVersion=\"{}\" LastWritingDateTime=\"{}\" />",
        esc(version),
        esc(written)
    );

    // Plant model, nested along isPartOf
    let guid_of: HashMap<&str, String> = objects
        .iter()
        .filter_map(|r| id_of(r))
        .map(|id| (id, guid(id)))
        .collect();
    let children = nesting(&plant);
    x.push_str("  <InstanceHierarchy Name=\"PlantModel\">\n");
    for i in 0..plant.len() {
        if children.parent[i].is_none() {
            write_element(&mut x, &plant, &children.kids, i, 4);
        }
    }
    x.push_str("  </InstanceHierarchy>\n");

    if !relations.is_empty() {
        x.push_str("  <InstanceHierarchy Name=\"Relationships\">\n");
        let _ = writeln!(
            x,
            "    <InternalElement Name=\"Relationships\" ID=\"{}\" RefBaseSystemUnitPath=\"{RELATION_CONTAINER}\">",
            guid("urn:pmef:caex:relationships")
        );
        for r in &relations {
            write_object(&mut x, r, 6, |_| {});
        }
        for r in &relations {
            let (Some(src), Some(dst)) = (
                r.get("sourceId").and_then(Value::as_str),
                r.get("targetId").and_then(Value::as_str),
            ) else {
                continue;
            };
            if let (Some(a), Some(b)) = (guid_of.get(src), guid_of.get(dst)) {
                let _ = writeln!(
                    x,
                    "      <InternalLink Name=\"{}\" RefPartnerSideA=\"{a}:pmefId\" RefPartnerSideB=\"{b}:pmefId\" />",
                    esc(id_of(r).unwrap_or("link"))
                );
            }
        }
        x.push_str("    </InternalElement>\n  </InstanceHierarchy>\n");
    }

    x.push_str("  <InterfaceClassLib Name=\"PMEF_InterfaceClasses\">\n    <InterfaceClass Name=\"PmefObject\" />\n  </InterfaceClassLib>\n");

    let mut types: Vec<&str> = objects.iter().filter_map(|r| type_of(r)).collect();
    types.sort_unstable();
    types.dedup();
    let _ = writeln!(x, "  <SystemUnitClassLib Name=\"{SUC_LIB}\">");
    for t in types {
        let _ = writeln!(x, "    <SystemUnitClass Name=\"{}\" />", esc(class_name(t)));
    }
    if !relations.is_empty() {
        x.push_str("    <SystemUnitClass Name=\"RelationshipContainer\" />\n");
    }
    x.push_str("  </SystemUnitClassLib>\n");
    let _ = writeln!(
        x,
        "  <AttributeTypeLib Name=\"{ATTR_LIB}\">\n    <AttributeType Name=\"Object\" />\n    <AttributeType Name=\"Array\" />\n    <AttributeType Name=\"Null\" />\n  </AttributeTypeLib>"
    );
    x.push_str("</CAEXFile>\n");
    x
}

struct Nesting {
    parent: Vec<Option<usize>>,
    kids: Vec<Vec<usize>>,
}

/// Parent of each record along `isPartOf`, within the given records; cycles stay at the top.
fn nesting(records: &[&Value]) -> Nesting {
    let index: HashMap<&str, usize> = records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| Some((id_of(r)?, i)))
        .collect();
    let mut parent: Vec<Option<usize>> = records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let p = *index.get(r.get("isPartOf")?.as_str()?)?;
            (p != i).then_some(p)
        })
        .collect();
    // break cycles: walk up at most n steps
    for i in 0..records.len() {
        let mut seen = HashSet::new();
        let mut at = i;
        while let Some(p) = parent[at] {
            if !seen.insert(at) || p == i {
                parent[i] = None;
                break;
            }
            at = p;
        }
    }
    let mut kids = vec![Vec::new(); records.len()];
    for (i, p) in parent.iter().enumerate() {
        if let Some(p) = p {
            kids[*p].push(i);
        }
    }
    Nesting { parent, kids }
}

fn write_element(x: &mut String, recs: &[&Value], kids: &[Vec<usize>], i: usize, indent: usize) {
    write_object(x, recs[i], indent, |x| {
        for &k in &kids[i] {
            write_element(x, recs, kids, k, indent + 2);
        }
    });
}

fn write_object(x: &mut String, r: &Value, indent: usize, inner: impl FnOnce(&mut String)) {
    let pad = " ".repeat(indent);
    let id = id_of(r).unwrap_or("");
    let ty = type_of(r).unwrap_or("pmef:Object");
    let _ = writeln!(
        x,
        "{pad}<InternalElement Name=\"{}\" ID=\"{}\" RefBaseSystemUnitPath=\"{SUC_LIB}/{}\">",
        esc(&display_name(r)),
        guid(id),
        esc(class_name(ty))
    );
    if let Some(map) = r.as_object() {
        for (k, v) in map {
            if k == "@id" || k == "@type" {
                continue;
            }
            write_attribute(x, k, v, indent + 2);
        }
    }
    let _ = writeln!(
        x,
        "{pad}  <ExternalInterface Name=\"pmefId\" ID=\"{}\" RefBaseClassPath=\"PMEF_InterfaceClasses/PmefObject\">",
        guid(&format!("{id}#pmefId"))
    );
    write_attribute(x, "id", &Value::String(id.to_string()), indent + 4);
    let _ = writeln!(x, "{pad}  </ExternalInterface>");
    inner(x);
    let _ = writeln!(x, "{pad}</InternalElement>");
}

fn write_attribute(x: &mut String, name: &str, v: &Value, indent: usize) {
    let pad = " ".repeat(indent);
    let name = esc(name);
    match v {
        Value::Null => {
            let _ = writeln!(x, "{pad}<Attribute Name=\"{name}\" RefAttributeType=\"{ATTR_LIB}/Null\" />");
        }
        Value::Bool(b) => {
            let _ = writeln!(
                x,
                "{pad}<Attribute Name=\"{name}\" AttributeDataType=\"xs:boolean\"><Value>{b}</Value></Attribute>"
            );
        }
        Value::Number(n) => {
            let t = if n.is_f64() { "xs:double" } else { "xs:long" };
            let _ = writeln!(
                x,
                "{pad}<Attribute Name=\"{name}\" AttributeDataType=\"{t}\"><Value>{n}</Value></Attribute>"
            );
        }
        Value::String(s) => {
            let _ = writeln!(
                x,
                "{pad}<Attribute Name=\"{name}\" AttributeDataType=\"xs:string\"><Value>{}</Value></Attribute>",
                esc(s)
            );
        }
        Value::Array(items) => {
            if items.is_empty() {
                let _ = writeln!(x, "{pad}<Attribute Name=\"{name}\" RefAttributeType=\"{ATTR_LIB}/Array\" />");
                return;
            }
            let _ = writeln!(x, "{pad}<Attribute Name=\"{name}\" RefAttributeType=\"{ATTR_LIB}/Array\">");
            for (i, item) in items.iter().enumerate() {
                write_attribute(x, &i.to_string(), item, indent + 2);
            }
            let _ = writeln!(x, "{pad}</Attribute>");
        }
        Value::Object(map) => {
            if map.is_empty() {
                let _ = writeln!(x, "{pad}<Attribute Name=\"{name}\" RefAttributeType=\"{ATTR_LIB}/Object\" />");
                return;
            }
            let _ = writeln!(x, "{pad}<Attribute Name=\"{name}\" RefAttributeType=\"{ATTR_LIB}/Object\">");
            for (k, item) in map {
                write_attribute(x, k, item, indent + 2);
            }
            let _ = writeln!(x, "{pad}</Attribute>");
        }
    }
}

fn type_of(r: &Value) -> Option<&str> {
    r.get("@type")?.as_str()
}

fn id_of(r: &Value) -> Option<&str> {
    r.get("@id")?.as_str()
}

fn is_relationship(r: &Value) -> bool {
    r.get("relationType").is_some() || (r.get("sourceId").is_some() && r.get("targetId").is_some())
}

/// `pmef:Pump` → `Pump`; other prefixes stay (`ext:Foo`).
fn class_name(ty: &str) -> &str {
    ty.strip_prefix("pmef:").unwrap_or(ty)
}

fn type_from_class(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("pmef:{name}")
    }
}

/// CAEX `Name`: tag, line number or name, else the local part of the `@id`.
fn display_name(r: &Value) -> String {
    for key in ["tagNumber", "lineNumber", "name"] {
        if let Some(s) = r.get(key).and_then(Value::as_str) {
            if !s.trim().is_empty() {
                return s.to_string();
            }
        }
    }
    id_of(r)
        .map(|id| id.rsplit(':').next().unwrap_or(id).to_string())
        .unwrap_or_else(|| "object".to_string())
}

/// Stable GUID (8-4-4-4-12) from a string: two FNV-1a 64-bit hashes.
fn guid(s: &str) -> String {
    let fnv = |seed: u64| {
        s.bytes().fold(seed, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
    };
    let a = fnv(0xcbf2_9ce4_8422_2325);
    let b = fnv(0x84222325_cbf29ce4);
    let hex = format!("{a:016x}{b:016x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            c => out.push(c),
        }
    }
    out
}

// ── reading ─────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
}

fn parse_tree(xml: &str) -> Result<Node, CaexError> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut stack: Vec<Node> = vec![Node::default()];
    let to_node = |e: &quick_xml::events::BytesStart| -> Result<Node, CaexError> {
        let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
        let mut attrs = Vec::new();
        for a in e.attributes() {
            let a = a.map_err(|e| CaexError::Xml(e.to_string()))?;
            let key = String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned();
            let value = a
                .unescape_value()
                .map_err(|e| CaexError::Xml(e.to_string()))?
                .into_owned();
            attrs.push((key, value));
        }
        Ok(Node { name, attrs, ..Node::default() })
    };
    loop {
        match reader.read_event().map_err(|e| CaexError::Xml(e.to_string()))? {
            Event::Start(e) => stack.push(to_node(&e)?),
            Event::Empty(e) => {
                let node = to_node(&e)?;
                stack.last_mut().expect("root").children.push(node);
            }
            Event::End(_) => {
                let node = stack.pop().expect("open element");
                stack
                    .last_mut()
                    .ok_or_else(|| CaexError::Xml("unbalanced end tag".into()))?
                    .children
                    .push(node);
            }
            Event::Text(t) => {
                let text = t.unescape().map_err(|e| CaexError::Xml(e.to_string()))?;
                stack.last_mut().expect("root").text.push_str(&text);
            }
            Event::CData(t) => {
                let top = stack.last_mut().expect("root");
                top.text.push_str(&String::from_utf8_lossy(&t.into_inner()));
            }
            Event::Eof => break,
            _ => {}
        }
        if stack.is_empty() {
            return Err(CaexError::Xml("unbalanced end tag".into()));
        }
    }
    if stack.len() != 1 {
        return Err(CaexError::Xml("unclosed element".into()));
    }
    Ok(stack.pop().expect("root"))
}

/// Read a CAEX document into PMEF records (the `pmef:FileHeader` first, if present).
///
/// Documents written by [`write_caex`] come back unchanged. Other AutomationML files are read
/// as far as they map: each `InternalElement` becomes a record with `@type` from its system unit
/// class (`pmef:` prefixed when the class has no prefix), `@id` from the `pmefId` interface or
/// else `urn:pmef:obj:aml:<ID>`, `isPartOf` from the nesting and its `Attribute`s as fields.
pub fn read_caex(xml: &str) -> Result<Vec<Value>, CaexError> {
    let root = parse_tree(xml)?;
    let caex = root.child("CAEXFile").ok_or(CaexError::NotCaex)?;
    let mut out = Vec::new();
    if let Some(h) = caex
        .child("AdditionalInformation")
        .and_then(|a| a.child("PMEFHeader"))
    {
        let mut map = Map::new();
        for a in h.all("Attribute") {
            map.insert(a.attr("Name").unwrap_or("").to_string(), attribute_value(a));
        }
        if !map.is_empty() {
            out.push(Value::Object(map));
        }
    }
    for ih in caex.all("InstanceHierarchy") {
        for ie in ih.all("InternalElement") {
            read_element(ie, None, &mut out);
        }
    }
    Ok(out)
}

fn read_element(ie: &Node, parent: Option<&str>, out: &mut Vec<Value>) {
    if ie.attr("RefBaseSystemUnitPath") == Some(RELATION_CONTAINER) {
        for child in ie.all("InternalElement") {
            read_element(child, None, out);
        }
        return;
    }
    let pmef_id = ie
        .all("ExternalInterface")
        .find(|i| i.attr("Name") == Some("pmefId"))
        .and_then(|i| i.all("Attribute").find(|a| a.attr("Name") == Some("id")))
        .map(|a| a.child("Value").map(|v| v.text.clone()).unwrap_or_default());
    let id = pmef_id.unwrap_or_else(|| {
        format!(
            "urn:pmef:obj:aml:{}",
            ie.attr("ID").or(ie.attr("Name")).unwrap_or("object")
        )
    });
    let class = ie
        .attr("RefBaseSystemUnitPath")
        .map(|p| p.rsplit('/').next().unwrap_or(p))
        .unwrap_or("Object");
    let mut map = Map::new();
    map.insert("@type".into(), Value::String(type_from_class(class)));
    map.insert("@id".into(), Value::String(id.clone()));
    for a in ie.all("Attribute") {
        map.insert(a.attr("Name").unwrap_or("").to_string(), attribute_value(a));
    }
    if let Some(p) = parent {
        map.entry("isPartOf")
            .or_insert_with(|| Value::String(p.to_string()));
    }
    if !map.contains_key("name") && map.get("tagNumber").is_none() {
        if let Some(name) = ie.attr("Name") {
            if pmef_id_missing(ie) {
                map.insert("name".into(), Value::String(name.to_string()));
            }
        }
    }
    out.push(Value::Object(map));
    for child in ie.all("InternalElement") {
        read_element(child, Some(&id), out);
    }
}

fn pmef_id_missing(ie: &Node) -> bool {
    !ie.all("ExternalInterface").any(|i| i.attr("Name") == Some("pmefId"))
}

fn attribute_value(a: &Node) -> Value {
    let kind = a
        .attr("RefAttributeType")
        .map(|t| t.rsplit('/').next().unwrap_or(t));
    let subs: Vec<&Node> = a.all("Attribute").collect();
    match kind {
        Some("Null") => return Value::Null,
        Some("Array") => return Value::Array(subs.iter().map(|s| attribute_value(s)).collect()),
        Some("Object") => {
            return Value::Object(
                subs.iter()
                    .map(|s| (s.attr("Name").unwrap_or("").to_string(), attribute_value(s)))
                    .collect(),
            )
        }
        _ => {}
    }
    if !subs.is_empty() && a.child("Value").is_none() {
        return Value::Object(
            subs.iter()
                .map(|s| (s.attr("Name").unwrap_or("").to_string(), attribute_value(s)))
                .collect(),
        );
    }
    let text = a.child("Value").map(|v| v.text.as_str()).unwrap_or("");
    let dtype = a
        .attr("AttributeDataType")
        .map(|t| t.rsplit(':').next().unwrap_or(t).to_ascii_lowercase());
    match dtype.as_deref() {
        Some("boolean") => match text.trim() {
            "true" | "1" => Value::Bool(true),
            "false" | "0" => Value::Bool(false),
            _ => Value::String(text.to_string()),
        },
        Some("long" | "int" | "integer" | "short" | "unsignedint" | "unsignedlong") => {
            if let Ok(i) = text.trim().parse::<i64>() {
                Value::Number(i.into())
            } else if let Ok(u) = text.trim().parse::<u64>() {
                Value::Number(u.into())
            } else {
                Value::String(text.to_string())
            }
        }
        Some("double" | "float" | "decimal") => text
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(text.to_string())),
        _ => Value::String(text.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn example(name: &str) -> Vec<Value> {
        let path = format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|l| l.trim_start().starts_with('{'))
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn by_id(records: &[Value]) -> HashMap<String, &Value> {
        records
            .iter()
            .map(|r| (id_of(r).unwrap_or("").to_string(), r))
            .collect()
    }

    #[test]
    fn examples_round_trip_losslessly() {
        let dir = format!("{}/../../examples", env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("ndjson") {
                continue;
            }
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            let records = example(&name);
            let back = read_caex(&write_caex(&records)).unwrap();
            assert_eq!(back.len(), records.len(), "{name}");
            let back_by_id = by_id(&back);
            for r in &records {
                let id = id_of(r).unwrap_or("");
                let b = back_by_id.get(id).unwrap_or_else(|| panic!("{name}: {id} fehlt"));
                assert_eq!(*b, r, "{name}: {id}");
                // key order is kept too
                let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
                assert_eq!(keys(b), keys(r), "{name}: {id}");
            }
            checked += 1;
        }
        assert!(checked >= 5);
    }

    #[test]
    fn document_follows_spec_layout() {
        let records = example("pump-skid-complete.ndjson");
        let xml = write_caex(&records);
        assert!(xml.contains("SchemaVersion=\"3.0\""));
        assert!(xml.contains("<SuperiorStandardVersion>AutomationML 2.10</SuperiorStandardVersion>"));
        assert!(xml.contains("<PMEFHeader"));
        assert!(xml.contains("RefBaseSystemUnitPath=\"PMEF_SystemUnitClasses/Pump\""));
        assert!(xml.contains("<ExternalInterface Name=\"pmefId\""));
        assert!(xml.contains("<InternalLink "));
        // a pump sits inside its unit
        let tree = parse_tree(&xml).unwrap();
        let caex = tree.child("CAEXFile").unwrap();
        let plant = caex.all("InstanceHierarchy").next().unwrap();
        fn depth_of(n: &Node, class: &str, d: usize) -> Option<usize> {
            if n.attr("RefBaseSystemUnitPath") == Some(class) {
                return Some(d);
            }
            n.all("InternalElement").find_map(|c| depth_of(c, class, d + 1))
        }
        let pump = plant
            .all("InternalElement")
            .find_map(|c| depth_of(c, "PMEF_SystemUnitClasses/Pump", 0));
        assert!(pump.unwrap() >= 1, "Pumpe verschachtelt");
    }

    #[test]
    fn foreign_aml_reads_as_far_as_it_maps() {
        let xml = r#"<?xml version="1.0"?>
<CAEXFile SchemaVersion="3.0" FileName="x.aml" xmlns="http://www.dke.de/CAEX">
  <InstanceHierarchy Name="IH">
    <InternalElement Name="U-100" ID="a1" RefBaseSystemUnitPath="Lib/Unit">
      <InternalElement Name="P-101" ID="b2" RefBaseSystemUnitPath="Lib/Pump">
        <Attribute Name="designFlow_m3h" AttributeDataType="xs:double"><Value>12.5</Value></Attribute>
        <Attribute Name="Hersteller" AttributeDataType="xs:string"><Value>A &amp; B</Value></Attribute>
      </InternalElement>
    </InternalElement>
  </InstanceHierarchy>
</CAEXFile>"#;
        let recs = read_caex(xml).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1]["@type"], "pmef:Pump");
        assert_eq!(recs[1]["@id"], "urn:pmef:obj:aml:b2");
        assert_eq!(recs[1]["isPartOf"], "urn:pmef:obj:aml:a1");
        assert_eq!(recs[1]["designFlow_m3h"], json!(12.5));
        assert_eq!(recs[1]["Hersteller"], "A & B");
        assert_eq!(recs[1]["name"], "P-101");
    }

    #[test]
    fn edge_values_survive() {
        let r = json!({"@type":"pmef:Pump","@id":"urn:pmef:obj:p:1","a":null,"b":[],"c":{},
            "d":[1,2.5,"x",{"e":true}],"f":"  spaces\nline <&>\"'","g":-3,"h":1e-7,"i":18446744073709551615u64});
        let back = read_caex(&write_caex(std::slice::from_ref(&r))).unwrap();
        assert_eq!(back, vec![r]);
    }

    #[test]
    fn cycles_do_not_loop() {
        let a = json!({"@type":"pmef:Unit","@id":"a","isPartOf":"b"});
        let b = json!({"@type":"pmef:Unit","@id":"b","isPartOf":"a"});
        let back = read_caex(&write_caex(&[a.clone(), b.clone()])).unwrap();
        assert_eq!(back.len(), 2);
    }

    #[test]
    fn not_caex_is_an_error() {
        assert!(read_caex("<root/>").is_err());
        assert!(read_caex("<CAEXFile><a></CAEXFile>").is_err());
    }
}
