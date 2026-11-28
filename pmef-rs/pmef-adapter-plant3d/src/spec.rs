//! Plant 3D spec sheets (`<spec>.pspc`) → PMEF piping class catalog.
//!
//! A Plant 3D project keeps one spec per piping class in `Spec Sheets/<name>.pspc`. The file is
//! SQLite with the same table layout as `Piping.dcf`: every part size of the spec is a row of a
//! part class (`Pipe`, `Elbow`, `Flange`, …) with its catalogue attributes, its first port in
//! `EngineeringItems` and the further ports in `Port` via `PartPort`. `RepositoryDescriptor`
//! names the spec.
//!
//! [`spec_catalog`] writes one `PIPING_CLASS` catalog with `scope: "PROJECT"`. Each part size is
//! one entry with the id `<spec>:<size record GUID>` ([`entry_id`]). A placed part in
//! `Piping.dcf` carries the same size record and spec, so [`crate::pds::to_pmef`] writes the
//! matching `catalogRef`.
//!
//! The spec sheet settings (`<name>.pspx`: branch table, part use priorities) are not read.

use serde_json::{json, Map, Value as J};

use crate::pds::{
    component_spec, end_type, guid_string, kind, nominal_dn, weight_kg, Item, Kind, PdsProject,
};
use crate::sqlite::{Database, SqliteError};

/// Catalog entry id of a part size: `<spec>:<size record GUID>`.
pub fn entry_id(spec: &str, size_record: &str) -> String {
    format!("{}:{}", spec.trim(), size_record.to_ascii_lowercase())
}

/// Catalog id of a spec: `p3d-spec-<name>` (lowercase, `-`).
pub fn catalog_id(spec: &str) -> String {
    let mut id = String::from("p3d-spec-");
    for c in spec.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_end_matches('-').to_owned()
}

/// Counts of a spec conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecStats {
    /// Entries written.
    pub entries: usize,
    /// Part rows without a size record or PMEF class, skipped.
    pub skipped: usize,
}

