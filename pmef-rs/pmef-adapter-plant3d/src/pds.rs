//! Plant 3D project database (`Piping.dcf`) → PMEF.
//!
//! An AutoCAD Plant 3D project keeps its engineering data in SQLite files next to the model
//! DWGs. `Piping.dcf` holds every piping part, equipment item and steel member of the 3D
//! model with its catalogue attributes, the port list of each part, the connection graph and
//! the line groups. `PnPDataLinks` ties each row to the DWG object (drawing + handle) that
//! carries its geometry.
//!
//! The database has no port coordinates. They come from the DWG geometry: the caller passes
//! a [`GeometryIndex`] with port candidates per DWG object (for example the end rings of the
//! proxy graphics), and [`to_pmef`] assigns them to the ports through the connection graph.
//! Without geometry the topology is still kept as `pmef:IsConnectedTo` relations and in
//! `pmef:Weld.connects`; only the `ports` arrays stay empty.
//!
//! Every object keeps all of its Plant 3D attributes as `customAttributes` (`p3d:<column>`),
//! its native ids (`p3d:PnPID`, `p3d:PnPGuid`) and a provenance reference to the DWG object.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{json, Map, Value as J};

use crate::equipment::p3d_class_to_pmef;
use crate::sqlite::{Database, SqliteError, Value};

/// Data version written to `pmefVersion`.
const PMEF_VERSION: &str = "0.9.0";
/// Two port candidates closer than this are the same point [mm].
const PORT_TOLERANCE: f64 = 25.0;

/// A DWG object: drawing (project-relative path, `/` separated) and entity handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DwgKey {
    /// Project-relative DWG path, e.g. `Plant 3D-Modelle/Bereich 51/Zone 1/30-Rohrleitungen/A.dwg`.
    pub drawing: String,
    /// Entity handle (`DwgHandleHigh << 32 | DwgHandleLow`).
    pub handle: u64,
}

/// One possible port location found in the geometry of a DWG object.
#[derive(Debug, Clone, PartialEq)]
pub struct PortCandidate {
    /// Port centre point [mm].
    pub point: [f64; 3],
    /// Axis at the port (either sign), if known — e.g. the normal of an end ring.
    pub normal: Option<[f64; 3]>,
    /// Ring radius [mm], if known.
    pub radius: Option<f64>,
}

/// Geometry facts about one DWG object needed for port assignment.
#[derive(Debug, Clone, PartialEq)]
pub struct PartGeometry {
    /// Centre of the object's geometry [mm]. For a connector this is its location.
    pub centroid: [f64; 3],
    /// Possible port locations.
    pub candidates: Vec<PortCandidate>,
    /// Placement of the object (row-major 4x4, translation in the last column) when the
    /// drawing stores one. Kept as `p3d:placement`; a support without ports gets its
    /// location as port `P1`.
    pub placement: Option<[f64; 16]>,
}

/// Port candidates per DWG object.
pub type GeometryIndex = HashMap<DwgKey, PartGeometry>;

/// Where a database row is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DwgLink {
    /// The DWG object.
    pub key: DwgKey,
    /// 0 for the object itself; >0 for sub-parts drawn by the same object (welds, gaskets and
    /// bolt sets of a connector, nozzles of an equipment item).
    pub sub_index: i64,
}

/// One project database row (a part, port, line, equipment item, steel member, …).
#[derive(Debug, Clone)]
pub struct Item {
    /// `PnPID`.
    pub id: i64,
    /// `PnPClassName`, e.g. `Pipe`, `Elbow`, `StructureMember`.
    pub class: String,
    /// `PnPGuid` as a lowercase GUID string.
    pub guid: Option<String>,
    /// All columns of the class tables (`EngineeringItems`, `PipeRunComponent`, `Elbow`, …).
    pub props: BTreeMap<String, Value>,
    /// DWG object, if the row is drawn.
    pub link: Option<DwgLink>,
}

impl Item {
    fn text(&self, column: &str) -> Option<&str> {
        self.props
            .get(column)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
    fn num(&self, column: &str) -> Option<f64> {
        self.props.get(column).and_then(Value::as_f64)
    }
}

/// A row of `P3dPartConnection`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// First part (`PnPID`).
    pub part1: i64,
    /// Port name on the first part, e.g. `S1`.
    pub port1: String,
    /// Second part (`PnPID`).
    pub part2: i64,
    /// Port name on the second part.
    pub port2: String,
}

/// The parts of `Piping.dcf` that PMEF needs.
#[derive(Debug, Clone, Default)]
pub struct PdsProject {
    /// `PnPProject.Project_Name`.
    pub project_name: Option<String>,
    /// `PnPProject.Project_Number`.
    pub project_number: Option<String>,
    /// Rows by `PnPID`.
    pub items: BTreeMap<i64, Item>,
    /// Ports of each part: part → (port name, port row id).
    pub ports: BTreeMap<i64, Vec<(String, i64)>>,
    /// Connection graph.
    pub connections: Vec<Connection>,
    /// Line group of each part.
    pub line_of: BTreeMap<i64, i64>,
    /// Drawing rows: id → project-relative path.
    pub drawings: BTreeMap<i64, String>,
    /// Class hierarchy: class → base class (from `PnPTables`).
    pub parents: HashMap<String, String>,
}

/// Root classes read from the database.
const ROOTS: [&str; 5] = [
    "EngineeringItems",
    "SteelStructure",
    "P3dLineGroup",
    "Port",
    "P3dConnector",
];

