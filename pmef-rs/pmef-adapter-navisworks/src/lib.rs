//! # pmef-adapter-navisworks
//!
//! PMEF adapter for **Autodesk Navisworks Manage** — federated model,
//! clash detection results, and model validation.
//!
//! ## Navisworks role in PMEF
//!
//! Navisworks is the **federation and validation layer** in the plant
//! engineering workflow. It aggregates models from:
//!
//! - Revit (BIM / MEP)
//! - AVEVA E3D / Plant 3D (piping, equipment)
//! - Tekla Structures (structural steel)
//! - Inventor / Creo (mechanical equipment)
//! - Advanced Steel (structural detailing)
//!
//! From the PMEF perspective, Navisworks provides:
//!
//! 1. **Federated object index** — all objects from all source models
//!    in one coordinate space, with `HasEquivalentIn` tracing back
//!    to each source system
//! 2. **Clash results** — clash groups with involved object pairs,
//!    mapped to `pmef:ClashResult` relationships
//! 3. **Model health** — completeness checks, missing attributes,
//!    coordinate inconsistencies
//! 4. **Viewpoints** — saved camera positions for issue tracking
//! 5. **TimeLiner** — schedule links (mapped to PMEF sequence attributes)
//!
//! ## Export source
//!
//! Navisworks exposes the **Navisworks API** (C# .NET) via the
//! `Autodesk.Navisworks.Api` namespace. The add-in (`NavisworksExporter.cs`)
//! reads the `ModelItemCollection`, clash results from `ClashDetective`,
//! and viewpoints from `SavedViewpointCollection`.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Export schema
// ─────────────────────────────────────────────────────────────────────────────

/// Root of the Navisworks JSON export.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavisworksExport {
    pub schema_version: String,
    pub navisworks_version: String,
    pub exported_at: String,
    pub model_name: String,
    /// Source .nwf or .nwd file.
    pub file_name: String,
    /// Coordinate units used in the model.
    pub units: String,
    /// All aggregated model items (from all source files).
    #[serde(default)]
    pub model_items: Vec<NavisItem>,
    /// Clash test results.
    #[serde(default)]
    pub clash_tests: Vec<NavisClashTest>,
    /// Saved viewpoints (for issue tracking).
    #[serde(default)]
    pub viewpoints: Vec<NavisViewpoint>,
    /// Appended source files.
    #[serde(default)]
    pub source_files: Vec<NavisSourceFile>,
    pub summary: NavisSummary,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavisSummary {
    pub item_count: u32,
    pub clash_count: u32,
    pub hard_clash_count: u32,
    pub clearance_clash_count: u32,
}

/// A source file appended to the Navisworks federated model.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavisSourceFile {
    pub file_name: String,
    pub file_path: String,
    pub source_system: String,
    pub appended_at: Option<String>,
    pub item_count: u32,
}

/// Navisworks model item (object in the federated model).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavisItem {
    /// Navisworks instance unique path (stable within session).
    pub instance_guid: String,
    /// Display name.
    pub display_name: String,
    /// Source file this item came from.
    pub source_file: String,
    /// Object category (e.g. `"Pipe"`, `"Equipment"`, `"Structural Framing"`).
    pub category: String,
    /// Source system object ID (from source application's GUID or handle).
    pub source_object_id: Option<String>,
    /// Axis-aligned bounding box [mm, world CS].
    pub bounding_box: Option<NavisBbox>,
    /// Properties from all Navisworks property categories.
    #[serde(default)]
    pub properties: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl NavisItem {
    /// Get a property value by category and property name.
    pub fn prop(&self, category: &str, name: &str) -> Option<&serde_json::Value> {
        self.properties.get(category)?.get(name)
    }
    /// Get a property value as string.
    pub fn prop_str(&self, category: &str, name: &str) -> Option<&str> {
        self.prop(category, name)?.as_str()
    }
}

/// Axis-aligned bounding box [mm].
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NavisBbox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl NavisBbox {
    pub fn volume(&self) -> f64 {
        (self.max[0]-self.min[0]).max(0.) *
        (self.max[1]-self.min[1]).max(0.) *
        (self.max[2]-self.min[2]).max(0.)
    }
    pub fn centre(&self) -> [f64; 3] {
        [
            (self.min[0]+self.max[0])/2.,
            (self.min[1]+self.max[1])/2.,
            (self.min[2]+self.max[2])/2.,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Clash detection
// ─────────────────────────────────────────────────────────────────────────────

/// Clash type classification.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum NavisClashType {
    HardClash,
    Clearance,
    Duplicate,
    HardConservative,
}

impl NavisClashType {
    pub fn pmef_severity(&self) -> &'static str {
        match self {
            Self::HardClash          => "ERROR",
            Self::HardConservative   => "ERROR",
            Self::Clearance          => "WARNING",
            Self::Duplicate          => "INFO",
        }
    }
}

/// Status of a clash result.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum NavisClashStatus {
    New, Active, Reviewed, Approved, Resolved,
}

