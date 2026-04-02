//! # pmef-adapter-advanced-steel
//!
//! PMEF adapter for **Autodesk Advanced Steel** — structural steel detailing.
//!
//! Advanced Steel extends AutoCAD with a full structural steel library:
//! - **Beams, columns, braces** — hot-rolled sections (DIN, AISC, BS, AS)
//! - **Plates, gussets** — flat plate elements with full geometry
//! - **Bolted connections** — bolt groups, endplates, cleats, baseplates
//! - **Welded connections** — weld seams with size/process attributes
//! - **Anchors** — concrete anchors and anchor bolt groups
//! - **Purlins/rails** — cold-formed sections (Zed, Cee)
//! - **Numbering** — member marks, assembly marks, GUID-based identity
//!
//! The DSTV NC file format and the Advanced Steel Detailing API (C# COM)
//! provide two export paths. The C# add-in (`AdvancedSteelExporter.cs`)
//! uses the Autodesk.AdvancedSteeling.Core API directly.

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Export schema
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvSteelExport {
    pub schema_version: String,
    pub advanced_steel_version: String,
    pub exported_at: String,
    pub model_name: String,
    pub drawing_number: Option<String>,
    pub coordinate_unit: String,
    #[serde(default)]
    pub beams: Vec<AdvSteelBeam>,
    #[serde(default)]
    pub plates: Vec<AdvSteelPlate>,
    #[serde(default)]
    pub bolt_patterns: Vec<AdvSteelBoltPattern>,
    #[serde(default)]
    pub weld_seams: Vec<AdvSteelWeldSeam>,
    #[serde(default)]
    pub anchor_patterns: Vec<AdvSteelAnchorPattern>,
    pub summary: AdvSteelSummary,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvSteelSummary {
    pub beam_count: u32,
    pub plate_count: u32,
    pub bolt_pattern_count: u32,
    pub weld_seam_count: u32,
}

/// An Advanced Steel beam/column/brace element.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvSteelBeam {
    /// Internal GUID (stable, used for `HasEquivalentIn`).
    pub handle: String,
    /// Member mark (part mark from numbering, e.g. `"B01"`, `"C01"`).
    pub member_mark: String,
    /// Assembly mark (e.g. `"ASM-001"`).
    pub assembly_mark: Option<String>,
    /// Section profile designation (e.g. `"HEA200"`, `"W12x53"`, `"RHS200x6"`).
    pub section: String,
    /// Profile catalog standard (`"DIN"`, `"AISC"`, `"BS"`, `"EUROPEAN"`).
    pub section_standard: String,
    /// Steel grade (e.g. `"S355JR"`, `"A992"`).
    pub grade: String,
    /// Member type from AS classification.
    pub member_type: AdvSteelMemberType,
    /// Start point [mm, world CS].
    pub start_point: [f64; 3],
    /// End point [mm, world CS].
    pub end_point: [f64; 3],
    /// Length [mm].
    pub length_mm: f64,
    /// Roll angle [degrees].
    pub roll_angle_deg: f64,
    /// Mass [kg].
    pub mass_kg: Option<f64>,
    /// Surface area [m²].
    pub surface_area_m2: Option<f64>,
    /// Finish / coating.
    pub finish: Option<String>,
    /// Fire protection type.
    pub fire_protection: Option<String>,
    /// User-defined attributes.
    #[serde(default)]
    pub udas: HashMap<String, serde_json::Value>,
    /// Start release condition.
    pub start_release: AdvSteelRelease,
    /// End release condition.
    pub end_release: AdvSteelRelease,
}

/// End release flags.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdvSteelRelease {
    pub moment: bool,
    pub torsion: bool,
}

impl AdvSteelRelease {
    pub fn pmef_connection_type(&self) -> &'static str {
        if self.moment { "PINNED" } else { "FIXED" }
    }
}

/// Advanced Steel member type classification.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum AdvSteelMemberType {
    Beam, Column, Brace, Purlin, Girt, Rail, Other,
}

impl AdvSteelMemberType {
    pub fn pmef_member_type(&self) -> &'static str {
        match self {
            Self::Beam | Self::Purlin | Self::Girt | Self::Rail => "BEAM",
            Self::Column => "COLUMN",
            Self::Brace  => "BRACE",
            _            => "GENERIC",
        }
    }
}

/// An Advanced Steel plate element.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvSteelPlate {
    pub handle: String,
    pub member_mark: String,
    pub grade: String,
    pub thickness_mm: f64,
    pub length_mm: f64,
    pub width_mm: f64,
    pub mass_kg: Option<f64>,
    /// Origin of the plate [mm, world CS].
    pub origin: [f64; 3],
    /// Normal vector of the plate face.
    pub normal: [f64; 3],
}