impl PdsProject {
    /// Reads a `Piping.dcf` file.
    pub fn read(data: &[u8]) -> Result<Self, SqliteError> {
        let db = Database::open(data)?;
        let mut p = PdsProject::default();

        let tables = db.rows_or_empty("PnPTables");
        for row in &tables.rows {
            if let (Some(t), Some(b)) = (
                tables.get(row, "TableName").as_str(),
                tables.get(row, "BaseTable").as_str(),
            ) {
                p.parents.insert(t.to_owned(), b.to_owned());
            }
        }

        let base = db.rows("PnPBase")?;
        for row in &base.rows {
            let (Some(id), Some(class)) = (
                base.get(row, "PnPID").as_i64(),
                base.get(row, "PnPClassName").as_str(),
            ) else {
                continue;
            };
            if !ROOTS.iter().any(|r| p.is_a(class, r)) {
                continue;
            }
            let guid = base.get(row, "PnPGuid").as_blob().and_then(guid_string);
            p.items.insert(
                id,
                Item {
                    id,
                    class: class.to_owned(),
                    guid,
                    props: BTreeMap::new(),
                    link: None,
                },
            );
        }

        // Merge the columns of every class table into its rows.
        let class_tables: Vec<String> = p
            .parents
            .keys()
            .filter(|t| ROOTS.iter().any(|r| p.is_a(t, r)))
            .cloned()
            .collect();
        for table in class_tables {
            let rows = db.rows_or_empty(&table);
            let Some(id_col) = rows.col("PnPID") else {
                continue;
            };
            for row in &rows.rows {
                let Some(item) = row.values[id_col]
                    .as_i64()
                    .and_then(|id| p.items.get_mut(&id))
                else {
                    continue;
                };
                for (c, v) in rows.columns.iter().zip(&row.values) {
                    if matches!(c.as_str(), "PnPID" | "PnPGuid" | "PnPTimestamp")
                        || matches!(v, Value::Null)
                    {
                        continue;
                    }
                    if matches!(v, Value::Text(s) if s.trim().is_empty()) {
                        continue;
                    }
                    item.props.insert(c.clone(), v.clone());
                }
            }
        }

        if let Ok(rows) = db.rows("PnPProject") {
            if let Some(r) = rows.rows.first() {
                p.project_name = rows.get(r, "Project_Name").as_str().map(str::to_owned);
                p.project_number = rows.get(r, "Project_Number").as_str().map(str::to_owned);
            }
        }

        let dwgs = db.rows_or_empty("PnPDrawings");
        for row in &dwgs.rows {
            let rel = dwgs
                .get(row, "PnPRelativePath")
                .as_str()
                .or(dwgs.get(row, "Dwg Name").as_str());
            if let (Some(id), Some(rel)) = (dwgs.get(row, "PnPID").as_i64(), rel) {
                p.drawings.insert(id, rel.replace('\\', "/"));
            }
        }

        let links = db.rows_or_empty("PnPDataLinks");
        for row in &links.rows {
            let (Some(item), Some(dwg)) = (
                links.get(row, "RowId").as_i64(),
                links.get(row, "DwgId").as_i64(),
            ) else {
                continue;
            };
            let Some(drawing) = p.drawings.get(&dwg).cloned() else {
                continue;
            };
            let low = links.get(row, "DwgHandleLow").as_i64().unwrap_or(0) as u64 & 0xFFFF_FFFF;
            let high = links.get(row, "DwgHandleHigh").as_i64().unwrap_or(0) as u64 & 0xFFFF_FFFF;
            let sub_index = links.get(row, "DwgSubIndex").as_i64().unwrap_or(0);
            if let Some(it) = p.items.get_mut(&item) {
                it.link = Some(DwgLink {
                    key: DwgKey {
                        drawing,
                        handle: high << 32 | low,
                    },
                    sub_index,
                });
            }
        }

        let pp = db.rows_or_empty("PartPort");
        for row in &pp.rows {
            if let (Some(part), Some(port), Some(name)) = (
                pp.get(row, "Part").as_i64(),
                pp.get(row, "Port").as_i64(),
                pp.get(row, "Name").as_str(),
            ) {
                p.ports
                    .entry(part)
                    .or_default()
                    .push((name.to_owned(), port));
            }
        }
        // A part is its own first port: `EngineeringItems` carries PortName, NominalDiameter and
        // EndType of it; `PartPort` lists the further ports.
        for item in p.items.values() {
            if let Some(name) = item.text("PortName") {
                let ports = p.ports.entry(item.id).or_default();
                if !ports.iter().any(|(n, _)| n == name) {
                    ports.push((name.to_owned(), item.id));
                }
            }
        }
        for v in p.ports.values_mut() {
            v.sort();
        }

        let conn = db.rows_or_empty("P3dPartConnection");
        for row in &conn.rows {
            if let (Some(a), Some(b)) = (
                conn.get(row, "Part1").as_i64(),
                conn.get(row, "Part2").as_i64(),
            ) {
                p.connections.push(Connection {
                    part1: a,
                    port1: conn
                        .get(row, "Port1")
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    part2: b,
                    port2: conn
                        .get(row, "Port2")
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                });
            }
        }

        let lg = db.rows_or_empty("P3dLineGroupPartRelationship");
        for row in &lg.rows {
            if let (Some(line), Some(part)) = (
                lg.get(row, "LineGroup").as_i64(),
                lg.get(row, "Part").as_i64(),
            ) {
                p.line_of.insert(part, line);
            }
        }
        Ok(p)
    }

    /// PMEF project segment from the project number or name (lowercase, `-`), or `plant3d`.
    pub fn project_code(&self) -> String {
        let raw = self
            .project_number
            .as_deref()
            .or(self.project_name.as_deref())
            .unwrap_or("plant3d");
        let mut code = String::new();
        for c in raw.to_lowercase().chars() {
            if c.is_ascii_alphanumeric() {
                code.push(c);
            } else if !code.ends_with('-') {
                code.push('-');
            }
        }
        let code = code.trim_matches('-').to_owned();
        if code.is_empty() {
            "plant3d".to_owned()
        } else {
            code
        }
    }

    /// `class` is `ancestor` or derives from it (`PnPTables.BaseTable`).
    pub fn is_a(&self, class: &str, ancestor: &str) -> bool {
        let mut c = class;
        for _ in 0..32 {
            if c == ancestor {
                return true;
            }
            match self.parents.get(c) {
                Some(b) if !b.is_empty() => c = b,
                _ => return false,
            }
        }
        false
    }

    /// Rows drawn by a DWG object, for attaching the database attributes to the geometry.
    pub fn linked_items(&self) -> impl Iterator<Item = &Item> {
        self.items.values().filter(|i| i.link.is_some())
    }

    /// The row drawn by a DWG object itself (sub index 0).
    pub fn item_for(&self, key: &DwgKey) -> Option<&Item> {
        self.items.values().find(|i| {
            i.link
                .as_ref()
                .is_some_and(|l| &l.key == key && l.sub_index == 0)
        })
    }
}

/// .NET GUID byte order → canonical string.
fn guid_string(b: &[u8]) -> Option<String> {
    if b.len() != 16 {
        return None;
    }
    Some(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    ))
}

/// Options for [`to_pmef`].
#[derive(Debug, Clone)]
pub struct PdsOptions {
    /// PMEF project segment of the ids (lowercase letters, digits, `-`).
    pub project_code: String,
}

/// Counts of a conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PdsStats {
    /// Objects written, by `@type`.
    pub objects: BTreeMap<String, usize>,
    /// Ports written with a coordinate.
    pub ports_placed: usize,
    /// Ports in the database without a coordinate (no geometry found).
    pub ports_unplaced: usize,
    /// `pmef:IsConnectedTo` relations written.
    pub connections: usize,
    /// Rows without a PMEF type, by Plant 3D class (e.g. gratings, railings).
    pub unmapped: BTreeMap<String, usize>,
}

/// Kind of PMEF object for a Plant 3D class.
enum Kind {
    Component(&'static str, &'static str),
    Weld(&'static str),
    Equipment,
    Steel,
    Skip,
}

fn kind(p: &PdsProject, item: &Item) -> Kind {
    let c = item.class.as_str();
    let is = |a: &str| p.is_a(c, a);
    let iso = item
        .text("ContentIsoSymbolDefinition")
        .unwrap_or_default()
        .to_ascii_uppercase();
    if is("P3dConnector") || is("Port") || is("P3dLineGroup") || is("Nozzle") {
        return Kind::Skip;
    }
    if is("Equipment") {
        return Kind::Equipment;
    }
    if is("StructureMember") {
        return Kind::Steel;
    }
    if is("Buttweld") {
        return Kind::Weld("WELD_BUTT");
    }
    if is("Socketweld") {
        return Kind::Weld("WELD_SOCKET");
    }
    if is("TapWeld") || is("FusionWeld") {
        return Kind::Weld("WELD_FILLET");
    }
    let generic = "pmef:GenericPipingComponent";
    let k = if is("Pipe") {
        ("pmef:Pipe", "PIPE")
    } else if is("Elbow") {
        ("pmef:Elbow", "ELBOW")
    } else if is("Lateral") || is("Wye") {
        ("pmef:Tee", "LATERAL")
    } else if is("Cross") {
        ("pmef:Tee", "CROSS")
    } else if is("SingleBranchFitting") {
        ("pmef:Tee", "TEE")
    } else if is("Reducer") || is("Swage") {
        (
            "pmef:Reducer",
            if iso.contains("ECCENTRIC") {
                "REDUCER_ECCENTRIC"
            } else {
                "REDUCER_CONCENTRIC"
            },
        )
    } else if is("BlindFlange") {
        ("pmef:Flange", "BLIND_FLANGE")
    } else if is("Flange") {
        ("pmef:Flange", "FLANGE")
    } else if is("Valve") {
        (
            "pmef:Valve",
            valve_class(item.text("PartFamilyLongDesc").unwrap_or_default()),
        )
    } else if is("Olet") {
        ("pmef:Olet", "OLET_WELDOLET")
    } else if is("Gasket") {
        ("pmef:Gasket", "GASKET")
    } else if is("Support") {
        ("pmef:PipeSupport", "PIPE_SUPPORT")
    } else if is("BoltSet") {
        (generic, "BOLT_SET")
    } else if is("Instrument") {
        (generic, "INLINE_INSTRUMENT")
    } else if is("SpectacleBlind") || is("BlindDisk") {
        (generic, "SPECTACLE_BLIND")
    } else if is("SpacerDisk") || is("BleedRing") {
        (generic, "SPACER")
    } else if is("Cap") {
        (generic, "CAP")
    } else if is("Plug") {
        (generic, "PLUG")
    } else if is("Coupling") {
        (generic, "COUPLING")
    } else if is("Strainer") {
        (generic, "STRAINER_Y")
    } else if is("PipeRunComponent") || is("Fasteners") {
        (generic, "SPECIAL")
    } else {
        return Kind::Skip;
    };
    Kind::Component(k.0, k.1)
}

/// Valve class from the part family description (English and German keywords).
fn valve_class(desc: &str) -> &'static str {
    let d = desc.to_lowercase();
    let has = |ks: &[&str]| ks.iter().any(|k| d.contains(k));
    if has(&["check", "rückschlag", "rueckschlag", "rk86"]) {
        "VALVE_CHECK"
    } else if has(&["safety", "saftey", "relief", "sicherheit"]) {
        "VALVE_RELIEF"
    } else if has(&["butterfly", "klappe", "stobu"]) {
        "VALVE_BUTTERFLY"
    } else if has(&["ball", "kugel"]) {
        "VALVE_BALL"
    } else if has(&["gate", "slide", "schieber"]) {
        "VALVE_GATE"
    } else if has(&["globe", "ventil", "stevi"]) {
        "VALVE_GLOBE"
    } else if has(&["control", "regel"]) {
        "VALVE_CONTROL"
    } else if has(&["needle", "nadel"]) {
        "VALVE_NEEDLE"
    } else if has(&["plug", "küken"]) {
        "VALVE_PLUG"
    } else if has(&["diaphragm", "membran"]) {
        "VALVE_DIAPHRAGM"
    } else {
        "VALVE_UNSPECIFIED"
    }
}

