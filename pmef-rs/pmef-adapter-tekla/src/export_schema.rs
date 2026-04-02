//! Tekla Structures JSON export data types.
//!
//! Tekla Structures can export structural data in multiple formats:
//!
//! 1. **Tekla Open API (C#/.NET)** — the primary programmatic interface,
//!    runs inside the Tekla process. The companion C# project
//!    (`pmef-tekla-dotnet/`) uses this API to produce a structured JSON
//!    export consumed by this Rust crate.
//!
//! 2. **CIS/2 XML** — structural steel exchange standard (parsed separately).
//!
//! 3. **IFC 2x3 / IFC 4** — general BIM exchange (handled by IFC adapter).
//!
//! The JSON export schema is defined here and mirrors the C# export model.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Top-level export
// ─────────────────────────────────────────────────────────────────────────────

/// Root of the Tekla JSON export produced by `PmefExporter.cs`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaExport {
    /// Export format version (must be "1.0").
    pub schema_version: String,
    /// Tekla Structures version string.
    pub tekla_version: String,
    /// Export timestamp (ISO 8601).
    pub exported_at: String,
    /// Model name.
    pub model_name: String,
    /// Project information.
    #[serde(default)]
    pub project: Option<TeklaProject>,
    /// All exported structural members.
    #[serde(default)]
    pub members: Vec<TeklaMember>,
    /// All exported connections.
    #[serde(default)]
    pub connections: Vec<TeklaConnection>,
    /// All exported assemblies.
    #[serde(default)]
    pub assemblies: Vec<TeklaAssembly>,
    /// All exported grids.
    #[serde(default)]
    pub grids: Vec<TeklaGrid>,
    /// Total object counts.
    pub summary: TeklaExportSummary,
}

/// Project metadata from Tekla.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaProject {
    pub project_name: String,
    #[serde(default)] pub project_number: Option<String>,
    #[serde(default)] pub designer: Option<String>,
    #[serde(default)] pub design_code: Option<String>,
    #[serde(default)] pub steel_grade: Option<String>,
}

/// Export summary counts.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaExportSummary {
    pub member_count: u32,
    pub connection_count: u32,
    pub assembly_count: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Structural members
// ─────────────────────────────────────────────────────────────────────────────

/// Coordinate [mm] in the Tekla world coordinate system.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeklaPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl TeklaPoint {
    pub fn distance_to(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
}

/// Bounding box [mm].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaBbox {
    pub min: TeklaPoint,
    pub max: TeklaPoint,
}

/// Tekla member type (maps to Tekla `ModelObject` subclass).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum TeklaMemberClass {
    Beam,
    Column,
    Brace,
    TrussMember,
    PolyBeam,
    CurvedBeam,
    Pad,         // concrete pad footing
    Strip,       // concrete strip footing
    ContourPlate,
    Slab,
    Wall,
    Other,
}

impl TeklaMemberClass {
    /// Map to a PMEF `memberType` string.
    pub fn pmef_member_type(&self) -> &'static str {
        match self {
            Self::Beam        => "BEAM",
            Self::Column      => "COLUMN",
            Self::Brace       => "BRACE",
            Self::TrussMember => "BRACE",
            Self::PolyBeam    => "BEAM",
            Self::CurvedBeam  => "BEAM",
            Self::Pad         => "FOUNDATION",
            Self::Strip       => "FOUNDATION",
            Self::ContourPlate=> "PLATE",
            Self::Slab        => "SLAB",
            Self::Wall        => "WALL",
            Self::Other       => "GENERIC",
        }
    }

    /// Returns true if this is a structural steel member (not concrete).
    pub fn is_steel(&self) -> bool {
        matches!(self,
            Self::Beam | Self::Column | Self::Brace |
            Self::TrussMember | Self::PolyBeam | Self::CurvedBeam |
            Self::ContourPlate
        )
    }
}

/// Tekla finish / coating type.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum TeklaFinish {
    None,
    HotDipGalvanized,
    PaintedEpoxy,
    PaintedAlkyd,
    Blasted,
    Other(String),
}

impl TeklaFinish {
    pub fn as_pmef_str(&self) -> &str {
        match self {
            Self::None             => "NONE",
            Self::HotDipGalvanized => "HOT_DIP_GALVANIZED",
            Self::PaintedEpoxy     => "PAINTED_EPOXY",
            Self::PaintedAlkyd     => "PAINTED_ALKYD",
            Self::Blasted          => "BLAST_CLEANED",
            Self::Other(_)         => "OTHER",
        }
    }
}