/// An Advanced Steel bolt group.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvSteelBoltPattern {
    pub handle: String,
    pub bolt_standard: String,
    pub bolt_diameter_mm: f64,
    pub bolt_grade: String,
    pub bolt_count: u32,
    pub hole_type: String,
    pub preloaded: bool,
    /// Centroid of the bolt group [mm, world CS].
    pub centroid: [f64; 3],
    /// Connected member handles.
    pub connected_handles: Vec<String>,
}

/// An Advanced Steel weld seam.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvSteelWeldSeam {
    pub handle: String,
    pub weld_type: String,
    pub leg_size_mm: f64,
    pub length_mm: f64,
    pub welding_process: String,
    pub weld_number: Option<String>,
    /// Connected member handles.
    pub connected_handles: Vec<String>,
}

/// An Advanced Steel anchor bolt pattern.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvSteelAnchorPattern {
    pub handle: String,
    pub anchor_standard: String,
    pub anchor_diameter_mm: f64,
    pub anchor_grade: String,
    pub anchor_count: u32,
    pub embedded_length_mm: f64,
    pub centroid: [f64; 3],
}

// ─────────────────────────────────────────────────────────────────────────────
// Profile ID normalisation
// ─────────────────────────────────────────────────────────────────────────────

/// Map an Advanced Steel section name + standard to a PMEF profile ID.
pub fn adv_steel_profile_id(section: &str, standard: &str) -> String {
    let s = section.trim().replace(' ', "");
    let std = match standard.to_uppercase().as_str() {
        "DIN" | "EN" | "EUROPEAN" | "ISO" => "EN",
        "AISC" | "ANSI" | "US"            => "AISC",
        "BS" | "BRITISH"                   => "BS",
        "AS" | "AUSTRALIAN"                => "AS",
        _                                  => "EN",
    };
    format!("{std}:{s}")
}