/// Plant 3D end type → PMEF end type.
fn end_type(s: &str) -> Option<&'static str> {
    let u = s.to_ascii_uppercase();
    Some(match u.as_str() {
        "BV" | "BW" => "BW",
        "SW" | "BRAZED" => "SW",
        "FL" | "LUG" | "WF" => "FL",
        "PL" => "PLAIN",
        "LAP" => "STUB",
        _ if u.starts_with("THD") || u.starts_with("TH") => "SC",
        _ => return None,
    })
}

/// Nominal size → DN [mm].
fn nominal_dn(size: f64, unit: Option<&str>) -> f64 {
    if !unit.is_some_and(|u| u.eq_ignore_ascii_case("in")) {
        return size;
    }
    const NPS: [(f64, f64); 26] = [
        (0.125, 6.0),
        (0.25, 8.0),
        (0.375, 10.0),
        (0.5, 15.0),
        (0.75, 20.0),
        (1.0, 25.0),
        (1.25, 32.0),
        (1.5, 40.0),
        (2.0, 50.0),
        (2.5, 65.0),
        (3.0, 80.0),
        (3.5, 90.0),
        (4.0, 100.0),
        (5.0, 125.0),
        (6.0, 150.0),
        (8.0, 200.0),
        (10.0, 250.0),
        (12.0, 300.0),
        (14.0, 350.0),
        (16.0, 400.0),
        (18.0, 450.0),
        (20.0, 500.0),
        (22.0, 550.0),
        (24.0, 600.0),
        (30.0, 750.0),
        (36.0, 900.0),
    ];
    NPS.iter()
        .find(|(n, _)| (n - size).abs() < 1e-6)
        .map(|(_, d)| *d)
        .unwrap_or((size * 25.0).round())
}

/// Plant 3D steel size → PMEF profile id (`EN:HEA200`, `EN:L100x10`, `EN:CHS42.4x3.2`, …).
pub fn profile_id(size: &str) -> String {
    let s = size.trim();
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let up = compact.to_ascii_uppercase();
    for fam in ["HEA", "HEB", "HEM", "IPE", "UPE", "UPN"] {
        if let Some(rest) = up.strip_prefix(fam) {
            return format!("EN:{fam}{}", rest.trim_start_matches('-'));
        }
    }
    if let Some(rest) = up.strip_prefix("U-") {
        return format!("EN:UPN{rest}");
    }
    if let Some(rest) = up.strip_prefix("L-") {
        return format!("EN:L{}", rest.to_lowercase());
    }
    if let Some(rest) = up.strip_prefix("PIPE") {
        return format!("EN:CHS{}", rest.to_lowercase());
    }
    if let Some(rest) = up.strip_prefix("FL.") {
        return format!("EN:FLAT{}", rest.to_lowercase());
    }
    format!("P3D:{compact}")
}

/// Percent-encodes a path for a URI (keeps unreserved characters and `/`).
fn uri_path(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// Characters allowed in a PMEF local id.
fn local_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-._".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(v, v).sqrt();
    (l > 1e-9).then(|| [v[0] / l, v[1] / l, v[2] / l])
}
fn round3(v: [f64; 3], digits: i32) -> [f64; 3] {
    let f = 10f64.powi(digits);
    v.map(|x| {
        let r = (x * f).round() / f;
        if r == 0.0 {
            0.0
        } else {
            r
        }
    })
}

/// Coordinate of a PMEF port object.
fn coord(port: &J) -> Option<[f64; 3]> {
    let c = port.get("coordinate")?.as_array()?;
    match c.as_slice() {
        [x, y, z] => Some([x.as_f64()?, y.as_f64()?, z.as_f64()?]),
        _ => None,
    }
}

/// A placed port.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed {
    point: [f64; 3],
    direction: Option<[f64; 3]>,
}

/// Assigns coordinates to the ports in the database from the geometry candidates.
///
/// 1. A port joined to a connector sits at the connector; the nearest end ring of the part
///    gives the exact centre and the axis.
/// 2. Two parts joined directly meet where a candidate of one coincides with a candidate of
///    the other.
/// 3. A part with exactly one unplaced port and exactly one unused candidate gets that one.
fn place_ports(p: &PdsProject, geom: &GeometryIndex) -> HashMap<(i64, String), Placed> {
    let g = |id: i64| -> Option<&PartGeometry> {
        // Sub-parts (nozzles) use the geometry of the object that draws them.
        geom.get(&p.items.get(&id)?.link.as_ref()?.key)
    };
    let is_connector = |id: i64| {
        p.items
            .get(&id)
            .is_some_and(|i| p.is_a(&i.class, "P3dConnector"))
    };
    let near = |pg: &PartGeometry, at: [f64; 3]| -> Option<Placed> {
        let c = pg
            .candidates
            .iter()
            .min_by(|a, b| dist(a.point, at).total_cmp(&dist(b.point, at)))
            .filter(|c| dist(c.point, at) <= PORT_TOLERANCE)?;
        Some(Placed {
            point: c.point,
            direction: oriented(c, pg.centroid),
        })
    };

    let mut placed: HashMap<(i64, String), Placed> = HashMap::new();
    for c in &p.connections {
        let ends = [(c.part1, &c.port1, c.part2), (c.part2, &c.port2, c.part1)];
        for (part, port, other) in ends {
            if is_connector(part) || !is_connector(other) {
                continue;
            }
            let Some(at) = g(other).map(|k| k.centroid) else {
                continue;
            };
            let pl = g(part).and_then(|pg| near(pg, at)).unwrap_or(Placed {
                point: at,
                direction: None,
            });
            placed.insert((part, port.clone()), pl);
        }
        if !is_connector(c.part1) && !is_connector(c.part2) {
            if let (Some(a), Some(b)) = (g(c.part1), g(c.part2)) {
                let best = a
                    .candidates
                    .iter()
                    .flat_map(|x| b.candidates.iter().map(move |y| (x, y)))
                    .min_by(|(x1, y1), (x2, y2)| {
                        dist(x1.point, y1.point).total_cmp(&dist(x2.point, y2.point))
                    });
                if let Some((x, y)) = best.filter(|(x, y)| dist(x.point, y.point) <= PORT_TOLERANCE)
                {
                    placed.insert(
                        (c.part1, c.port1.clone()),
                        Placed {
                            point: x.point,
                            direction: oriented(x, a.centroid),
                        },
                    );
                    placed.insert(
                        (c.part2, c.port2.clone()),
                        Placed {
                            point: y.point,
                            direction: oriented(y, b.centroid),
                        },
                    );
                }
            }
        }
    }

    for (&part, ports) in &p.ports {
        let Some(pg) = g(part) else { continue };
        let open: Vec<&String> = ports
            .iter()
            .map(|(n, _)| n)
            .filter(|n| !placed.contains_key(&(part, (*n).clone())))
            .collect();
        if open.len() != 1 {
            continue;
        }
        let used: Vec<[f64; 3]> = ports
            .iter()
            .filter_map(|(n, _)| placed.get(&(part, n.clone())).map(|x| x.point))
            .collect();
        let mut free: Vec<&PortCandidate> = Vec::new();
        for c in &pg.candidates {
            if used.iter().all(|u| dist(*u, c.point) > PORT_TOLERANCE)
                && free.iter().all(|f| dist(f.point, c.point) > PORT_TOLERANCE)
            {
                free.push(c);
            }
        }
        if let [c] = free[..] {
            placed.insert(
                (part, open[0].clone()),
                Placed {
                    point: c.point,
                    direction: oriented(c, pg.centroid),
                },
            );
        }
    }

    // The centre of the drawn body is off-axis for parts with operators or supports (a
    // hand lever makes both faces of a wafer valve point the same way). With two or more
    // placed ports the ports themselves give the inside of the part.
    for (&part, ports) in &p.ports {
        let keys: Vec<(i64, String)> = ports
            .iter()
            .map(|(n, _)| (part, n.clone()))
            .filter(|k| placed.contains_key(k))
            .collect();
        if keys.len() < 2 {
            continue;
        }
        let mut mid = [0.0; 3];
        for k in &keys {
            let pt = placed[k].point;
            for (m, x) in mid.iter_mut().zip(pt) {
                *m += x / keys.len() as f64;
            }
        }
        for k in &keys {
            let pl = placed.get_mut(k).expect("placed port");
            let out = sub(pl.point, mid);
            if let Some(d) = pl.direction.as_mut() {
                if dot(*d, out) < 0.0 {
                    *d = d.map(|x| -x);
                }
            }
        }
    }
    placed
}