/// Analysis result for a single member (from Tekla Structural Designer or RSTAB link).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TeklaAnalysisResult {
    /// Unity check / utilisation ratio (0–1, or >1 if overstressed).
    #[serde(default)] pub utilisation_ratio: Option<f64>,
    /// Critical design check description.
    #[serde(default)] pub critical_check: Option<String>,
    /// Axial force [kN] (positive = tension).
    #[serde(default)] pub axial_force_kn: Option<f64>,
    /// Major axis bending moment [kN·m].
    #[serde(default)] pub major_bending_knm: Option<f64>,
    /// Minor axis bending moment [kN·m].
    #[serde(default)] pub minor_bending_knm: Option<f64>,
    /// Major axis shear [kN].
    #[serde(default)] pub shear_y_kn: Option<f64>,
    /// Minor axis shear [kN].
    #[serde(default)] pub shear_z_kn: Option<f64>,
}

/// A single structural member from Tekla Structures.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaMember {
    /// Tekla model object identifier (GlobalId from Open API).
    pub identifier: String,
    /// Tekla numeric object ID (from `ModelObject.Identifier.ID`).
    pub tekla_id: u64,
    /// Member class (Beam, Column, Brace, etc.).
    pub member_class: TeklaMemberClass,
    /// Member mark / position number.
    pub member_mark: String,
    /// Assembly / part mark.
    #[serde(default)] pub part_mark: Option<String>,
    /// Profile designation (Tekla native string, e.g. `"HEA200"`, `"W12X53"`).
    pub profile: String,
    /// Steel grade designation.
    pub material: String,
    /// Start point of the member axis [mm, world CS].
    pub start_point: TeklaPoint,
    /// End point of the member axis [mm, world CS].
    pub end_point: TeklaPoint,
    /// Roll angle around member axis [degrees].
    #[serde(default)] pub roll_angle_deg: f64,
    /// Member length [mm].
    pub length_mm: f64,
    /// Mass [kg].
    #[serde(default)] pub mass_kg: Option<f64>,
    /// Surface area [m²].
    #[serde(default)] pub surface_area_m2: Option<f64>,
    /// Tekla GUID for round-trip identity.
    pub guid: String,
    /// CIS/2 member reference (from Tekla CIS/2 export).
    #[serde(default)] pub cis2_ref: Option<String>,
    /// User-defined attributes (UDAs) from the Tekla model.
    #[serde(default)] pub udas: HashMap<String, serde_json::Value>,
    /// Assembly the member belongs to.
    #[serde(default)] pub assembly_id: Option<String>,
    /// Finish / coating type.
    #[serde(default)] pub finish: Option<TeklaFinish>,
    /// Fire protection specification.
    #[serde(default)] pub fire_protection: Option<TeklaFireProtection>,
    /// Analysis results (if available from linked analysis tool).
    #[serde(default)] pub analysis: Option<TeklaAnalysisResult>,
    /// Bounding box.
    #[serde(default)] pub bbox: Option<TeklaBbox>,
    /// Start release condition.
    #[serde(default)] pub start_release: TeklaEndRelease,
    /// End release condition.
    #[serde(default)] pub end_release: TeklaEndRelease,
}

/// End release condition (for analysis).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TeklaEndRelease {
    #[serde(default)] pub moment_major: bool,  // true = moment released (pinned)
    #[serde(default)] pub moment_minor: bool,
    #[serde(default)] pub torsion: bool,
}

impl TeklaEndRelease {
    /// True if both major and minor moments are released (fully pinned).
    pub fn is_pinned(&self) -> bool { self.moment_major && self.moment_minor }
    /// True if no releases (fully fixed).
    pub fn is_fixed(&self) -> bool { !self.moment_major && !self.moment_minor && !self.torsion }
}

/// Fire protection specification on a steel member.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaFireProtection {
    /// Type of fire protection (e.g. `"INTUMESCENT_PAINT"`, `"BOARD"`, `"SPRAY"`).
    pub protection_type: String,
    /// Required fire resistance period [minutes].
    pub required_period_min: u32,
    /// Section factor Am/V [m⁻¹].
    #[serde(default)] pub section_factor_m: Option<f64>,
    /// Paint thickness [mm] for intumescent coating.
    #[serde(default)] pub thickness_mm: Option<f64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Connections
// ─────────────────────────────────────────────────────────────────────────────

/// Tekla connection type.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum TeklaConnectionType {
    BoltedEndPlate,
    BoltedCleat,
    WeldedDirect,
    BoltedSplice,
    PinnedBase,
    FixedBase,
    MomentEndPlate,
    TubularKJoint,
    TubularYJoint,
    Other,
}