/// Reads a `.pspc` file and writes it as a PMEF `PIPING_CLASS` catalog.
///
/// `fallback_name` names the spec when the file has no `RepositoryDescriptor` (normally the
/// file name without extension).
pub fn spec_catalog(data: &[u8], fallback_name: &str) -> Result<(J, SpecStats), SqliteError> {
    let db = Database::open(data)?;
    let desc = db.rows_or_empty("RepositoryDescriptor");
    let first = desc.rows.first();
    let text = |col: &str| {
        first
            .and_then(|r| desc.get(r, col).as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let name = text("Name").unwrap_or_else(|| fallback_name.trim().to_owned());
    let version = text("Version").unwrap_or_else(|| "1".to_owned());
    let description = text("Description").unwrap_or_else(|| format!("Plant 3D spec {name}"));

    let p = PdsProject::read(data)?;
    let mut stats = SpecStats::default();
    let mut entries = Vec::new();
    for i in p.items.values() {
        if !p.is_a(&i.class, "EngineeringItems") || p.is_a(&i.class, "ValveActuator") {
            continue;
        }
        match entry(&p, i, &name) {
            Some(e) => entries.push(e),
            None => stats.skipped += 1,
        }
    }
    entries.sort_by(|a, b| {
        let k = |e: &J| {
            (
                e["componentClass"].as_str().unwrap_or_default().to_owned(),
                e["nominalDiameter_mm"].as_f64().unwrap_or(0.0),
                e["catalogId"].as_str().unwrap_or_default().to_owned(),
            )
        };
        let (x, y) = (k(a), k(b));
        x.0.cmp(&y.0)
            .then(x.1.total_cmp(&y.1))
            .then_with(|| x.2.cmp(&y.2))
    });
    stats.entries = entries.len();
    let catalog = json!({
        "$schema": "https://pmef.net/schemas/0.9/pmef-catalog.schema.json",
        "catalogId": catalog_id(&name),
        "catalogType": "PIPING_CLASS",
        "scope": "PROJECT",
        "version": version,
        "description": description,
        "pipeClass": name,
        "units": {"length": "mm", "mass": "kg", "angle": "deg"},
        "note": "Converted from an AutoCAD Plant 3D spec (.pspc) by pmef-adapter-plant3d. Entry ids are <spec>:<size record GUID>, the same key a placed part carries in Piping.dcf.",
        "componentCount": entries.len(),
        "entries": entries,
    });
    Ok((catalog, stats))
}

/// One part size as a catalog entry.
fn entry(p: &PdsProject, i: &Item, spec: &str) -> Option<J> {
    let size = i
        .props
        .get("SizeRecordId")
        .and_then(|v| v.as_blob())
        .and_then(guid_string)?;
    let Kind::Component(_, class) = kind(p, i) else {
        return None;
    };
    let cs = component_spec(i, class);
    let mut e = Map::new();
    e.insert("catalogId".into(), json!(entry_id(spec, &size)));
    e.insert("componentClass".into(), json!(class));
    e.insert(
        "skey".into(),
        json!(cs["skey"].as_str().unwrap_or_default()),
    );
    let standard = ["CompatibleStandard", "FlangeStd", "GasketStd", "DesignStd"]
        .iter()
        .find_map(|c| i.text(c))
        .unwrap_or("Plant 3D");
    e.insert("standard".into(), json!(standard));
    let mut put = |k: &str, v: Option<J>| {
        if let Some(v) = v {
            e.insert(k.into(), v);
        }
    };
    put("description", i.text("PartSizeLongDesc").map(|s| json!(s)));
    put("partFamily", i.text("PartFamilyLongDesc").map(|s| json!(s)));
    put(
        "shortDescription",
        i.text("ShortDescription").map(|s| json!(s)),
    );
    let unit = i.text("NominalUnit");
    put(
        "nominalDiameter_mm",
        i.num("NominalDiameter").map(|d| json!(nominal_dn(d, unit))),
    );
    let positive = |c: &str| i.num(c).filter(|v| *v > 0.0).map(|v| json!(v));
    put("outsideDiameter_mm", positive("MatchingPipeOd"));
    put("wallThickness_mm", positive("WallThickness"));
    put("flangeThickness_mm", positive("FlangeThickness"));
    put("schedule", i.text("Schedule").map(|s| json!(s)));
    put("rating", i.text("PressureClass").map(|s| json!(s)));
    put(
        "material",
        i.text("Material")
            .or(i.text("MaterialCode"))
            .map(|s| json!(s)),
    );
    put("facing", i.text("Facing").map(|s| json!(s)));
    put(
        "mass_kg",
        weight_kg(i).filter(|w| *w > 0.0).map(|w| json!(w)),
    );
    put("angle_deg", positive("PathAngle"));
    put("radius", positive("CurveRadius"));
    put("length_mm", positive("Length"));
    put("manufacturer", i.text("Manufacturer").map(|s| json!(s)));
    put("itemCode", i.text("ItemCode").map(|s| json!(s)));
    put("boltSize", i.text("BoltSize").map(|s| json!(s)));
    put(
        "numberOfBolts",
        i.num("NumberInSet").map(|n| json!(n.round() as i64)),
    );
    put(
        "geometryParameters",
        i.text("ContentGeometryParamDefinition").map(|s| json!(s)),
    );
    // Ports: the part itself is the first port, `PartPort` lists the others.
    let ports: Vec<&Item> = p
        .ports
        .get(&i.id)
        .into_iter()
        .flatten()
        .filter_map(|(_, row)| p.items.get(row))
        .collect();
    for (k, port) in ports.iter().enumerate().take(3) {
        if let Some(et) = port.text("EndType").and_then(end_type) {
            e.insert(format!("endType{}", k + 1), json!(et));
        }
    }
    let dn = |port: &Item| {
        port.num("NominalDiameter")
            .map(|d| nominal_dn(d, port.text("NominalUnit")))
    };
    if let (Some(a), Some(b)) = (
        ports.first().and_then(|x| dn(x)),
        ports.get(1).and_then(|x| dn(x)),
    ) {
        if (a - b).abs() > 1e-6 && class.starts_with("REDUCER") {
            e.insert("reducingDiameter_mm".into(), json!(b));
        }
    }
    if let (Some(a), Some(c)) = (
        ports.first().and_then(|x| dn(x)),
        ports.get(2).and_then(|x| dn(x)),
    ) {
        if (a - c).abs() > 1e-6 {
            e.insert("branchDiameter_mm".into(), json!(c));
        }
    }
    e.insert(
        "vendorMappings".into(),
        json!([{"vendorSystem": "Plant3D", "vendorId": size}]),
    );
    Some(J::Object(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pds::{to_pmef, GeometryIndex, PdsOptions};

    const SPEC: &[u8] = include_bytes!("../../../tests/plant3d/spec-synthetic.pspc");
    const PDS: &[u8] = include_bytes!("../../../tests/plant3d/pds-synthetic.dcf");

    fn by_class<'a>(c: &'a J, class: &str) -> &'a J {
        c["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["componentClass"] == class)
            .unwrap_or_else(|| panic!("no {class}"))
    }

    #[test]
    fn spec_becomes_a_project_piping_class() {
        let (c, stats) = spec_catalog(SPEC, "fallback").unwrap();
        assert_eq!(c["catalogId"], "p3d-spec-16hc01");
        assert_eq!(c["catalogType"], "PIPING_CLASS");
        assert_eq!(c["scope"], "PROJECT");
        assert_eq!(c["pipeClass"], "16HC01");
        assert_eq!(c["version"], "5_0");
        assert_eq!(c["description"], "Carbon steel, PN 16 (synthetic)");
        // The valve actuator is not a spec entry.
        assert_eq!(
            stats,
            SpecStats {
                entries: 4,
                skipped: 0
            }
        );

        let pipe = by_class(&c, "PIPE");
        assert_eq!(
            pipe["catalogId"],
            "16HC01:00000000-0000-0000-0000-000000009001"
        );
        assert_eq!(pipe["standard"], "EN 10216-2");
        assert_eq!(pipe["nominalDiameter_mm"], 200.0);
        assert_eq!(pipe["outsideDiameter_mm"], 219.1);
        assert_eq!(pipe["wallThickness_mm"], 6.3);
        assert_eq!(pipe["endType1"], "BW");
        assert_eq!(pipe["endType2"], "BW");
        assert_eq!(pipe["skey"], "");

        let elbow = by_class(&c, "ELBOW");
        assert_eq!(elbow["skey"], "BEBW");
        assert_eq!(elbow["angle_deg"], 90.0);
        assert_eq!(elbow["radius"], 305.0);

        let red = by_class(&c, "REDUCER_CONCENTRIC");
        assert_eq!(red["reducingDiameter_mm"], 150.0);

        let fl = by_class(&c, "FLANGE");
        assert_eq!(
            (fl["endType1"].as_str(), fl["endType2"].as_str()),
            (Some("FL"), Some("BW"))
        );
        assert_eq!(fl["rating"], "PN16");
        assert_eq!(fl["facing"], "RF");
        assert_eq!(fl["vendorMappings"][0]["vendorSystem"], "Plant3D");
    }

    #[test]
    fn placed_parts_reference_their_spec_entry() {
        let (c, _) = spec_catalog(SPEC, "x").unwrap();
        let ids: Vec<&str> = c["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["catalogId"].as_str())
            .collect();
        let p = PdsProject::read(PDS).unwrap();
        let opts = PdsOptions {
            project_code: p.project_code(),
        };
        let (objs, _) = to_pmef(&p, &opts, &GeometryIndex::new());
        let refs: Vec<&str> = objs
            .iter()
            .filter_map(|o| o["catalogRef"]["catalogId"].as_str())
            .collect();
        assert_eq!(refs.len(), 2, "pipe and elbow");
        assert!(refs.iter().all(|r| ids.contains(r)), "{refs:?}");
    }

    #[test]
    fn ids_are_stable_and_readable() {
        assert_eq!(
            catalog_id("CustomParts Metric"),
            "p3d-spec-customparts-metric"
        );
        assert_eq!(entry_id(" 10HS01 ", "ABC"), "10HS01:abc");
    }
}