/// Candidate axis pointing away from the part centre.
fn oriented(c: &PortCandidate, centroid: [f64; 3]) -> Option<[f64; 3]> {
    let n = unit(c.normal?)?;
    let out = sub(c.point, centroid);
    Some(if dot(n, out) < 0.0 { n.map(|x| -x) } else { n })
}

/// Converts a Plant 3D project database to PMEF objects (NDJSON lines).
pub fn to_pmef(p: &PdsProject, opts: &PdsOptions, geom: &GeometryIndex) -> (Vec<J>, PdsStats) {
    let proj = local_id(&opts.project_code.to_lowercase());
    let mut stats = PdsStats::default();
    let placed = place_ports(p, geom);

    let obj_id = |i: &Item| {
        format!(
            "urn:pmef:obj:{proj}:{}",
            local_id(i.guid.as_deref().unwrap_or(&format!("pnp-{}", i.id)))
        )
    };
    let line_id = |i: &Item| {
        format!(
            "urn:pmef:line:{proj}:{}",
            local_id(i.guid.as_deref().unwrap_or(&format!("pnp-{}", i.id)))
        )
    };
    let plant_id = format!("urn:pmef:plant:{proj}:plant");

    // Units: one per top-level model folder of the drawings.
    let unit_of = |i: &Item| -> String {
        let folder = i
            .link
            .as_ref()
            .and_then(|l| {
                let parts: Vec<&str> = l.key.drawing.split('/').collect();
                (parts.len() > 2).then(|| parts[1].to_owned())
            })
            .unwrap_or_else(|| "model".to_owned());
        format!("urn:pmef:unit:{proj}:{}", local_id(&folder))
    };

    let mut out: Vec<J> = Vec::new();
    let name = p
        .project_name
        .clone()
        .unwrap_or_else(|| opts.project_code.clone());
    out.push(json!({"@type": "pmef:FileHeader", "@id": format!("urn:pmef:pkg:{proj}:plant3d-pds"), "pmefVersion": PMEF_VERSION,
        "plantId": plant_id, "projectCode": p.project_number.clone().unwrap_or_else(|| opts.project_code.clone()),
        "coordinateSystem": "Z-up", "units": "mm", "authoringTool": "AutoCAD Plant 3D (Piping.dcf via pmef-adapter-plant3d)",
        "description": format!("Plant 3D project {name}")}));
    out.push(
        json!({"@type": "pmef:Plant", "@id": plant_id, "pmefVersion": PMEF_VERSION, "name": name}),
    );

    let mut units: BTreeSet<String> = BTreeSet::new();
    for i in p.items.values() {
        if !matches!(kind(p, i), Kind::Skip) {
            units.insert(unit_of(i));
        }
    }
    for u in &units {
        let n = u.rsplit(':').next().unwrap_or_default();
        out.push(json!({"@type": "pmef:Unit", "@id": u, "pmefVersion": PMEF_VERSION, "name": n, "isPartOf": plant_id}));
    }

    // Lines and their segment.
    let mut line_members: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for (&part, &line) in &p.line_of {
        if let Some(i) = p.items.get(&part) {
            if matches!(kind(p, i), Kind::Component(..) | Kind::Weld(_)) {
                line_members.entry(line).or_default().push(obj_id(i));
            }
        }
    }
    let seg_of = |line: &Item| {
        format!(
            "urn:pmef:seg:{proj}:{}-S1",
            local_id(line.guid.as_deref().unwrap_or(&format!("pnp-{}", line.id)))
        )
    };
    for line in p
        .items
        .values()
        .filter(|i| p.is_a(&i.class, "P3dLineGroup"))
    {
        let members = line_members.get(&line.id).cloned().unwrap_or_default();
        let unit = p
            .line_of
            .iter()
            .find(|(_, &l)| l == line.id)
            .and_then(|(part, _)| p.items.get(part))
            .map(unit_of)
            .unwrap_or_else(|| {
                units
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| format!("urn:pmef:unit:{proj}:model"))
            });
        let number = line
            .text("Tag")
            .or(line.text("Number"))
            .unwrap_or("unnamed")
            .to_owned();
        let mut o = json!({"@type": "pmef:PipingNetworkSystem", "@id": line_id(line), "pmefVersion": PMEF_VERSION,
            "lineNumber": number, "isPartOf": unit, "segments": [seg_of(line)]});
        if let Some(s) = line.text("Service") {
            o["mediumCode"] = json!(s);
        }
        // Piping class: the line's spec, else the spec most of its parts use.
        let spec = line.text("NominalSpec").map(str::to_owned).or_else(|| {
            let mut n: BTreeMap<&str, usize> = BTreeMap::new();
            for (part, _) in p.line_of.iter().filter(|(_, &l)| l == line.id) {
                if let Some(s) = p.items.get(part).and_then(|i| i.text("Spec")) {
                    *n.entry(s).or_default() += 1;
                }
            }
            n.into_iter()
                .max_by_key(|(_, c)| *c)
                .map(|(s, _)| s.to_owned())
        });
        if let Some(s) = spec {
            o["pipeClass"] = json!(s);
        }
        add_native(&mut o, line, None);
        out.push(o);
        out.push(json!({"@type": "pmef:PipingSegment", "@id": seg_of(line), "isPartOf": line_id(line), "segmentNumber": 1, "components": members}));
    }

    // Sub-parts of a connector (welds, gaskets, bolt sets) → the connector.
    let mut connector_of: HashMap<i64, i64> = HashMap::new();
    let by_key: HashMap<&DwgKey, i64> = p
        .items
        .values()
        .filter_map(|i| {
            i.link
                .as_ref()
                .filter(|l| l.sub_index == 0)
                .map(|l| (&l.key, i.id))
        })
        .collect();
    for i in p.items.values() {
        if let Some(l) = i.link.as_ref().filter(|l| l.sub_index > 0) {
            if let Some(&host) = by_key.get(&l.key) {
                connector_of.insert(i.id, host);
            }
        }
    }

    // Connections resolved through connectors: part ↔ part.
    let is_connector = |id: i64| {
        p.items
            .get(&id)
            .is_some_and(|i| p.is_a(&i.class, "P3dConnector"))
    };
    let mut via: BTreeMap<i64, Vec<(i64, String)>> = BTreeMap::new(); // connector → parts
    let mut pairs: Vec<((i64, String), (i64, String))> = Vec::new();
    for c in &p.connections {
        match (is_connector(c.part1), is_connector(c.part2)) {
            (false, true) => via
                .entry(c.part2)
                .or_default()
                .push((c.part1, c.port1.clone())),
            (true, false) => via
                .entry(c.part1)
                .or_default()
                .push((c.part2, c.port2.clone())),
            (false, false) => pairs.push(((c.part1, c.port1.clone()), (c.part2, c.port2.clone()))),
            (true, true) => {}
        }
    }
    // A flanged joint with a gasket: part ↔ gasket ↔ part (the gasket fills the gap).
    let mut gasket_of: HashMap<i64, i64> = HashMap::new();
    for (&sub, &host) in &connector_of {
        if p.items
            .get(&sub)
            .is_some_and(|i| p.is_a(&i.class, "Gasket"))
        {
            gasket_of.entry(host).or_insert(sub);
        }
    }
    let mut gasket_ports: HashMap<i64, [(i64, String); 2]> = HashMap::new();
    for (k, parts) in &via {
        if let (Some(&g), [a, b]) = (gasket_of.get(k), parts.as_slice()) {
            pairs.push((a.clone(), (g, "P1".to_owned())));
            pairs.push(((g, "P2".to_owned()), b.clone()));
            gasket_ports.insert(g, [a.clone(), b.clone()]);
            continue;
        }
        for (k, a) in parts.iter().enumerate() {
            for b in &parts[k + 1..] {
                pairs.push((a.clone(), b.clone()));
            }
        }
    }
    let mut peer: HashMap<(i64, String), i64> = HashMap::new();
    for (a, b) in &pairs {
        peer.insert(a.clone(), b.0);
        peer.insert(b.clone(), a.0);
    }
    let placement_of = |i: &Item| -> Option<[f64; 16]> {
        geom.get(&i.link.as_ref().filter(|l| l.sub_index == 0)?.key)?
            .placement
    };
    let connector_point = |cid: i64| -> Option<[f64; 3]> {
        let l = p.items.get(&cid)?.link.as_ref()?;
        geom.get(&l.key).map(|g| g.centroid)
    };

    // Id a port refers to: a nozzle is written inside its equipment, so it stands for that.
    let target_id = |other: &Item| -> String {
        if p.is_a(&other.class, "Nozzle") {
            if let Some(host) = connector_of.get(&other.id).and_then(|h| p.items.get(h)) {
                return obj_id(host);
            }
        }
        obj_id(other)
    };
    // Piping network a nozzle connects to: the line of the part joined to it.
    let line_ref = |other: &Item| -> Option<String> {
        p.line_of
            .get(&other.id)
            .and_then(|l| p.items.get(l))
            .map(line_id)
    };

    let mut nozzles_of: BTreeMap<i64, Vec<&Item>> = BTreeMap::new();
    for i in p.items.values().filter(|i| p.is_a(&i.class, "Nozzle")) {
        if let Some(&host) = connector_of.get(&i.id) {
            nozzles_of.entry(host).or_default().push(i);
        }
    }

    for i in p.items.values() {
        let k = kind(p, i);
        let isa = line_of_seg(p, i, &seg_of).unwrap_or_else(|| unit_of(i));
        let o = match k {
            Kind::Skip => {
                if !(p.is_a(&i.class, "P3dConnector")
                    || p.is_a(&i.class, "Port")
                    || p.is_a(&i.class, "P3dLineGroup")
                    || p.is_a(&i.class, "Nozzle"))
                {
                    *stats.unmapped.entry(i.class.clone()).or_default() += 1;
                }
                continue;
            }
            Kind::Component(ty, class) => {
                let mut o = json!({"@type": ty, "@id": obj_id(i), "pmefVersion": PMEF_VERSION, "isPartOf": isa,
                    "componentSpec": component_spec(i, class)});
                if ty == "pmef:GenericPipingComponent" {
                    o["nativeType"] = json!(format!("Plant3D:{}", i.class));
                }
                if let Some(t) = i.text("Tag") {
                    o["tagNumber"] = json!(t);
                }
                type_fields(&mut o, ty, i, p);
                add_end_types(&mut o, p, i);
                let mut ports = match gasket_ports.get(&i.id) {
                    Some(ends) => gasket_ports_json(p, ends, &placed, &target_id, &mut stats),
                    None => component_ports(p, i, &placed, &peer, &target_id, &mut stats),
                };
                if ports.is_empty() {
                    if let Some(at) = connector_of.get(&i.id).and_then(|&c| connector_point(c)) {
                        ports.push(json!({"portId": "P1", "coordinate": round3(at, 3)}));
                        stats.ports_placed += 1;
                    }
                }
                // The cut length includes the socket or thread engagement; PMEF
                // `pipeLength` is the port-to-port length. `p3d:CutLength` keeps the cut.
                if let (true, [a, b]) = (ty == "pmef:Pipe", ports.as_slice()) {
                    if let (Some(a), Some(b)) = (coord(a), coord(b)) {
                        o["pipeLength"] = json!((dist(a, b) * 10.0).round() / 10.0);
                    }
                }
                if ports.is_empty() && ty == "pmef:PipeSupport" {
                    if let Some(m) = placement_of(i) {
                        ports.push(
                            json!({"portId": "P1", "coordinate": round3([m[3], m[7], m[11]], 3)}),
                        );
                        stats.ports_placed += 1;
                    }
                }
                if !ports.is_empty() {
                    o["ports"] = J::Array(ports);
                }
                o
            }
            Kind::Weld(class) => {
                let joined: Vec<String> = connector_of
                    .get(&i.id)
                    .and_then(|c| via.get(c))
                    .map(|v| {
                        v.iter()
                            .filter_map(|(id, _)| p.items.get(id))
                            .map(&obj_id)
                            .collect()
                    })
                    .unwrap_or_default();
                let mut o = json!({"@type": "pmef:Weld", "@id": obj_id(i), "pmefVersion": PMEF_VERSION, "isPartOf": isa,
                    "componentSpec": component_spec(i, class),
                    "weldSpec": {"weldNumber": i.text("Tag").map(str::to_owned).unwrap_or_else(|| format!("W-{}", i.id)),
                                 "weldType": if class == "WELD_SOCKET" { "SW" } else if class == "WELD_FILLET" { "FW" } else { "BW" }}});
                if joined.len() == 2 {
                    o["connects"] = json!(joined);
                } else {
                    o["@type"] = json!("pmef:GenericPipingComponent");
                    o["nativeType"] = json!(format!("Plant3D:{}", i.class));
                    o.as_object_mut().map(|m| m.remove("weldSpec"));
                }
                if let Some(at) = connector_of.get(&i.id).and_then(|&c| connector_point(c)) {
                    o["ports"] = json!([{"portId": "P1", "coordinate": round3(at, 3)}]);
                    stats.ports_placed += 1;
                }
                o
            }
            Kind::Equipment => {
                let words = camel_words(&i.class);
                let (ty, class) = p3d_class_to_pmef(&words);
                let tag = i
                    .text("Tag")
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{}-{}", i.class, i.id));
                let mut basic = json!({"tagNumber": tag, "equipmentClass": class});
                if let Some(d) = i.text("Description").or(i.text("PartFamilyLongDesc")) {
                    basic["serviceDescription"] = json!(d);
                }
                if let Some(m) = i.text("Manufacturer") {
                    basic["manufacturer"] = json!(m);
                }
                let mut o = json!({"@type": ty, "@id": obj_id(i), "pmefVersion": PMEF_VERSION, "isPartOf": unit_of(i), "equipmentBasic": basic});
                // The database has no design data; the typed specs stay empty.
                if let Some(spec) = match ty {
                    "pmef:Pump" => Some("pumpSpec"),
                    "pmef:Vessel" => Some("vesselDesign"),
                    "pmef:HeatExchanger" => Some("hxSpec"),
                    _ => None,
                } {
                    o[spec] = json!({});
                }
                let mut nozzles = Vec::new();
                for n in nozzles_of.get(&i.id).into_iter().flatten() {
                    if let Some(nz) = nozzle(p, n, &placed, &peer, &line_ref, &mut stats) {
                        nozzles.push(nz);
                    }
                }
                if !nozzles.is_empty() {
                    o["nozzles"] = J::Array(nozzles);
                }
                o
            }
            Kind::Steel => {
                let (Some(s), Some(e)) = (point(i, "Start"), point(i, "End")) else {
                    *stats.unmapped.entry(i.class.clone()).or_default() += 1;
                    continue;
                };
                let d = sub(e, s);
                let len = dot(d, d).sqrt();
                let member = if len > 0.0 && d[2].abs() / len > 0.98 {
                    "COLUMN"
                } else {
                    "BEAM"
                };
                let size = i.text("Size").unwrap_or("UNKNOWN");
                let mut o = json!({"@type": "pmef:SteelMember", "@id": obj_id(i), "pmefVersion": PMEF_VERSION, "isPartOf": unit_of(i),
                    "memberType": member, "profileId": profile_id(size), "startPoint": round3(s, 3), "endPoint": round3(e, 3)});
                // The database length is the cut length (with end cuts); PMEF `length` is the
                // system line length. The cut length stays in `p3d:Length`.
                o["length"] = json!((len * 1000.0).round() / 1000.0);
                if let Some(t) = i.text("Tag") {
                    o["memberMark"] = json!(t);
                }
                let grade = i
                    .text("Materialcode")
                    .or(i.text("MaterialCode"))
                    .or(i.text("Material"));
                if let Some(g) = grade {
                    let mut m = json!({"grade": g});
                    if let Some(st) = i.text("MaterialStandard") {
                        m["standard"] = json!(st);
                    }
                    o["material"] = m;
                }
                o
            }
        };
        let mut o = o;
        add_native(&mut o, i, i.link.as_ref());
        if let Some(m) = placement_of(i) {
            // customAttributes hold scalars: the 16 numbers row-major, space separated.
            let text: Vec<String> = m
                .iter()
                .map(|x| ((x * 1e6).round() / 1e6 + 0.0).to_string())
                .collect();
            o["customAttributes"]["p3d:placement"] = json!(text.join(" "));
        }
        *stats
            .objects
            .entry(o["@type"].as_str().unwrap_or_default().to_owned())
            .or_default() += 1;
        out.push(o);
    }

    // Topology as relations (kept even when ports have no coordinates).
    let mut seen = BTreeSet::new();
    for (a, b) in &pairs {
        let (Some(ia), Some(ib)) = (p.items.get(&a.0), p.items.get(&b.0)) else {
            continue;
        };
        if matches!(kind(p, ia), Kind::Skip) && !p.is_a(&ia.class, "Nozzle") {
            continue;
        }
        if matches!(kind(p, ib), Kind::Skip) && !p.is_a(&ib.class, "Nozzle") {
            continue;
        }
        let (sa, sb) = (
            owner_id(p, ia, &connector_of, &obj_id),
            owner_id(p, ib, &connector_of, &obj_id),
        );
        let key = if (&sa, &a.1) <= (&sb, &b.1) {
            (sa.clone(), a.1.clone(), sb.clone(), b.1.clone())
        } else {
            (sb.clone(), b.1.clone(), sa.clone(), a.1.clone())
        };
        if !seen.insert(key.clone()) {
            continue;
        }
        let rel = format!(
            "urn:pmef:rel:{proj}:conn-{}-{}-{}-{}",
            ia.id.min(ib.id),
            ia.id.max(ib.id),
            local_id(&key.1),
            local_id(&key.3)
        );
        out.push(json!({"@type": "pmef:IsConnectedTo", "@id": rel, "relationType": "IS_CONNECTED_TO",
            "sourceId": key.0, "targetId": key.2, "connectionPointSource": key.1, "connectionPointTarget": key.3,
            "derivedBy": "ADAPTER_IMPORT", "confidence": 1.0}));
        stats.connections += 1;
    }
    *stats
        .objects
        .entry("pmef:IsConnectedTo".into())
        .or_default() += stats.connections;
    (out, stats)
}