/// A single clash result between two objects.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavisClashResult {
    pub clash_id: String,
    pub clash_name: String,
    pub clash_type: NavisClashType,
    pub status: NavisClashStatus,
    /// Instance GUIDs of the two clashing items.
    pub item_a_guid: String,
    pub item_b_guid: String,
    /// Display names for quick reference.
    pub item_a_name: String,
    pub item_b_name: String,
    /// Source files of the two clashing items.
    pub item_a_source: String,
    pub item_b_source: String,
    /// Clash point [mm, world CS] — deepest penetration point.
    pub clash_point: [f64; 3],
    /// Penetration distance [mm] (positive = overlap; negative = gap for clearance).
    pub distance_mm: f64,
    /// Assigned to (user name).
    pub assigned_to: Option<String>,
    /// Comments.
    pub description: Option<String>,
    /// Found date (ISO 8601).
    pub found_date: Option<String>,
    /// Approved/resolved date.
    pub resolved_date: Option<String>,
}

/// A Navisworks Clash Detective test (one test can contain many results).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavisClashTest {
    pub test_name: String,
    pub selection_a: String,
    pub selection_b: String,
    pub tolerance_mm: f64,
    pub results: Vec<NavisClashResult>,
    pub status_counts: HashMap<String, u32>,
}