fn fy_fu(grade: &str) -> (f64, f64, &'static str) {
    match grade.to_uppercase().replace(['-',' '], "").as_str() {
        "S235" | "S235JR" => (235., 360., "EN 10025-2"),
        "S275" | "S275JR" => (275., 430., "EN 10025-2"),
        "S355" | "S355JR" => (355., 490., "EN 10025-2"),
        "S420" | "S420ML" => (420., 520., "EN 10025-4"),
        "S460" | "S460ML" => (460., 550., "EN 10025-4"),
        "A992"            => (345., 448., "ASTM A992"),
        "A36" | "A36"     => (235., 400., "ASTM A36"),
        _                 => (275., 430., "EN 10025-2"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AdvSteelConfig {
    pub project_code: String,
    pub export_path: PathBuf,
    pub include_plates: bool,
    pub include_connections: bool,
    pub include_welds: bool,
    pub unit_id: Option<String>,
}

impl Default for AdvSteelConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            export_path: PathBuf::from("advsteel-export.json"),
            include_plates: false,
            include_connections: true,
            include_welds: true,
            unit_id: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Adapter
// ─────────────────────────────────────────────────────────────────────────────

pub struct AdvSteelAdapter {
    config: AdvSteelConfig,
    handle_to_id: HashMap<String, String>,
}

impl AdvSteelAdapter {
    pub fn new(config: AdvSteelConfig) -> Self {
        Self { config, handle_to_id: HashMap::new() }
    }

    fn unit_id(&self) -> String {
        self.config.unit_id.clone()
            .unwrap_or_else(|| format!("urn:pmef:unit:{}:U-01", self.config.project_code))
    }

    fn beam_id(&self, mark: &str, handle: &str) -> String {
        let clean: String = mark.chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
            .collect();
        format!("urn:pmef:obj:{}:STR-{clean}", self.config.project_code)
    }

    fn conn_id(&self, handle: &str) -> String {
        let short: String = handle.chars().take(8).collect();
        format!("urn:pmef:obj:{}:CON-{short}", self.config.project_code)
    }

    fn has_equiv(&self, pmef_id: &str, handle: &str) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-adsteel", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id, "targetId": pmef_id,
            "targetSystem": "ADVANCED_STEEL",
            "targetSystemId": handle,
            "confidence": 1.0,
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-advanced-steel 0.9.0" }
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
        let export: AdvSteelExport = serde_json::from_str(&json).map_err(|e| AdapterError::Json(e))?;

        tracing::info!("Advanced Steel: {} beams, {} plates, {} bolt patterns",
            export.beams.len(), export.plates.len(), export.bolt_patterns.len());

        // Pre-register beam IDs
        for beam in &export.beams {
            let id = self.beam_id(&beam.member_mark, &beam.handle);
            self.handle_to_id.insert(beam.handle.clone(), id);
        }

        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        let proj = self.config.project_code.clone();
        let proj_clean: String = export.model_name.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-').collect();
        let plant_id = format!("urn:pmef:plant:{proj}:{proj_clean}");

        for hdr in [
            serde_json::json!({
                "@type":"pmef:FileHeader","@id":format!("urn:pmef:pkg:{proj}:{proj_clean}"),
                "pmefVersion":"0.9.0","plantId":plant_id,"projectCode":proj,
                "coordinateSystem":"Z-up","units":"mm","revisionId":"r2026-01-01-001",
                "changeState":"SHARED",
                "authoringTool":format!("pmef-adapter-advanced-steel 0.9.0 / {}", export.advanced_steel_version)
            }),
            serde_json::json!({
                "@type":"pmef:Plant","@id":plant_id,"pmefVersion":"0.9.0",
                "name":export.model_name,
                "revision":{"revisionId":"r2026-01-01-001","changeState":"SHARED"}
            }),
            serde_json::json!({
                "@type":"pmef:Unit","@id":self.unit_id(),"pmefVersion":"0.9.0",
                "name":export.model_name,"isPartOf":plant_id,
                "revision":{"revisionId":"r2026-01-01-001","changeState":"SHARED"}
            }),
        ] {
            writer.write_value(&hdr).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Beams
        for beam in &export.beams {
            for obj in self.map_beam(beam) {
                writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                stats.objects_ok += 1;
            }
        }

        // Bolt patterns → SteelConnection
        if self.config.include_connections {
            for bp in &export.bolt_patterns {
                let objs = self.map_bolt_pattern(bp);
                for obj in objs {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // Weld seams → SteelConnection
        if self.config.include_welds {
            for ws in &export.weld_seams {
                let objs = self.map_weld_seam(ws);
                for obj in objs {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        Ok(stats)
    }

    fn map_beam(&self, beam: &AdvSteelBeam) -> Vec<serde_json::Value> {
        let obj_id = self.handle_to_id.get(&beam.handle)
            .cloned()
            .unwrap_or_else(|| self.beam_id(&beam.member_mark, &beam.handle));
        let profile_id = adv_steel_profile_id(&beam.section, &beam.section_standard);
        let (fy, fu, std) = fy_fu(&beam.grade);

        let obj = serde_json::json!({
            "@type": "pmef:SteelMember",
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": self.unit_id(),
            "memberMark": beam.member_mark,
            "memberType": beam.member_type.pmef_member_type(),
            "profileId": profile_id,
            "startPoint": beam.start_point,
            "endPoint":   beam.end_point,
            "rollAngle":  beam.roll_angle_deg,
            "material": { "grade": beam.grade, "standard": std, "fy": fy, "fu": fu },
            "weight": beam.mass_kg,
            "finish": beam.finish,
            "fireProtection": beam.fire_protection.as_ref().map(|fp| serde_json::json!({ "type": fp })),
            "startConnectionType": beam.start_release.pmef_connection_type(),
            "endConnectionType":   beam.end_release.pmef_connection_type(),
            "customAttributes": {
                "advSteelHandle": beam.handle,
                "assemblyMark": beam.assembly_mark,
                "surfaceAreaM2": beam.surface_area_m2,
                "udas": beam.udas
            },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-advanced-steel 0.9.0" }
        });
        vec![obj, self.has_equiv(&obj_id, &beam.handle)]
    }

    fn map_bolt_pattern(&self, bp: &AdvSteelBoltPattern) -> Vec<serde_json::Value> {
        let conn_id = self.conn_id(&bp.handle);
        let member_ids: Vec<String> = bp.connected_handles.iter()
            .filter_map(|h| self.handle_to_id.get(h))
            .cloned()
            .collect();

        let obj = serde_json::json!({
            "@type": "pmef:SteelConnection",
            "@id": conn_id,
            "connectionType": "BOLTED_ENDPLATE",
            "memberIds": member_ids,
            "coordinate": bp.centroid,
            "boltSpec": {
                "boltGrade": bp.bolt_grade,
                "boltDiameter": bp.bolt_diameter_mm,
                "numberOfBolts": bp.bolt_count,
                "holeType": bp.hole_type,
                "preloaded": bp.preloaded,
                "standard": bp.bolt_standard
            },
            "customAttributes": { "advSteelHandle": bp.handle },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-advanced-steel 0.9.0" }
        });
        vec![obj, self.has_equiv(&conn_id, &bp.handle)]
    }

    fn map_weld_seam(&self, ws: &AdvSteelWeldSeam) -> Vec<serde_json::Value> {
        let conn_id = self.conn_id(&ws.handle);
        let member_ids: Vec<String> = ws.connected_handles.iter()
            .filter_map(|h| self.handle_to_id.get(h))
            .cloned()
            .collect();

        let obj = serde_json::json!({
            "@type": "pmef:SteelConnection",
            "@id": conn_id,
            "connectionType": "WELDED",
            "memberIds": member_ids,
            "weldSpec": {
                "weldNumber": ws.weld_number,
                "weldType": ws.weld_type,
                "legSizeMm": ws.leg_size_mm,
                "lengthMm": ws.length_mm,
                "weldingProcess": ws.welding_process
            },
            "customAttributes": { "advSteelHandle": ws.handle },
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-advanced-steel 0.9.0" }
        });
        vec![obj, self.has_equiv(&conn_id, &ws.handle)]
    }
}

impl PmefAdapter for AdvSteelAdapter {
    fn name(&self) -> &str { "pmef-adapter-advanced-steel" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "ADVANCED_STEEL" }
    fn supported_domains(&self) -> &[&str] { &["steel"] }
    fn conformance_level(&self) -> u8 { 3 } // Level 3: full steel domain
    fn description(&self) -> &str {
        "Autodesk Advanced Steel → PMEF adapter. Maps structural beams, columns, \
         braces, bolt groups and weld seams to PMEF SteelMember and SteelConnection \
         objects. Level 3 conformance for the steel domain. \
         Full profile mapping for DIN/EN/AISC/BS/AS section catalogs."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_beam() -> AdvSteelBeam {
        AdvSteelBeam {
            handle: "HANDLE-B101".to_owned(),
            member_mark: "B-101".to_owned(),
            assembly_mark: Some("ASM-001".to_owned()),
            section: "HEA200".to_owned(),
            section_standard: "DIN".to_owned(),
            grade: "S355JR".to_owned(),
            member_type: AdvSteelMemberType::Beam,
            start_point: [0.,0.,6000.],
            end_point: [6000.,0.,6000.],
            length_mm: 6000., roll_angle_deg: 0.,
            mass_kg: Some(126.), surface_area_m2: Some(2.4),
            finish: Some("HotDipGalvanized".to_owned()),
            fire_protection: None, udas: Default::default(),
            start_release: AdvSteelRelease::default(),
            end_release:   AdvSteelRelease::default(),
        }
    }

    #[test]
    fn test_adv_steel_profile_id() {
        assert_eq!(adv_steel_profile_id("HEA200",  "DIN"),   "EN:HEA200");
        assert_eq!(adv_steel_profile_id("W12x53",  "AISC"),  "AISC:W12x53");
        assert_eq!(adv_steel_profile_id("SHS150x6","EN"),    "EN:SHS150x6");
        assert_eq!(adv_steel_profile_id("203x203x60UC","BS"),"BS:203x203x60UC");
    }

    #[test]
    fn test_map_beam() {
        let config = AdvSteelConfig { project_code: "test".to_owned(),
            export_path: PathBuf::from("x.json"), ..Default::default() };
        let adapter = AdvSteelAdapter::new(config);
        let beam = test_beam();
        let objs = adapter.map_beam(&beam);
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0]["@type"], "pmef:SteelMember");
        assert_eq!(objs[0]["memberMark"], "B-101");
        assert_eq!(objs[0]["memberType"], "BEAM");
        assert_eq!(objs[0]["profileId"], "EN:HEA200");
        assert_eq!(objs[0]["material"]["grade"], "S355JR");
        assert!((objs[0]["material"]["fy"].as_f64().unwrap() - 355.).abs() < 0.1);
        assert_eq!(objs[0]["finish"], "HotDipGalvanized");
        assert_eq!(objs[0]["startConnectionType"], "FIXED");
        assert_eq!(objs[1]["targetSystem"], "ADVANCED_STEEL");
    }

    #[test]
    fn test_end_release_types() {
        let pinned = AdvSteelRelease { moment: true, torsion: false };
        assert_eq!(pinned.pmef_connection_type(), "PINNED");
        let fixed = AdvSteelRelease::default();
        assert_eq!(fixed.pmef_connection_type(), "FIXED");
    }

    #[test]
    fn test_member_type_pmef() {
        assert_eq!(AdvSteelMemberType::Beam.pmef_member_type(),   "BEAM");
        assert_eq!(AdvSteelMemberType::Column.pmef_member_type(), "COLUMN");
        assert_eq!(AdvSteelMemberType::Brace.pmef_member_type(),  "BRACE");
        assert_eq!(AdvSteelMemberType::Purlin.pmef_member_type(), "BEAM");
    }

    #[test]
    fn test_fy_fu() {
        let (fy, fu, _) = fy_fu("S355JR");
        assert!((fy - 355.).abs() < 0.1);
        assert!((fu - 490.).abs() < 0.1);
        let (fy2, _, std2) = fy_fu("A992");
        assert!((fy2 - 345.).abs() < 0.1);
        assert_eq!(std2, "ASTM A992");
    }

    #[test]
    fn test_adapter_level_3() {
        let adapter = AdvSteelAdapter::new(AdvSteelConfig::default());
        assert_eq!(adapter.conformance_level(), 3);
        assert_eq!(adapter.target_system(), "ADVANCED_STEEL");
    }
}