/// The PMEF object that represents a row (a nozzle is part of its equipment).
fn owner_id(
    p: &PdsProject,
    i: &Item,
    connector_of: &HashMap<i64, i64>,
    obj_id: &dyn Fn(&Item) -> String,
) -> String {
    if p.is_a(&i.class, "Nozzle") {
        if let Some(host) = connector_of.get(&i.id).and_then(|h| p.items.get(h)) {
            return obj_id(host);
        }
    }
    obj_id(i)
}

fn line_of_seg(p: &PdsProject, i: &Item, seg_of: &dyn Fn(&Item) -> String) -> Option<String> {
    let line = p.items.get(p.line_of.get(&i.id)?)?;
    Some(seg_of(line))
}

/// `HeatExchanger` → `Heat Exchanger`.
fn camel_words(s: &str) -> String {
    let mut o = String::new();
    for (k, c) in s.chars().enumerate() {
        if k > 0 && c.is_uppercase() {
            o.push(' ');
        }
        o.push(c);
    }
    o
}

fn point(i: &Item, prefix: &str) -> Option<[f64; 3]> {
    Some([
        i.num(&format!("{prefix} X"))?,
        i.num(&format!("{prefix} Y"))?,
        i.num(&format!("{prefix} Z"))?,
    ])
}