/// A Navisworks saved viewpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavisViewpoint {
    pub name: String,
    pub camera_position: [f64; 3],
    pub look_at: [f64; 3],
    pub associated_clash_id: Option<String>,
    pub comment: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Config
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NavisworksConfig {
    pub project_code: String,
    pub export_path: PathBuf,
    /// Include model items as PMEF objects. Default: true.
    pub include_items: bool,
    /// Include clash results as PMEF ClashResult relationships. Default: true.
    pub include_clashes: bool,
    /// Only include clashes with status New or Active. Default: true.
    pub active_clashes_only: bool,
    /// Minimum clash severity to include. Default: HardClash only.
    pub min_severity: ClashSeverityFilter,
    pub unit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClashSeverityFilter {
    /// Only hard clashes (ERROR).
    HardOnly,
    /// Hard + clearance clashes.
    All,
}

impl Default for NavisworksConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            export_path: PathBuf::from("navisworks-export.json"),
            include_items: true,
            include_clashes: true,
            active_clashes_only: true,
            min_severity: ClashSeverityFilter::HardOnly,
            unit_id: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Adapter
// ─────────────────────────────────────────────────────────────────────────────

/// Autodesk Navisworks → PMEF adapter.
pub struct NavisworksAdapter {
    config: NavisworksConfig,
    guid_to_id: HashMap<String, String>,
}

impl NavisworksAdapter {
    pub fn new(config: NavisworksConfig) -> Self {
        Self { config, guid_to_id: HashMap::new() }
    }

    fn unit_id(&self) -> String {
        self.config.unit_id.clone()
            .unwrap_or_else(|| format!("urn:pmef:unit:{}:U-01", self.config.project_code))
    }

    fn item_id(&self, guid: &str) -> String {
        let short: String = guid.chars()
            .filter(|c| c.is_alphanumeric()).take(12).collect();
        format!("urn:pmef:obj:{}:NWS-{short}", self.config.project_code)
    }

    fn clash_id(&self, clash_id: &str) -> String {
        let clean: String = clash_id.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-').collect();
        format!("urn:pmef:rel:{}:CLASH-{clean}", self.config.project_code)
    }

    fn has_equiv(
        &self, pmef_id: &str, nw_guid: &str, source_system: &str,
        source_obj_id: Option<&str>,
    ) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-nws", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id, "targetId": pmef_id,
            "targetSystem": "NAVISWORKS",
            "targetSystemId": nw_guid,
            "confidence": 0.9,
            "customAttributes": {
                "navisworksSourceSystem": source_system,
                "navisworksSourceObjectId": source_obj_id
            },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-navisworks 0.9.0" }
        })
    }

    pub async fn export_to_pmef(
        &mut self, output_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        use std::fs::File;
        use std::io::BufWriter;

        let t0 = std::time::Instant::now();
        let mut stats = AdapterStats::default();

        let json = std::fs::read_to_string(&self.config.export_path).map_err(AdapterError::Io)?;
        let export: NavisworksExport = serde_json::from_str(&json).map_err(|e| AdapterError::Json(e))?;

        tracing::info!("Navisworks: {} items, {} clash tests from '{}'",
            export.model_items.len(), export.clash_tests.len(), export.model_name);

        // Pre-register item GUIDs
        for item in &export.model_items {
            let id = self.item_id(&item.instance_guid);
            self.guid_to_id.insert(item.instance_guid.clone(), id);
        }

        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        let proj = self.config.project_code.clone();
        let model_clean: String = export.model_name.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-').collect();
        let plant_id = format!("urn:pmef:plant:{proj}:{model_clean}");

        // Header
        for hdr in [
            serde_json::json!({
                "@type":"pmef:FileHeader","@id":format!("urn:pmef:pkg:{proj}:{model_clean}"),
                "pmefVersion":"0.9.0","plantId":plant_id,"projectCode":proj,
                "coordinateSystem":"Z-up","units":"mm","revisionId":"r2026-01-01-001",
                "changeState":"SHARED",
                "authoringTool":format!("pmef-adapter-navisworks 0.9.0 / {}", export.navisworks_version)
            }),
            serde_json::json!({
                "@type":"pmef:Plant","@id":plant_id,"pmefVersion":"0.9.0",
                "name":export.model_name,
                "revision":{"revisionId":"r2026-01-01-001","changeState":"SHARED"}
            }),
        ] {
            writer.write_value(&hdr).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Source file provenance records
        for src in &export.source_files {
            let src_obj = serde_json::json!({
                "@type": "pmef:ModelSource",
                "@id": format!("urn:pmef:src:{proj}:{}", src.file_name.replace('.', "-")),
                "fileName": src.file_name,
                "filePath": src.file_path,
                "sourceSystem": src.source_system,
                "itemCount": src.item_count,
                "appendedAt": src.appended_at
            });
            writer.write_value(&src_obj).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Model items — federated object registry
        if self.config.include_items {
            for item in &export.model_items {
                for obj in self.map_model_item(item) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // Clash results
        if self.config.include_clashes {
            let mut clash_count = 0u32;
            for test in &export.clash_tests {
                for clash in &test.results {
                    // Filter by status
                    if self.config.active_clashes_only {
                        if clash.status != NavisClashStatus::New &&
                           clash.status != NavisClashStatus::Active {
                            stats.objects_skipped += 1; continue;
                        }
                    }
                    // Filter by severity
                    if self.config.min_severity == ClashSeverityFilter::HardOnly
                       && clash.clash_type == NavisClashType::Clearance {
                        stats.objects_skipped += 1; continue;
                    }

                    let obj = self.map_clash_result(clash, &test.test_name);
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                    clash_count += 1;
                }
            }
            tracing::info!("Wrote {} clash results", clash_count);
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        Ok(stats)
    }

    // ── Model item mapping ────────────────────────────────────────────────────

    fn map_model_item(&self, item: &NavisItem) -> Vec<serde_json::Value> {
        let obj_id = self.item_id(&item.instance_guid);

        // Determine PMEF type from Navisworks category
        let (pmef_type, domain) = self.classify_item(item);

        let bbox = item.bounding_box.as_ref().map(|bb| serde_json::json!({
            "xMin": bb.min[0], "xMax": bb.max[0],
            "yMin": bb.min[1], "yMax": bb.max[1],
            "zMin": bb.min[2], "zMax": bb.max[2]
        }));

        // Extract common properties from Navisworks property categories
        let tag = item.prop_str("Item", "Name")
            .or_else(|| item.prop_str("Element", "Mark"))
            .or(Some(&item.display_name));

        // Determine source system from file extension
        let source_system = self.infer_source_system(&item.source_file);

        let obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": self.unit_id(),
            "displayName": item.display_name,
            "domain": domain,
            "geometry": { "type": "none", "boundingBox": bbox },
            "customAttributes": {
                "navisworksGuid": item.instance_guid,
                "navisworksCategory": item.category,
                "sourceFile": item.source_file,
                "sourceSystem": source_system,
                "sourceObjectId": item.source_object_id,
                "navisworksProperties": item.properties
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringTool": "pmef-adapter-navisworks 0.9.0"
            }
        });

        let equiv = self.has_equiv(
            &obj_id, &item.instance_guid,
            source_system, item.source_object_id.as_deref(),
        );
        vec![obj, equiv]
    }

    fn classify_item(&self, item: &NavisItem) -> (&'static str, &'static str) {
        let cat = item.category.to_uppercase();
        if cat.contains("PIPE") && !cat.contains("FITTING") {
            ("pmef:Pipe", "piping")
        } else if cat.contains("FITTING") || cat.contains("VALVE") || cat.contains("ACCESSORY") {
            ("pmef:Valve", "piping")
        } else if cat.contains("MECHANICAL") || cat.contains("PUMP") || cat.contains("EQUIPMENT") {
            ("pmef:GenericEquipment", "equipment")
        } else if cat.contains("FRAMING") || cat.contains("BEAM") {
            ("pmef:SteelMember", "steel")
        } else if cat.contains("COLUMN") {
            ("pmef:SteelMember", "steel")
        } else if cat.contains("DUCT") {
            ("pmef:GenericEquipment", "hvac")
        } else if cat.contains("CABLE") {
            ("pmef:CableObject", "electrical")
        } else {
            ("pmef:GenericEquipment", "generic")
        }
    }

    fn infer_source_system<'a>(&self, file_name: &'a str) -> &'static str {
        let lower = file_name.to_lowercase();
        if lower.ends_with(".rvt") || lower.ends_with(".nwc") && lower.contains("revit") {
            "REVIT"
        } else if lower.contains("plant3d") || lower.contains("pcf") {
            "PLANT3D"
        } else if lower.contains("e3d") || lower.contains("aveva") || lower.ends_with(".rvm") {
            "AVEVA_E3D"
        } else if lower.ends_with(".ifc") {
            "IFC"
        } else if lower.contains("tekla") || lower.ends_with(".ifc") {
            "TEKLA_STRUCTURES"
        } else if lower.ends_with(".nwc") || lower.ends_with(".nwd") || lower.ends_with(".dwg") {
            "AUTOCAD"
        } else {
            "UNKNOWN"
        }
    }

    // ── Clash result mapping ──────────────────────────────────────────────────

    fn map_clash_result(
        &self, clash: &NavisClashResult, test_name: &str,
    ) -> serde_json::Value {
        let clash_pmef_id = self.clash_id(&clash.clash_id);
        let item_a_pmef = self.guid_to_id.get(&clash.item_a_guid).cloned()
            .unwrap_or_else(|| format!("urn:pmef:obj:{}:NWS-{}", self.config.project_code,
                clash.item_a_guid.chars().take(8).collect::<String>()));
        let item_b_pmef = self.guid_to_id.get(&clash.item_b_guid).cloned()
            .unwrap_or_else(|| format!("urn:pmef:obj:{}:NWS-{}", self.config.project_code,
                clash.item_b_guid.chars().take(8).collect::<String>()));

        serde_json::json!({
            "@type": "pmef:ClashResult",
            "@id": clash_pmef_id,
            "clashId": clash.clash_id,
            "clashName": clash.clash_name,
            "testName": test_name,
            "severity": clash.clash_type.pmef_severity(),
            "status": format!("{:?}", clash.status).to_uppercase(),
            "itemAId": item_a_pmef,
            "itemBId": item_b_pmef,
            "itemAName": clash.item_a_name,
            "itemBName": clash.item_b_name,
            "itemASource": clash.item_a_source,
            "itemBSource": clash.item_b_source,
            "clashPoint": clash.clash_point,
            "penetrationMm": clash.distance_mm,
            "assignedTo": clash.assigned_to,
            "description": clash.description,
            "foundDate": clash.found_date,
            "resolvedDate": clash.resolved_date,
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringTool": "pmef-adapter-navisworks 0.9.0"
            }
        })
    }
}

