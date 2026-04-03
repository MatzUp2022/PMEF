//! # pmef-adapter-plant3d
//!
//! PMEF adapter for **AutoCAD Plant 3D** — bidirectional piping and equipment exchange.
//!
//! ## Data sources
//!
//! Plant 3D stores data in two places:
//!
//! 1. **DWG model** — 3D geometry + piping routing (PCF/IDF export)
//! 2. **Project Data Store (PDS)** — engineering attributes for equipment,
//!    line data, instrumentation tags (Plant SDK / .NET API)
//!
//! ## Export pipeline
//!
//! ```text
//! AutoCAD Plant 3D
//!   │
//!   ├── PCF Export (per line)     → *.pcf files
//!   │         │                         │
//!   │         │                   pcf::parse_pcf()
//!   │         │                         │
//!   ├── IDF Export (per spool)    → *.idf files
//!   │         │                         │
//!   │         │                   idf::parse_idf()
//!   │         │                         │
//!   └── Plant SDK (C# plugin)     → plant3d-export.json
//!               │                         │
//!               │                  equipment mapper
//!               │                         │
//!               └────────────────→ PMEF NDJSON
//! ```
//!
//! ## Usage — PCF only (no SDK required)
//!
//! ```rust,no_run
//! use pmef_adapter_plant3d::{Plant3DAdapter, Plant3DConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = Plant3DConfig {
//!         project_code: "eaf-2026".to_owned(),
//!         pcf_dir: Some("exports/pcf".into()),
//!         ..Default::default()
//!     };
//!     let adapter = Plant3DAdapter::new(config);
//!     let stats = adapter.export_pcf_dir_to_pmef("output.ndjson").await?;
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod equipment;
pub mod idf;

pub use equipment::{p3d_class_to_pmef, P3dEquipment, P3dNozzle};
pub use idf::{idf_bore_to_mm, idf_coord_to_mm, parse_idf, IdfFile};

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Unit system
// ─────────────────────────────────────────────────────────────────────────────

/// PCF/IDF file unit system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcfUnits { Millimetres, Inches }