fn weight_kg(i: &Item) -> Option<f64> {
    let w = i.num("Weight")?;
    let lb = i
        .text("WeightUnit")
        .is_some_and(|u| u.to_ascii_lowercase().starts_with("lb"));
    Some(if lb { w * 0.453_592_37 } else { w })
}

fn component_spec(i: &Item, class: &str) -> J {
    let mut s = json!({"componentClass": class});
    let iso = i.text("ContentIsoSymbolDefinition").unwrap_or_default();
    if let Some(sk) = iso
        .split(',')
        .find_map(|kv| kv.trim().strip_prefix("SKEY="))
        .filter(|s| !s.is_empty() && s.len() <= 8)
    {
        s["skey"] = json!(sk);
    }
    if let Some(w) = weight_kg(i) {
        s["weight"] = json!(w);
    }
    if let Some(f) = i
        .text("Facing")
        .map(str::to_ascii_uppercase)
        .filter(|f| ["RF", "FF", "RTJ", "AARH", "TG", "MF"].contains(&f.as_str()))
    {
        s["facingType"] = json!(f);
    }
    s
}

fn type_fields(o: &mut J, ty: &str, i: &Item, p: &PdsProject) {
    match ty {
        "pmef:Pipe" => {
            if let Some(l) = i.num("CutLength").or(i.num("Length")) {
                o["pipeLength"] = json!(l);
            }
        }
        "pmef:Elbow" => {
            if let Some(a) = i.num("PathAngle") {
                o["angle"] = json!(a);
            }
            if let Some(r) = i.num("CurveRadius") {
                o["radius"] = json!("CUSTOM");
                o["radiusMm"] = json!(r);
            }
        }
        "pmef:Tee" => {
            if let Some(a) = i.num("BranchAngle1") {
                o["branchAngle"] = json!(a);
            }
        }
        "pmef:Flange" => {
            if p.is_a(&i.class, "BlindFlange") {
                o["flangeType"] = json!("BLIND");
            }
            if let Some(r) = i.text("PressureClass") {
                o["rating"] = json!(r);
            }
        }
        "pmef:Gasket" => {
            if let Some(m) = i.text("Material") {
                o["gasketMaterial"] = json!(m);
            }
        }
        "pmef:PipeSupport" => {
            if let Some(t) = i.text("Tag") {
                o["supportsMark"] = json!(t);
            }
        }
        _ => {}
    }
}