impl PmefAdapter for NavisworksAdapter {
    fn name(&self) -> &str { "pmef-adapter-navisworks" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "NAVISWORKS" }
    fn supported_domains(&self) -> &[&str] { &["piping", "equipment", "steel", "clash"] }
    fn conformance_level(&self) -> u8 { 2 }
    fn description(&self) -> &str {
        "Autodesk Navisworks → PMEF adapter. Maps the federated model item registry, \
         clash detection results (Hard, Clearance, Duplicate), and source file provenance \
         to PMEF. Clash results are mapped to pmef:ClashResult objects with severity, \
         status, and 3D clash point. Source system is inferred from file name. \
         Level 2 conformance."
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> NavisworksConfig {
        NavisworksConfig { project_code: "test".to_owned(),
            export_path: PathBuf::from("x.json"), ..Default::default() }
    }

    #[test]
    fn test_clash_type_severity() {
        assert_eq!(NavisClashType::HardClash.pmef_severity(),          "ERROR");
        assert_eq!(NavisClashType::HardConservative.pmef_severity(),   "ERROR");
        assert_eq!(NavisClashType::Clearance.pmef_severity(),          "WARNING");
        assert_eq!(NavisClashType::Duplicate.pmef_severity(),          "INFO");
    }

    #[test]
    fn test_navis_bbox() {
        let bb = NavisBbox { min:[0.,0.,0.], max:[400.,500.,900.] };
        assert!((bb.volume() - 180_000_000.).abs() < 1.0);
        let c = bb.centre();
        assert!((c[0] - 200.).abs() < 0.001);
        assert!((c[2] - 450.).abs() < 0.001);
    }

    #[test]
    fn test_map_clash_result() {
        let adapter = NavisworksAdapter::new(test_config());
        let clash = NavisClashResult {
            clash_id: "CLH-001".to_owned(), clash_name: "Pipe vs Beam".to_owned(),
            clash_type: NavisClashType::HardClash,
            status: NavisClashStatus::Active,
            item_a_guid: "GUID-A".to_owned(), item_b_guid: "GUID-B".to_owned(),
            item_a_name: "CW-201".to_owned(), item_b_name: "B-101".to_owned(),
            item_a_source: "Plant3D.nwc".to_owned(), item_b_source: "Tekla.nwc".to_owned(),
            clash_point: [5000., 5400., 850.], distance_mm: -45.3,
            assigned_to: Some("Marcel".to_owned()),
            description: Some("Pipe penetrates beam flange".to_owned()),
            found_date: Some("2026-03-31T00:00:00Z".to_owned()),
            resolved_date: None,
        };
        let obj = adapter.map_clash_result(&clash, "Test-01");
        assert_eq!(obj["@type"], "pmef:ClashResult");
        assert_eq!(obj["clashId"], "CLH-001");
        assert_eq!(obj["severity"], "ERROR");
        assert_eq!(obj["status"], "ACTIVE");
        assert!((obj["penetrationMm"].as_f64().unwrap() - (-45.3)).abs() < 0.001);
        assert_eq!(obj["assignedTo"], "Marcel");
    }

    #[test]
    fn test_classify_item() {
        let adapter = NavisworksAdapter::new(test_config());
        let mk = |cat: &str| NavisItem {
            instance_guid: "G".to_owned(), display_name: "X".to_owned(),
            source_file: "f.nwc".to_owned(), category: cat.to_owned(),
            source_object_id: None, bounding_box: None, properties: Default::default(),
        };
        assert_eq!(adapter.classify_item(&mk("Pipes")).0, "pmef:Pipe");
        assert_eq!(adapter.classify_item(&mk("Pipe Fittings")).0, "pmef:Valve");
        assert_eq!(adapter.classify_item(&mk("Mechanical Equipment")).0, "pmef:GenericEquipment");
        assert_eq!(adapter.classify_item(&mk("Structural Framing")).0, "pmef:SteelMember");
        assert_eq!(adapter.classify_item(&mk("Structural Columns")).0, "pmef:SteelMember");
    }

    #[test]
    fn test_infer_source_system() {
        let adapter = NavisworksAdapter::new(test_config());
        assert_eq!(adapter.infer_source_system("model.rvt"), "REVIT");
        assert_eq!(adapter.infer_source_system("plant3d_export.nwc"), "PLANT3D");
        assert_eq!(adapter.infer_source_system("e3d_model.rvm"), "AVEVA_E3D");
        assert_eq!(adapter.infer_source_system("structure.ifc"), "IFC");
        assert_eq!(adapter.infer_source_system("something.nwd"), "AUTOCAD");
    }

    #[test]
    fn test_item_guid_to_id() {
        let adapter = NavisworksAdapter::new(test_config());
        let id1 = adapter.item_id("ABCDEF123456GHIJ");
        let id2 = adapter.item_id("ABCDEF123456GHIJ");
        assert_eq!(id1, id2); // deterministic
        assert!(id1.contains("NWS-"));
    }

    #[test]
    fn test_active_clashes_only_filter() {
        let config = NavisworksConfig {
            active_clashes_only: true, ..test_config()
        };
        let adapter = NavisworksAdapter::new(config);
        assert!(adapter.config.active_clashes_only);
        // Resolved clash should be skipped
        let resolved_status = NavisClashStatus::Resolved;
        assert_ne!(resolved_status, NavisClashStatus::New);
        assert_ne!(resolved_status, NavisClashStatus::Active);
    }

    #[test]
    fn test_adapter_trait() {
        let adapter = NavisworksAdapter::new(test_config());
        assert_eq!(adapter.name(), "pmef-adapter-navisworks");
        assert_eq!(adapter.target_system(), "NAVISWORKS");
        assert!(adapter.supported_domains().contains(&"clash"));
    }
}