impl PcfUnits {
    pub fn to_mm(self, v: f64) -> f64 {
        match self { Self::Millimetres => v, Self::Inches => v * 25.4 }
    }
    /// Gauge pressure in native units → Pa absolute.
    pub fn pressure_to_pa_abs(self, p: f64) -> f64 {
        match self {
            Self::Millimetres => p * 100.0 + 101_325.0, // bar-g → Pa abs (approx)
            Self::Inches => p * 6894.757 + 101_325.0,   // psig → Pa abs
        }
    }
    /// Temperature in native units → K.
    pub fn temp_to_k(self, t: f64) -> f64 {
        match self {
            Self::Millimetres => t + 273.15,             // °C → K
            Self::Inches => (t - 32.0) * 5.0 / 9.0 + 273.15, // °F → K
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the Plant 3D adapter.
#[derive(Debug, Clone)]
pub struct Plant3DConfig {
    /// PMEF project code for @id generation.
    pub project_code: String,
    /// Directory containing PCF files (one per line).
    pub pcf_dir: Option<PathBuf>,
    /// Directory containing IDF files.
    pub idf_dir: Option<PathBuf>,
    /// Path to the Plant SDK JSON export (equipment + line metadata).
    pub sdk_export_path: Option<PathBuf>,
    /// Unit area for isPartOf references.
    pub unit_area: String,
    /// PCF unit override (auto-detected from file if None).
    pub pcf_units_override: Option<PcfUnits>,
    /// Maximum PCF files to process (None = all).
    pub max_pcf_files: Option<usize>,
}

impl Default for Plant3DConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            pcf_dir: None,
            idf_dir: None,
            sdk_export_path: None,
            unit_area: "U-100".to_owned(),
            pcf_units_override: None,
            max_pcf_files: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Material mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map Plant 3D / PCF material identifiers to PMEF material strings.
pub struct MaterialMapper {
    table: HashMap<String, &'static str>,
}

impl Default for MaterialMapper {
    fn default() -> Self {
        let mut t = HashMap::new();
        let entries: &[(&str, &str)] = &[
            ("A106B",        "ASTM A106 Gr. B"),
            ("A106GRB",      "ASTM A106 Gr. B"),
            ("A106GRBSMLS",  "ASTM A106 Gr. B"),
            ("A53B",         "ASTM A53 Gr. B"),
            ("A53GRB",       "ASTM A53 Gr. B"),
            ("A312TP316L",   "ASTM A312 TP316L"),
            ("A312TP304L",   "ASTM A312 TP304L"),
            ("A312316L",     "ASTM A312 TP316L"),
            ("A312304L",     "ASTM A312 TP304L"),
            ("SS316L",       "ASTM A312 TP316L"),
            ("SS304L",       "ASTM A312 TP304L"),
            ("A335P11",      "ASTM A335 Gr. P11"),
            ("A335P22",      "ASTM A335 Gr. P22"),
            ("A335P91",      "ASTM A335 Gr. P91"),
            ("A333GR6",      "ASTM A333 Gr. 6"),
            ("A234WPB",      "ASTM A234 WPB"),
            ("WPBFITTING",   "ASTM A234 WPB"),
            ("A105",         "ASTM A105"),
            ("A105N",        "ASTM A105"),
            ("A216WCB",      "ASTM A216 WCB"),
            ("WCB",          "ASTM A216 WCB"),
            ("P265GH",       "EN 10216-2 P265GH"),
            ("P235GH",       "EN 10216-2 P235GH"),
            ("SA516GR70",    "ASTM A516 Gr. 70"),
            ("CS",           "ASTM A106 Gr. B"),
            ("CARBONSTEEL",  "ASTM A106 Gr. B"),
        ];
        for (k, v) in entries {
            t.insert(k.to_uppercase(), v);
        }
        Self { table: t }
    }
}

impl MaterialMapper {
    pub fn map(&self, mat: &str) -> &str {
        let key = mat.trim().to_uppercase().replace(['-', ' '], "");
        self.table.get(&key).copied().unwrap_or(mat)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PCF keyword → PMEF type mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Map a PCF keyword + SKEY prefix to PMEF @type and componentClass.
/// Returns (pmef_type, component_class, default_skey).
pub fn pcf_to_pmef_type(keyword: &str, skey: Option<&str>) -> (&'static str, &'static str, &'static str) {
    let skey_prefix = skey.and_then(|s| s.get(..2)).unwrap_or("").to_uppercase();
    let skey_prefix = skey_prefix.as_str();

    match keyword.to_uppercase().as_str() {
        "PIPE"                    => ("pmef:Pipe",       "PIPE",                "PIPW    "),
        "ELBOW"                   => ("pmef:Elbow",      "ELBOW",               "ELBWLR90"),
        "TEE"                     => ("pmef:Tee",        "TEE",                 "TEBWEQUL"),
        "CROSS"                   => ("pmef:Tee",        "TEE",                 "CRBWEQUL"),
        "REDUCER-CONCENTRIC"      => ("pmef:Reducer",    "REDUCER_CONCENTRIC",  "RDCWCNCN"),
        "REDUCER-ECCENTRIC"       => ("pmef:Reducer",    "REDUCER_ECCENTRIC",   "RDCWECCT"),
        "FLANGE" => match skey_prefix {
            "BL" | "FB"           => ("pmef:Flange",     "BLIND_FLANGE",        "FLBLRF  "),
            "SO"                  => ("pmef:Flange",     "FLANGE",              "FLSORF  "),
            "SW"                  => ("pmef:Flange",     "FLANGE",              "FLSWRF  "),
            "LJ"                  => ("pmef:Flange",     "FLANGE",              "FLLJ    "),
            "TH"                  => ("pmef:Flange",     "FLANGE",              "FLTHRF  "),
            _                     => ("pmef:Flange",     "FLANGE",              "FLWNRF  "),
        },
        "FLANGE-BLIND"            => ("pmef:Flange",     "BLIND_FLANGE",        "FLBLRF  "),
        "VALVE" => match skey_prefix {
            "GT"                  => ("pmef:Valve",      "VALVE_GATE",          "GTBWFLFL"),
            "GL"                  => ("pmef:Valve",      "VALVE_GLOBE",         "GLBWFLFL"),
            "BL"                  => ("pmef:Valve",      "VALVE_BALL",          "BLBWFLFL"),
            "BF"                  => ("pmef:Valve",      "VALVE_BUTTERFLY",     "BFBWFLFL"),
            "CK"                  => ("pmef:Valve",      "VALVE_CHECK",         "CKBWFLFL"),
            "CV" | "GL"           => ("pmef:Valve",      "VALVE_CONTROL",       "CVBWFLFL"),
            "SV" | "RV" | "PRV"   => ("pmef:Valve",      "VALVE_RELIEF",        "SVBWFLFL"),
            "NL"                  => ("pmef:Valve",      "VALVE_NEEDLE",        "NLBWFLFL"),
            _                     => ("pmef:Valve",      "VALVE_GATE",          "GTBWFLFL"),
        },
        "INSTRUMENT"              => ("pmef:Valve",      "VALVE_CONTROL",       "CVBWFLFL"),
        "GASKET"                  => ("pmef:Gasket",     "GASKET",              "GKSWRG  "),
        "OLET" | "WELDOLET"      => ("pmef:Olet",       "OLET_WELDOLET",       "WOLW    "),
        "SOCKOLET"                => ("pmef:Olet",       "OLET_SOCKOLET",       "SOLW    "),
        "WELD" | "BUTT-WELD"     => ("pmef:Weld",       "WELD_BUTT",           "WLDW    "),
        "SUPPORT"                 => ("pmef:PipeSupport","PIPE_SUPPORT",        "SUPRW   "),
        "BOLT-SET"                => ("pmef:Gasket",     "BOLT_SET",            "BLTSET  "),
        "SPECTACLE-BLIND"         => ("pmef:Flange",     "SPECTACLE_BLIND",     "SPBLIND "),
        _                         => ("pmef:Pipe",       "PIPE",                "PIPW    "),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PCF parser (line-by-line state machine)
// ─────────────────────────────────────────────────────────────────────────────

/// A parsed PCF component block.
#[derive(Debug, Default)]
pub struct PcfComponent {
    pub keyword: String,
    pub attrs: HashMap<String, String>,
    pub end_points: Vec<[f64; 4]>,
}

impl PcfComponent {
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.get(&k.to_uppercase()).map(|s| s.as_str())
    }
    pub fn skey(&self) -> Option<&str> { self.attr("SKEY") }
    pub fn material(&self) -> Option<&str> {
        self.attr("MATERIAL-IDENTIFIER").or_else(|| self.attr("MATERIAL"))
    }
    pub fn weight(&self) -> Option<f64> {
        self.attr("WEIGHT").and_then(|w| w.parse().ok())
    }
    pub fn tag_number(&self) -> Option<&str> {
        self.attr("ATTRIBUTE0").or_else(|| self.attr("COMPONENT-TAG"))
    }
}

/// Parse a PCF file content.
pub fn parse_pcf(content: &str) -> (PcfUnits, String, Vec<PcfComponent>) {
    let mut units = PcfUnits::Millimetres;
    let mut pipeline_ref = String::new();
    let mut components: Vec<PcfComponent> = Vec::new();
    let mut current: Option<PcfComponent> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') { continue; }

        let (kw, rest) = match line.find(' ') {
            Some(i) => (&line[..i], line[i+1..].trim()),
            None    => (line, ""),
        };
        let kw_upper = kw.to_uppercase();

        if kw_upper == "UNITS-BORE" {
            units = if rest.trim().eq_ignore_ascii_case("INCHES") {
                PcfUnits::Inches
            } else { PcfUnits::Millimetres };
            continue;
        }
        if kw_upper == "PIPELINE-REFERENCE" {
            pipeline_ref = rest.to_owned();
            continue;
        }
        if kw_upper == "END-POINT" {
            let pts: Vec<f64> = rest.split_whitespace()
                .filter_map(|p| p.parse().ok()).collect();
            if pts.len() >= 4 {
                if let Some(ref mut c) = current {
                    c.end_points.push([pts[0], pts[1], pts[2], pts[3]]);
                }
            }
            continue;
        }

        // Component-starting keywords
        let is_comp = matches!(kw_upper.as_str(),
            "PIPE" | "ELBOW" | "TEE" | "CROSS" | "REDUCER-CONCENTRIC" |
            "REDUCER-ECCENTRIC" | "FLANGE" | "FLANGE-BLIND" | "VALVE" |
            "INSTRUMENT" | "GASKET" | "OLET" | "WELDOLET" | "SOCKOLET" |
            "WELD" | "BUTT-WELD" | "SUPPORT" | "BOLT-SET" | "SPECTACLE-BLIND"
        );

        if is_comp {
            if let Some(prev) = current.take() { components.push(prev); }
            let mut c = PcfComponent::default();
            c.keyword = kw_upper;
            current = Some(c);
        } else if let Some(ref mut c) = current {
            c.attrs.insert(kw_upper, rest.to_owned());
        }
    }
    if let Some(last) = current { components.push(last); }
    (units, pipeline_ref, components)
}

// ─────────────────────────────────────────────────────────────────────────────
// Coordinate key for topology resolution
// ─────────────────────────────────────────────────────────────────────────────

pub type CoordKey = (i64, i64, i64);

pub fn coord_key_mm(x_mm: f64, y_mm: f64, z_mm: f64) -> CoordKey {
    ((x_mm * 100.0).round() as i64,
     (y_mm * 100.0).round() as i64,
     (z_mm * 100.0).round() as i64)
}

// ─────────────────────────────────────────────────────────────────────────────
// Plant 3D Adapter
// ─────────────────────────────────────────────────────────────────────────────

/// AutoCAD Plant 3D → PMEF adapter.
pub struct Plant3DAdapter {
    config: Plant3DConfig,
    material_mapper: MaterialMapper,
    /// handle → PMEF @id
    handle_to_id: HashMap<String, String>,
    /// coord_key → (comp_idx, port_idx) per file
    coord_index: HashMap<CoordKey, String>,
    counters: HashMap<String, usize>,
}

impl Plant3DAdapter {
    pub fn new(config: Plant3DConfig) -> Self {
        Self {
            config,
            material_mapper: MaterialMapper::default(),
            handle_to_id: HashMap::new(),
            coord_index: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    // ── @id helpers ───────────────────────────────────────────────────────────

    fn make_id(&mut self, domain: &str, local: &str) -> String {
        let clean: String = local.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        format!("urn:pmef:{domain}:{}:{clean}", self.config.project_code)
    }

    fn component_id(&mut self, line_clean: &str, kw_short: &str) -> String {
        let key = format!("{line_clean}-{kw_short}");
        let n = self.counters.entry(key.clone()).or_insert(0);
        *n += 1;
        format!("urn:pmef:obj:{}:{line_clean}-{kw_short}-{:03}", self.config.project_code, n)
    }

    fn make_has_equivalent_in(&self, pmef_id: &str, handle: &str) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-p3d", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id, "targetId": pmef_id,
            "targetSystem": "PLANT3D",
            "targetSystemId": handle,
            "mappingType": "EXACT",
            "derivedBy": "ADAPTER_IMPORT",
            "confidence": 1.0,
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-plant3d 0.9.0" }
        })
    }

    // ── PCF export pipeline ───────────────────────────────────────────────────

    /// Export a single PCF string to PMEF NDJSON objects.
    pub fn export_pcf_to_pmef(&mut self, pcf_content: &str) -> Result<String, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        let mut buf = Vec::new();
        let mut writer = NdjsonWriter::new(&mut buf, WriterConfig { canonical_key_order: false, ..Default::default() });

        let (units, pipeline_ref, components) = parse_pcf(pcf_content);
        let units = self.config.pcf_units_override.unwrap_or(units);

        // Build coordinate index for topology
        let mut coord_idx: HashMap<CoordKey, (usize, usize)> = HashMap::new();
        for (ci, comp) in components.iter().enumerate() {
            for (pi, ep) in comp.end_points.iter().enumerate() {
                let (x, y, z) = (units.to_mm(ep[0]), units.to_mm(ep[1]), units.to_mm(ep[2]));
                coord_idx.insert(coord_key_mm(x, y, z), (ci, pi));
            }
        }

        let line_clean: String = pipeline_ref.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        let line_id = self.make_id("line", &line_clean);
        let seg_id  = format!("urn:pmef:seg:{}:{line_clean}-S1", self.config.project_code);
        let unit_id = format!("urn:pmef:unit:{}:{}", self.config.project_code, self.config.unit_area);

        // PipingNetworkSystem
        let pns = serde_json::json!({
            "@type": "pmef:PipingNetworkSystem",
            "@id": line_id,
            "pmefVersion": "0.9.0",
            "lineNumber": pipeline_ref,
            "isPartOf": unit_id,
            "segments": [seg_id],
            "revision": { "revisionId":"r2026-01-01-001","changeState":"WIP",
                          "authoringTool":"pmef-adapter-plant3d 0.9.0" }
        });
        writer.write_value(&pns).map_err(|e| AdapterError::Json(e.into()))?;

        // Pre-generate component IDs
        let mut comp_ids: Vec<String> = Vec::new();
        let mut kw_counters: HashMap<String, usize> = HashMap::new();
        for comp in &components {
            let (_, cls, _) = pcf_to_pmef_type(&comp.keyword, comp.skey());
            let short = &cls[..4.min(cls.len())];
            let n = kw_counters.entry(short.to_owned()).or_insert(0);
            *n += 1;
            let id = format!("urn:pmef:obj:{}:{line_clean}-{short}-{:03}", self.config.project_code, n);
            comp_ids.push(id);
        }

        // PipingSegment
        let seg = serde_json::json!({
            "@type": "pmef:PipingSegment",
            "@id": seg_id,
            "isPartOf": line_id,
            "segmentNumber": 1,
            "components": comp_ids,
            "revision": { "revisionId":"r2026-01-01-001","changeState":"WIP" }
        });
        writer.write_value(&seg).map_err(|e| AdapterError::Json(e.into()))?;

        // Map each component
        for (ci, comp) in components.iter().enumerate() {
            let obj_id = comp_ids[ci].clone();
            let (pmef_type, cls, skey_default) = pcf_to_pmef_type(&comp.keyword, comp.skey());
            let skey = comp.skey().unwrap_or(skey_default);
            let material = comp.material()
                .map(|m| self.material_mapper.map(m))
                .unwrap_or("ASTM A106 Gr. B");

            // Build ports
            let mut ports = Vec::new();
            for (pi, ep) in comp.end_points.iter().enumerate() {
                let x = units.to_mm(ep[0]);
                let y = units.to_mm(ep[1]);
                let z = units.to_mm(ep[2]);
                let bore = units.to_mm(ep[3]);

                // Topology: find connected component by coordinate
                let connected = coord_idx.iter()
                    .find(|(&ck, &(other_ci, _))| other_ci != ci && ck == coord_key_mm(x, y, z))
                    .map(|(_, &(other_ci, _))| comp_ids[other_ci].clone());

                let mut port = serde_json::json!({
                    "portId": format!("P{}", pi + 1),
                    "coordinate": [x, y, z],
                    "nominalDiameter": bore,
                    "endType": "BW"
                });
                if let Some(ct) = connected {
                    port["connectedTo"] = serde_json::Value::String(ct);
                }
                ports.push(port);
            }

            let mut obj = serde_json::json!({
                "@type": pmef_type,
                "@id": obj_id,
                "pmefVersion": "0.9.0",
                "isPartOf": seg_id,
                "componentSpec": {
                    "componentClass": cls,
                    "skey": skey,
                    "weight": comp.weight()
                },
                "ports": ports,
                "catalogRef": {
                    "vendorMappings": [{
                        "vendorSystem": "PLANT3D",
                        "vendorId": comp.attr("ATTRIBUTE0").unwrap_or("")
                    }]
                },
                "revision": { "revisionId":"r2026-01-01-001","changeState":"WIP",
                              "authoringTool":"pmef-adapter-plant3d 0.9.0" }
            });

            // Type-specific fields
            match pmef_type {
                "pmef:Pipe" => {
                    if comp.end_points.len() >= 2 {
                        let p1 = &comp.end_points[0];
                        let p2 = &comp.end_points[1];
                        let dx = units.to_mm(p2[0] - p1[0]);
                        let dy = units.to_mm(p2[1] - p1[1]);
                        let dz = units.to_mm(p2[2] - p1[2]);
                        obj["pipeLength"] = serde_json::Value::from((dx*dx+dy*dy+dz*dz).sqrt());
                    }
                }
                "pmef:Elbow" => {
                    let angle = comp.attr("ANGLE").and_then(|a| a.parse::<f64>().ok()).unwrap_or(90.0);
                    obj["angle"]  = serde_json::Value::from(angle);
                    obj["radius"] = serde_json::Value::from(
                        if skey.contains("LR") { "LONG_RADIUS" } else if skey.contains("SR") { "SHORT_RADIUS" } else { "LONG_RADIUS" }
                    );
                }
                "pmef:Reducer" => {
                    if comp.end_points.len() >= 2 {
                        obj["reducerType"]   = serde_json::Value::from(if cls.contains("ECC") { "ECCENTRIC" } else { "CONCENTRIC" });
                        obj["largeDiameter"] = serde_json::Value::from(units.to_mm(comp.end_points[0][3]));
                        obj["smallDiameter"] = serde_json::Value::from(units.to_mm(comp.end_points[1][3]));
                    }
                }
                "pmef:Flange" => {
                    obj["flangeType"] = serde_json::Value::from(
                        if cls.contains("BLIND") { "BLIND" }
                        else if skey.starts_with("FLSO") { "SLIP_ON" }
                        else if skey.starts_with("FLSW") { "SOCKET_WELD" }
                        else { "WELD_NECK" }
                    );
                    obj["rating"] = serde_json::Value::from("ANSI-150");
                    obj["facing"] = serde_json::Value::from("RF");
                }
                "pmef:Gasket" => {
                    obj["gasketType"]     = serde_json::Value::from("SPIRAL_WOUND");
                    obj["gasketMaterial"] = serde_json::Value::from("SS316-FLEXITE");
                }
                "pmef:Valve" => {
                    if let Some(tag) = comp.tag_number() {
                        obj["tagNumber"] = serde_json::Value::from(tag);
                    }
                }
                "pmef:PipeSupport" => {
                    obj["supportsMark"] = serde_json::Value::from(
                        comp.attr("SUPPORT-MARK").unwrap_or("S1")
                    );
                    obj["supportSpec"] = serde_json::json!({
                        "supportType": "RESTING",
                        "attachmentType": "WELDED"
                    });
                }
                _ => {}
            }

            writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;

            // HasEquivalentIn (use handle from ATTRIBUTE0 or sequential)
            let handle = comp.attr("ATTRIBUTE0").unwrap_or(&format!("{ci}")).to_owned();
            let equiv = self.make_has_equivalent_in(&obj_id, &handle);
            writer.write_value(&equiv).map_err(|e| AdapterError::Json(e.into()))?;
        }

        writer.flush().map_err(|e| AdapterError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        Ok(String::from_utf8(buf).unwrap_or_default())
    }

    /// Export all PCF files in `config.pcf_dir` to a single PMEF NDJSON file.
    pub async fn export_pcf_dir_to_pmef(
        &mut self, output_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        use std::fs::File;
        use std::io::BufWriter;

        let t0 = std::time::Instant::now();
        let mut stats = AdapterStats::default();

        let pcf_dir = self.config.pcf_dir.clone()
            .ok_or_else(|| AdapterError::Other("pcf_dir not configured".to_owned()))?;

        let mut pcf_files: Vec<std::path::PathBuf> = std::fs::read_dir(&pcf_dir)
            .map_err(AdapterError::Io)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("pcf")).unwrap_or(false))
            .collect();
        pcf_files.sort();
        if let Some(max) = self.config.max_pcf_files {
            pcf_files.truncate(max);
        }

        tracing::info!("Processing {} PCF files from {}", pcf_files.len(), pcf_dir.display());

        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        // FileHeader + Plant + Unit
        let pc = &self.config.project_code.clone();
        for hdr in self.make_header_objects(pc) {
            writer.write_value(&hdr).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Equipment (from SDK export if available)
        if let Some(ref sdk_path) = self.config.sdk_export_path.clone() {
            match std::fs::read_to_string(sdk_path) {
                Ok(json) => {
                    match serde_json::from_str::<Vec<P3dEquipment>>(&json) {
                        Ok(equipment) => {
                            for equip in &equipment {
                                for obj in self.map_equipment(equip) {
                                    writer.write_value(&obj)
                                        .map_err(|e| AdapterError::Json(e.into()))?;
                                    stats.objects_ok += 1;
                                }
                            }
                            tracing::info!("Mapped {} equipment objects", equipment.len());
                        }
                        Err(e) => tracing::warn!("SDK JSON parse error: {e}"),
                    }
                }
                Err(e) => tracing::warn!("Cannot read SDK export: {e}"),
            }
        }

        // PCF files
        for pcf_path in &pcf_files {
            match std::fs::read_to_string(pcf_path) {
                Ok(content) => {
                    match self.export_pcf_to_pmef(&content) {
                        Ok(ndjson) => {
                            for line in ndjson.lines() {
                                if !line.trim().is_empty() {
                                    let val: serde_json::Value = serde_json::from_str(line)
                                        .map_err(|e| AdapterError::Json(e))?;
                                    writer.write_value(&val)
                                        .map_err(|e| AdapterError::Json(e.into()))?;
                                    stats.objects_ok += 1;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to process {}: {e}", pcf_path.display());
                            stats.objects_failed += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Cannot read {}: {e}", pcf_path.display());
                    stats.objects_failed += 1;
                }
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        tracing::info!(
            "Plant3D export: {} ok, {} failed in {}ms",
            stats.objects_ok, stats.objects_failed, stats.duration_ms
        );
        Ok(stats)
    }

    // ── Header objects ────────────────────────────────────────────────────────

    fn make_header_objects(&self, proj: &str) -> Vec<serde_json::Value> {
        let plant_id = format!("urn:pmef:plant:{proj}:{proj}");
        let unit_id  = format!("urn:pmef:unit:{proj}:{}", self.config.unit_area);
        vec![
            serde_json::json!({
                "@type": "pmef:FileHeader",
                "@id": format!("urn:pmef:pkg:{proj}:{proj}"),
                "pmefVersion": "0.9.0",
                "plantId": plant_id,
                "projectCode": proj,
                "coordinateSystem": "Z-up",
                "units": "mm",
                "revisionId": "r2026-01-01-001",
                "changeState": "WIP",
                "authoringTool": "pmef-adapter-plant3d 0.9.0"
            }),
            serde_json::json!({
                "@type": "pmef:Plant",
                "@id": plant_id,
                "pmefVersion": "0.9.0",
                "name": proj,
                "revision": { "revisionId":"r2026-01-01-001","changeState":"WIP" }
            }),
            serde_json::json!({
                "@type": "pmef:Unit",
                "@id": unit_id,
                "pmefVersion": "0.9.0",
                "name": self.config.unit_area,
                "isPartOf": plant_id,
                "revision": { "revisionId":"r2026-01-01-001","changeState":"WIP" }
            }),
        ]
    }

    // ── Equipment mapping ─────────────────────────────────────────────────────

    fn map_equipment(&self, equip: &P3dEquipment) -> Vec<serde_json::Value> {
        let (pmef_type, equip_class) = p3d_class_to_pmef(&equip.equipment_class);
        let obj_id = format!("urn:pmef:obj:{}:{}", self.config.project_code,
            equip.tag_number.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-'|'_')).collect::<String>());
        let unit_id = format!("urn:pmef:unit:{}:{}", self.config.project_code, self.config.unit_area);

        let nozzles: Vec<serde_json::Value> = equip.nozzles.iter().map(|noz| {
            serde_json::json!({
                "nozzleId": noz.nozzle_number,
                "nozzleMark": noz.nozzle_number,
                "service": noz.service,
                "nominalDiameter": noz.dn_mm(),
                "flangeRating": noz.flange_rating.as_deref()
                    .map(|r| format!("ANSI-{r}")),
                "facingType": noz.facing_type.as_deref().unwrap_or("RF"),
                "coordinate": noz.position_mm,
                "direction": noz.direction,
                "connectedLineId": noz.connected_line_tag.as_ref()
                    .map(|t| format!("urn:pmef:line:{}:{}",
                        self.config.project_code,
                        t.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-'|'_')).collect::<String>()))
            })
        }).collect();

        let bbox = equip.bbox_min_mm.zip(equip.bbox_max_mm).map(|(mn, mx)| {
            serde_json::json!({
                "xMin": mn[0], "xMax": mx[0],
                "yMin": mn[1], "yMax": mx[1],
                "zMin": mn[2], "zMax": mx[2]
            })
        });

        let obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": unit_id,
            "equipmentBasic": {
                "tagNumber": equip.tag_number,
                "equipmentClass": equip_class,
                "serviceDescription": equip.description,
                "designCode": equip.design_code,
                "manufacturer": equip.manufacturer,
                "model": equip.model
            },
            "nozzles": nozzles,
            "geometry": {
                "type": "none",
                "boundingBox": bbox
            },
            "customAttributes": {
                "plant3dHandle": equip.handle,
                "pidTag": equip.pid_tag,
                "designPressure_Pa": equip.design_pressure_pa(),
                "designTemperature_K": equip.design_temperature_k(),
                "operatingPressure_Pa": equip.operating_pressure_psig
                    .map(|p| p * 6894.757 + 101_325.0),
                "operatingTemperature_K": equip.operating_temperature_f
                    .map(|f| (f - 32.0) * 5.0 / 9.0 + 273.15),
                "material": equip.material.as_deref()
                    .map(|m| self.material_mapper.map(m)),
                "weightKg": equip.weight_kg(),
                "motorPower_W": equip.motor_power_kw().map(|kw| kw * 1000.0),
                "designFlow_m3h": equip.design_flow_m3h(),
                "designHead_m": equip.design_head_m(),
                "volume_m3": equip.volume_m3(),
                "heatDuty_W": equip.heat_duty_w(),
                "heatTransferArea_m2": equip.heat_transfer_area_m2()
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "WIP",
                "authoringToolObjectId": equip.handle,
                "authoringTool": "pmef-adapter-plant3d 0.9.0"
            }
        });

        vec![obj, self.make_has_equivalent_in(&obj_id, &equip.handle)]
    }
}

impl PmefAdapter for Plant3DAdapter {
    fn name(&self) -> &str { "pmef-adapter-plant3d" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "PLANT3D" }
    fn supported_domains(&self) -> &[&str] { &["piping", "equipment"] }
    fn conformance_level(&self) -> u8 { 2 }
    fn description(&self) -> &str {
        "AutoCAD Plant 3D → PMEF adapter. Reads PCF/IDF files (piping geometry) \
         and Plant SDK JSON export (equipment + line engineering data). \
         Level 2 conformance. Bidirectional via HasEquivalentIn."
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcf_units_mm_conversion() {
        assert!((PcfUnits::Inches.to_mm(1.0) - 25.4).abs() < 1e-9);
        assert!((PcfUnits::Millimetres.to_mm(100.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_pressure_psig_to_pa() {
        // 217.6 psig ≈ 15 barg → 1,601,325 Pa abs
        let pa = PcfUnits::Inches.pressure_to_pa_abs(217.6);
        assert!((pa - 1_601_325.0).abs() < 500.0, "Got {pa}");
    }

    #[test]
    fn test_temp_f_to_k() {
        assert!((PcfUnits::Inches.temp_to_k(32.0) - 273.15).abs() < 0.01);
        assert!((PcfUnits::Inches.temp_to_k(212.0) - 373.15).abs() < 0.01);
    }

    #[test]
    fn test_temp_c_to_k() {
        assert!((PcfUnits::Millimetres.temp_to_k(0.0) - 273.15).abs() < 0.01);
        assert!((PcfUnits::Millimetres.temp_to_k(60.0) - 333.15).abs() < 0.01);
    }

    #[test]
    fn test_pcf_keyword_mapping() {
        let (t, c, _) = pcf_to_pmef_type("PIPE", None);
        assert_eq!(t, "pmef:Pipe"); assert_eq!(c, "PIPE");

        let (t, c, _) = pcf_to_pmef_type("ELBOW", Some("ELBWLR90"));
        assert_eq!(t, "pmef:Elbow"); assert_eq!(c, "ELBOW");

        let (t, c, _) = pcf_to_pmef_type("VALVE", Some("GTBWFLFL"));
        assert_eq!(t, "pmef:Valve"); assert_eq!(c, "VALVE_GATE");

        let (t, c, _) = pcf_to_pmef_type("VALVE", Some("BLBWFLFL"));
        assert_eq!(t, "pmef:Valve"); assert_eq!(c, "VALVE_BALL");

        let (t, c, _) = pcf_to_pmef_type("FLANGE", Some("FLBLRF  "));
        assert_eq!(t, "pmef:Flange"); assert_eq!(c, "BLIND_FLANGE");

        let (t, c, _) = pcf_to_pmef_type("REDUCER-ECCENTRIC", None);
        assert_eq!(t, "pmef:Reducer"); assert_eq!(c, "REDUCER_ECCENTRIC");
    }

    #[test]
    fn test_material_mapper() {
        let m = MaterialMapper::default();
        assert_eq!(m.map("A106B"), "ASTM A106 Gr. B");
        assert_eq!(m.map("SS316L"), "ASTM A312 TP316L");
        assert_eq!(m.map("A234WPB"), "ASTM A234 WPB");
        assert_eq!(m.map("UNKNOWN"), "UNKNOWN"); // passthrough
    }

    #[test]
    fn test_parse_pcf_minimal() {
        let pcf = r#"
UNITS-BORE INCHES
PIPELINE-REFERENCE 8"-CW-201-A1A2
PIPE
    SKEY PIPW
    MATERIAL-IDENTIFIER A106B
    END-POINT 0.0 0.0 33.46 7.981
    END-POINT 98.43 0.0 33.46 7.981
ELBOW
    SKEY ELBWLR90
    END-POINT 98.43 0.0 33.46 7.981
    END-POINT 98.43 0.0 45.47 7.981
"#;
        let (units, pipeline_ref, components) = parse_pcf(pcf);
        assert_eq!(units, PcfUnits::Inches);
        assert_eq!(pipeline_ref, "8\"-CW-201-A1A2");
        assert_eq!(components.len(), 2);
        assert_eq!(components[0].keyword, "PIPE");
        assert_eq!(components[0].skey(), Some("PIPW"));
        assert_eq!(components[0].material(), Some("A106B"));
        assert_eq!(components[0].end_points.len(), 2);
        let bore_mm = units.to_mm(components[0].end_points[0][3]);
        assert!((bore_mm - 202.72).abs() < 0.1);
    }

    #[test]
    fn test_export_pcf_to_pmef() {
        let pcf = r#"
UNITS-BORE MILLIMETERS
PIPELINE-REFERENCE CW-201
PIPE
    SKEY PIPW
    MATERIAL-IDENTIFIER A106B
    END-POINT 0.0 0.0 850.0 219.1
    END-POINT 2500.0 0.0 850.0 219.1
"#;
        let mut adapter = Plant3DAdapter::new(Plant3DConfig {
            project_code: "test".to_owned(), ..Default::default()
        });
        let ndjson = adapter.export_pcf_to_pmef(pcf).unwrap();
        let lines: Vec<&str> = ndjson.lines().filter(|l| !l.is_empty()).collect();
        // PipingNetworkSystem + PipingSegment + Pipe + HasEquivalentIn = 4
        assert!(lines.len() >= 3);
        let pns: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(pns["@type"], "pmef:PipingNetworkSystem");
        assert_eq!(pns["lineNumber"], "CW-201");

        let pipe: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(pipe["@type"], "pmef:Pipe");
        let len = pipe["pipeLength"].as_f64().unwrap();
        assert!((len - 2500.0).abs() < 1.0);
    }

    #[test]
    fn test_coord_key_deterministic() {
        let k1 = coord_key_mm(100.001, 200.0, 850.0);
        let k2 = coord_key_mm(100.001, 200.0, 850.0);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_adapter_trait() {
        let adapter = Plant3DAdapter::new(Plant3DConfig::default());
        assert_eq!(adapter.name(), "pmef-adapter-plant3d");
        assert_eq!(adapter.target_system(), "PLANT3D");
        assert_eq!(adapter.conformance_level(), 2);
        assert!(adapter.supported_domains().contains(&"piping"));
        assert!(adapter.supported_domains().contains(&"equipment"));
    }
}