fn port_props(p: &PdsProject, port_row: i64) -> (Option<f64>, Option<&'static str>) {
    let Some(r) = p.items.get(&port_row) else {
        return (None, None);
    };
    let dn = r
        .num("NominalDiameter")
        .map(|d| nominal_dn(d, r.text("NominalUnit")));
    (dn, r.text("EndType").and_then(end_type))
}

fn component_ports(
    p: &PdsProject,
    i: &Item,
    placed: &HashMap<(i64, String), Placed>,
    peer: &HashMap<(i64, String), i64>,
    obj_id: &dyn Fn(&Item) -> String,
    stats: &mut PdsStats,
) -> Vec<J> {
    let mut out = Vec::new();
    for (name, row) in p.ports.get(&i.id).into_iter().flatten() {
        let Some(pl) = placed.get(&(i.id, name.clone())) else {
            stats.ports_unplaced += 1;
            continue;
        };
        let mut port = json!({"portId": name, "coordinate": round3(pl.point, 3)});
        if let Some(d) = pl.direction {
            port["direction"] = json!(round3(d, 6));
        }
        let (dn, et) = port_props(p, *row);
        if let Some(dn) = dn {
            port["nominalDiameter"] = json!(dn);
        }
        if let Some(et) = et {
            port["endType"] = json!(et);
        }
        if let Some(other) = peer.get(&(i.id, name.clone())).and_then(|o| p.items.get(o)) {
            port["connectedTo"] = json!(obj_id(other));
        }
        stats.ports_placed += 1;
        out.push(port);
    }
    out
}

/// Ports of a gasket: on the faces of the two joined parts, pointing back at them.
fn gasket_ports_json(
    p: &PdsProject,
    ends: &[(i64, String); 2],
    placed: &HashMap<(i64, String), Placed>,
    obj_id: &dyn Fn(&Item) -> String,
    stats: &mut PdsStats,
) -> Vec<J> {
    let mut out = Vec::new();
    for (k, (part, port)) in ends.iter().enumerate() {
        let Some(pl) = placed.get(&(*part, port.clone())) else {
            stats.ports_unplaced += 1;
            continue;
        };
        let mut j = json!({"portId": format!("P{}", k + 1), "coordinate": round3(pl.point, 3)});
        if let Some(d) = pl.direction {
            j["direction"] = json!(round3(d.map(|x| -x), 6));
        }
        if let Some(i) = p.items.get(part) {
            j["connectedTo"] = json!(obj_id(i));
        }
        stats.ports_placed += 1;
        out.push(j);
    }
    out
}

/// End types of the first two ports into the component spec.
fn add_end_types(o: &mut J, p: &PdsProject, i: &Item) {
    for (k, (_, row)) in p.ports.get(&i.id).into_iter().flatten().take(2).enumerate() {
        if let Some(et) = port_props(p, *row).1 {
            o["componentSpec"][format!("endType{}", k + 1)] = json!(et);
        }
    }
}

fn nozzle(
    p: &PdsProject,
    n: &Item,
    placed: &HashMap<(i64, String), Placed>,
    peer: &HashMap<(i64, String), i64>,
    line_ref: &dyn Fn(&Item) -> Option<String>,
    stats: &mut PdsStats,
) -> Option<J> {
    let (name, row) = p.ports.get(&n.id).and_then(|v| v.first())?;
    let Some(pl) = placed.get(&(n.id, name.clone())).copied().or_else(|| {
        point(n, "Position").map(|pt| Placed {
            point: pt,
            direction: None,
        })
    }) else {
        stats.ports_unplaced += 1;
        return None;
    };
    let Some(dir) = pl.direction else {
        stats.ports_unplaced += 1;
        return None;
    };
    let mark = n
        .text("Tag")
        .or(n.text("Number"))
        .map(|s| format!("N{}", s.trim_start_matches('N')))
        .unwrap_or_else(|| name.clone());
    let mut z = json!({"nozzleId": name, "nozzleMark": mark, "coordinate": round3(pl.point, 3), "direction": round3(dir, 6)});
    let (dn, _) = port_props(p, *row);
    if let Some(dn) = dn {
        z["nominalDiameter"] = json!(dn);
    }
    if let Some(line) = peer
        .get(&(n.id, name.clone()))
        .and_then(|o| p.items.get(o))
        .and_then(line_ref)
    {
        z["connectedLineId"] = json!(line);
    }
    stats.ports_placed += 1;
    Some(z)
}