impl TeklaConnectionType {
    pub fn pmef_connection_type(&self) -> &'static str {
        match self {
            Self::BoltedEndPlate   => "BOLTED_ENDPLATE",
            Self::BoltedCleat      => "BOLTED_CLEAT",
            Self::WeldedDirect     => "WELDED",
            Self::BoltedSplice     => "BOLTED_SPLICE",
            Self::PinnedBase       => "PINNED_BASE",
            Self::FixedBase        => "FIXED_BASE",
            Self::MomentEndPlate   => "MOMENT_ENDPLATE",
            Self::TubularKJoint    => "TUBULAR_K",
            Self::TubularYJoint    => "TUBULAR_Y",
            Self::Other            => "OTHER",
        }
    }
}

/// Bolt specification.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaBoltSpec {
    /// Bolt grade (e.g. `"8.8"`, `"10.9"`, `"A325"`, `"A490"`).
    pub grade: String,
    /// Bolt diameter [mm].
    pub diameter_mm: f64,
    /// Total number of bolts.
    pub count: u32,
    /// Bolt hole type.
    pub hole_type: TeklaHoleType,
    /// True if high-strength preloaded (HSFG).
    #[serde(default)] pub preloaded: bool,
    /// Bolt assembly (nut + washer specification).
    #[serde(default)] pub assembly: Option<String>,
}

/// Bolt hole type.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum TeklaHoleType {
    Clearance,
    Oversized,
    SlottedShort,
    SlottedLong,
}

/// A structural connection between two or more members.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaConnection {
    /// Tekla connection GUID.
    pub identifier: String,
    /// Tekla connection number (from component dialog).
    pub tekla_id: u64,
    /// Tekla system component number (e.g. 142 = End Plate Moment Connection).
    pub component_number: u32,
    /// Connection type.
    pub connection_type: TeklaConnectionType,
    /// Connection mark.
    #[serde(default)] pub connection_mark: Option<String>,
    /// GUIDs of connected members.
    pub member_guids: Vec<String>,
    /// Position of the connection centroid [mm].
    pub position: TeklaPoint,
    /// Bolt specification (if bolted).
    #[serde(default)] pub bolt_spec: Option<TeklaBoltSpec>,
    /// Weld size [mm] (if welded, leg length).
    #[serde(default)] pub weld_size_mm: Option<f64>,
    /// Design capacity (from Tekla Connection Designer or Tedds link).
    #[serde(default)] pub design_capacity: Option<TeklaConnectionCapacity>,
    /// Utilisation ratio (0–1).
    #[serde(default)] pub utilisation_ratio: Option<f64>,
}

/// Connection design capacity from analysis.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaConnectionCapacity {
    /// Shear capacity [kN].
    #[serde(default)] pub shear_kn: Option<f64>,
    /// Moment capacity [kN·m].
    #[serde(default)] pub moment_knm: Option<f64>,
    /// Axial capacity [kN].
    #[serde(default)] pub axial_kn: Option<f64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Assemblies
// ─────────────────────────────────────────────────────────────────────────────

/// A Tekla assembly (fabrication unit — one piece or one pre-assembled unit).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaAssembly {
    pub identifier: String,
    pub assembly_mark: String,
    pub member_guids: Vec<String>,
    pub mass_kg: f64,
    #[serde(default)] pub surface_area_m2: Option<f64>,
    #[serde(default)] pub finish: Option<TeklaFinish>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Grid
// ─────────────────────────────────────────────────────────────────────────────

/// A Tekla structural grid (axes).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeklaGrid {
    pub name: String,
    pub origin: TeklaPoint,
    pub x_labels: Vec<String>,
    pub y_labels: Vec<String>,
    pub z_labels: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// CIS/2 XML export types
// ─────────────────────────────────────────────────────────────────────────────

/// A member record parsed from a CIS/2 STRUCTURAL_MEMBER element.
#[derive(Debug, Clone)]
pub struct Cis2Member {
    /// CIS/2 member ID.
    pub id: String,
    /// CIS/2 section designation.
    pub section: String,
    /// Steel grade.
    pub grade: String,
    /// Length [mm].
    pub length_mm: f64,
    /// Start point [mm, CIS/2 CS = Y-up; adapter converts to PMEF Z-up].
    pub start_point: (f64, f64, f64),
    /// End point [mm].
    pub end_point: (f64, f64, f64),
    /// CIS/2 member type string.
    pub member_type: String,
}