/// Native ids, all attributes and the DWG reference.
fn add_native(o: &mut J, i: &Item, link: Option<&DwgLink>) {
    let mut ca = Map::new();
    ca.insert("p3d:PnPID".into(), json!(i.id));
    ca.insert("p3d:PnPClassName".into(), json!(i.class));
    if let Some(g) = &i.guid {
        ca.insert("p3d:PnPGuid".into(), json!(g));
    }
    for (k, v) in &i.props {
        let j = match v {
            Value::Integer(n) => json!(n),
            Value::Real(f) if f.is_finite() => json!(f),
            Value::Text(s) => json!(s),
            _ => continue,
        };
        ca.insert(format!("p3d:{k}"), j);
    }
    if let Some(l) = link {
        ca.insert("p3d:DwgHandle".into(), json!(format!("{:X}", l.key.handle)));
        o["provenance"] = json!({"sourceType": "cad-import",
            "sourceRef": format!("plant3d-project:///{}#handle={:X}", uri_path(&l.key.drawing), l.key.handle)});
    }
    o["customAttributes"] = J::Object(ca);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_uses_dotnet_byte_order() {
        let b = [
            0x24, 0x35, 0xd4, 0x2b, 0xa2, 0x5e, 0x46, 0xb1, 0x97, 0x48, 0x61, 0x75, 0x11, 0x91,
            0x98, 0xee,
        ];
        assert_eq!(
            guid_string(&b).unwrap(),
            "2bd43524-5ea2-b146-9748-6175119198ee"
        );
    }

    #[test]
    fn profiles_map_to_catalog_ids() {
        assert_eq!(profile_id("HEA-200"), "EN:HEA200");
        assert_eq!(profile_id("IPE-100"), "EN:IPE100");
        assert_eq!(profile_id("U-200"), "EN:UPN200");
        assert_eq!(profile_id("L-100x10"), "EN:L100x10");
        assert_eq!(profile_id("Pipe 42.4 x 3.2"), "EN:CHS42.4x3.2");
        assert_eq!(profile_id("Fl. 100x5"), "EN:FLAT100x5");
        assert_eq!(profile_id("50x50x4"), "P3D:50x50x4");
    }

    #[test]
    fn valve_classes_from_family_text() {
        assert_eq!(valve_class("Shut-off ball cock PN 16, C"), "VALVE_BALL");
        assert_eq!(valve_class("RK86 Check valve  PN 16, C"), "VALVE_CHECK");
        assert_eq!(valve_class("Erhard Schieber 2712, PN 16, C"), "VALVE_GATE");
        assert_eq!(
            valve_class("Shut-off butterfly valve PN 10"),
            "VALVE_BUTTERFLY"
        );
        assert_eq!(valve_class("Saftey corner valve  PN 16, C"), "VALVE_RELIEF");
    }

    const FIXTURE: &[u8] = include_bytes!("../../../tests/plant3d/pds-synthetic.dcf");

    fn geometry(p: &PdsProject) -> GeometryIndex {
        let key = |id: i64| p.items[&id].link.as_ref().unwrap().key.clone();
        let ring = |pt: [f64; 3], n: [f64; 3]| PortCandidate {
            point: pt,
            normal: Some(n),
            radius: Some(109.55),
        };
        let mut g = GeometryIndex::new();
        g.insert(
            key(10),
            PartGeometry {
                centroid: [500.0, 0.0, 0.0],
                candidates: vec![
                    ring([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                    ring([1000.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
                ],
                placement: None,
            },
        );
        g.insert(
            key(20),
            PartGeometry {
                centroid: [1000.0, 0.0, 0.0],
                candidates: vec![],
                placement: None,
            },
        );
        g.insert(
            key(30),
            PartGeometry {
                centroid: [1100.0, 100.0, 0.0],
                candidates: vec![
                    ring([1000.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                    ring([1305.0, 305.0, 0.0], [0.0, 1.0, 0.0]),
                ],
                placement: None,
            },
        );
        g.insert(
            key(40),
            PartGeometry {
                centroid: [1305.0, 305.0, 0.0],
                candidates: vec![],
                placement: None,
            },
        );
        g.insert(
            key(50),
            PartGeometry {
                centroid: [1305.0, 800.0, 0.0],
                candidates: vec![ring([1305.0, 305.0, 0.0], [0.0, 1.0, 0.0])],
                placement: Some([
                    1.0, 0.0, 0.0, 1305.0, 0.0, 1.0, 0.0, 800.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                    1.0,
                ]),
            },
        );
        g
    }

    fn by_type<'a>(objs: &'a [J], ty: &str) -> Vec<&'a J> {
        objs.iter().filter(|o| o["@type"] == ty).collect()
    }

    #[test]
    fn reads_the_project_database() {
        let p = PdsProject::read(FIXTURE).unwrap();
        assert_eq!(p.project_name.as_deref(), Some("Synthetic demo"));
        assert_eq!(p.project_code(), "syn-1");
        let pipe = &p.items[&10];
        assert_eq!(pipe.class, "Pipe");
        assert_eq!(
            pipe.guid.as_deref(),
            Some("00000000-0000-0000-0000-00000000000a")
        );
        assert_eq!(pipe.num("CutLength"), Some(1000.0));
        assert_eq!(pipe.text("Shop_Field"), Some("SHOP"));
        assert_eq!(
            pipe.props["PartSizeLongDesc"].as_str().unwrap().len(),
            3000,
            "overflow pages are followed"
        );
        let link = pipe.link.as_ref().unwrap();
        assert_eq!(
            link.key,
            DwgKey {
                drawing: "Models/Area A/pipe.dwg".into(),
                handle: 0x1A0
            }
        );
        assert_eq!(p.items[&21].link.as_ref().unwrap().sub_index, 1);
        assert_eq!(
            p.ports[&10]
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>(),
            ["S1", "S2"]
        );
        assert_eq!(p.connections.len(), 4);
        assert!(p.is_a("Pump", "EngineeringItems"));
        assert_eq!(p.item_for(&link.key).map(|i| i.id), Some(10));
    }

    #[test]
    fn maps_to_pmef_with_ports_from_geometry() {
        let p = PdsProject::read(FIXTURE).unwrap();
        let (objs, stats) = to_pmef(
            &p,
            &PdsOptions {
                project_code: "syn".into(),
            },
            &geometry(&p),
        );
        let id = |n: u32| format!("urn:pmef:obj:syn:00000000-0000-0000-0000-{n:012x}");

        let line = by_type(&objs, "pmef:PipingNetworkSystem")[0];
        assert_eq!(line["lineNumber"], "L-100");
        assert_eq!(line["pipeClass"], "16HC01");
        assert_eq!(line["isPartOf"], "urn:pmef:unit:syn:Area-A");

        let pipe = by_type(&objs, "pmef:Pipe")[0];
        assert_eq!(pipe["@id"], id(10));
        assert_eq!(pipe["pipeLength"], 1000.0);
        assert_eq!(pipe["componentSpec"]["endType1"], "BW");
        let ports = pipe["ports"].as_array().unwrap();
        assert_eq!(ports[0]["portId"], "S1");
        assert_eq!(ports[0]["coordinate"], json!([0.0, 0.0, 0.0]));
        assert_eq!(
            ports[0]["direction"],
            json!([-1.0, 0.0, 0.0]),
            "by elimination, pointing away from the pipe"
        );
        assert_eq!(ports[1]["coordinate"], json!([1000.0, 0.0, 0.0]));
        assert_eq!(ports[1]["direction"], json!([1.0, 0.0, 0.0]));
        assert_eq!(ports[1]["connectedTo"], id(30), "through the connector");
        assert_eq!(ports[1]["nominalDiameter"], 200.0);
        assert_eq!(pipe["customAttributes"]["p3d:Shop_Field"], "SHOP");
        assert_eq!(
            pipe["provenance"]["sourceRef"],
            "plant3d-project:///Models/Area%20A/pipe.dwg#handle=1A0"
        );

        let elbow = by_type(&objs, "pmef:Elbow")[0];
        assert_eq!(elbow["angle"], 90.0);
        assert_eq!(elbow["radiusMm"], 305.0);
        assert_eq!(elbow["componentSpec"]["skey"], "BEBW");
        assert_eq!(elbow["ports"][1]["direction"], json!([0.0, 1.0, 0.0]));
        assert_eq!(
            elbow["ports"][1]["connectedTo"],
            id(50),
            "a nozzle stands for its equipment"
        );

        let weld = by_type(&objs, "pmef:Weld")[0];
        assert_eq!(weld["connects"], json!([id(10), id(30)]));
        assert_eq!(weld["ports"][0]["coordinate"], json!([1000.0, 0.0, 0.0]));

        let pump = by_type(&objs, "pmef:Pump")[0];
        assert_eq!(pump["equipmentBasic"]["tagNumber"], "P-100");
        let nz = &pump["nozzles"][0];
        assert_eq!(nz["coordinate"], json!([1305.0, 305.0, 0.0]));
        assert_eq!(nz["direction"], json!([0.0, -1.0, 0.0]));
        assert_eq!(
            nz["connectedLineId"], line["@id"],
            "the line of the joined part"
        );
        assert_eq!(
            pump["customAttributes"]["p3d:placement"],
            "1 0 0 1305 0 1 0 800 0 0 1 0 0 0 0 1"
        );

        let col = by_type(&objs, "pmef:SteelMember")[0];
        assert_eq!(col["memberType"], "COLUMN");
        assert_eq!(col["profileId"], "EN:HEA200");
        assert_eq!(col["material"]["grade"], "S235JR");

        let rels = by_type(&objs, "pmef:IsConnectedTo");
        assert_eq!(rels.len(), 2, "pipe–elbow and elbow–pump");
        assert!(rels
            .iter()
            .any(|r| r["sourceId"] == id(30) && r["targetId"] == id(50)));
        assert_eq!(stats.unmapped.get("StructureGrating"), Some(&1));
        assert_eq!(stats.ports_unplaced, 0);
    }

    #[test]
    fn without_geometry_keeps_the_topology() {
        let p = PdsProject::read(FIXTURE).unwrap();
        let (objs, stats) = to_pmef(
            &p,
            &PdsOptions {
                project_code: "syn".into(),
            },
            &GeometryIndex::new(),
        );
        assert!(by_type(&objs, "pmef:Pipe")[0].get("ports").is_none());
        assert_eq!(
            by_type(&objs, "pmef:Weld")[0]["connects"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(by_type(&objs, "pmef:IsConnectedTo").len(), 2);
        assert!(
            by_type(&objs, "pmef:Pump")[0].get("nozzles").is_none(),
            "a nozzle needs a direction"
        );
        assert!(stats.ports_unplaced > 0);
        let ids: BTreeSet<&str> = objs.iter().map(|o| o["@id"].as_str().unwrap()).collect();
        assert_eq!(ids.len(), objs.len(), "unique ids");
    }

    #[test]
    fn nps_to_dn() {
        assert_eq!(nominal_dn(8.0, Some("in")), 200.0);
        assert_eq!(nominal_dn(0.5, Some("in")), 15.0);
        assert_eq!(nominal_dn(100.0, Some("mm")), 100.0);
    }
}