impl Cis2Member {
    /// Convert CIS/2 Y-up coordinates to PMEF Z-up.
    /// CIS/2 uses: X=East, Y=Up, Z=South.
    /// PMEF uses:  X=East, Y=North, Z=Up.
    pub fn start_pmef(&self) -> (f64, f64, f64) {
        cis2_to_pmef(self.start_point.0, self.start_point.1, self.start_point.2)
    }
    pub fn end_pmef(&self) -> (f64, f64, f64) {
        cis2_to_pmef(self.end_point.0, self.end_point.1, self.end_point.2)
    }
}

/// Convert CIS/2 (X=East, Y=Up, Z=South) to PMEF (X=East, Y=North, Z=Up).
/// Transformation: PMEF_x = CIS2_x, PMEF_y = -CIS2_z, PMEF_z = CIS2_y
pub fn cis2_to_pmef(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    (x, -z, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_member_class_pmef_type() {
        assert_eq!(TeklaMemberClass::Beam.pmef_member_type(), "BEAM");
        assert_eq!(TeklaMemberClass::Column.pmef_member_type(), "COLUMN");
        assert_eq!(TeklaMemberClass::Brace.pmef_member_type(), "BRACE");
        assert_eq!(TeklaMemberClass::Pad.pmef_member_type(), "FOUNDATION");
    }

    #[test]
    fn test_member_class_is_steel() {
        assert!(TeklaMemberClass::Beam.is_steel());
        assert!(TeklaMemberClass::Column.is_steel());
        assert!(!TeklaMemberClass::Pad.is_steel());
        assert!(!TeklaMemberClass::Slab.is_steel());
    }

    #[test]
    fn test_end_release_pinned() {
        let pinned = TeklaEndRelease { moment_major: true, moment_minor: true, torsion: false };
        assert!(pinned.is_pinned());
        assert!(!pinned.is_fixed());
    }

    #[test]
    fn test_end_release_fixed() {
        let fixed = TeklaEndRelease::default();
        assert!(!fixed.is_pinned());
        assert!(fixed.is_fixed());
    }

    #[test]
    fn test_connection_type_mapping() {
        assert_eq!(TeklaConnectionType::BoltedEndPlate.pmef_connection_type(), "BOLTED_ENDPLATE");
        assert_eq!(TeklaConnectionType::WeldedDirect.pmef_connection_type(), "WELDED");
        assert_eq!(TeklaConnectionType::PinnedBase.pmef_connection_type(), "PINNED_BASE");
    }

    #[test]
    fn test_finish_as_pmef_str() {
        assert_eq!(TeklaFinish::HotDipGalvanized.as_pmef_str(), "HOT_DIP_GALVANIZED");
        assert_eq!(TeklaFinish::None.as_pmef_str(), "NONE");
    }

    #[test]
    fn test_cis2_to_pmef_coord() {
        // CIS/2 Y-up: (X=100, Y=5000, Z=-200) → PMEF Z-up: (X=100, Y=200, Z=5000)
        let (px, py, pz) = cis2_to_pmef(100.0, 5000.0, -200.0);
        assert!((px - 100.0).abs() < 0.001);
        assert!((py - 200.0).abs() < 0.001);  // -(-200) = 200
        assert!((pz - 5000.0).abs() < 0.001); // Y_cis2 = Z_pmef
    }

    #[test]
    fn test_cis2_to_pmef_at_origin() {
        let (px, py, pz) = cis2_to_pmef(0.0, 0.0, 0.0);
        assert_eq!((px, py, pz), (0.0, 0.0, 0.0));
    }

    #[test]
    fn test_tekla_point_distance() {
        let a = TeklaPoint { x: 0.0, y: 0.0, z: 0.0 };
        let b = TeklaPoint { x: 3000.0, y: 4000.0, z: 0.0 };
        assert!((a.distance_to(&b) - 5000.0).abs() < 0.001);
    }

    #[test]
    fn test_tekla_export_deserialise() {
        let json = r#"{
            "schemaVersion": "1.0",
            "teklaVersion": "2024",
            "exportedAt": "2026-03-31T00:00:00Z",
            "modelName": "TestModel",
            "members": [],
            "connections": [],
            "assemblies": [],
            "grids": [],
            "summary": { "memberCount": 0, "connectionCount": 0, "assemblyCount": 0 }
        }"#;
        let export: TeklaExport = serde_json::from_str(json).unwrap();
        assert_eq!(export.schema_version, "1.0");
        assert_eq!(export.model_name, "TestModel");
        assert_eq!(export.summary.member_count, 0);
    }
}
