//! Recognition provenance, review status and as-built data (RFC-0001, spec 02 §2.9).
//!
//! The three optional blocks are carried by every PMEF object that has `revision`.
//! In Rust they are grouped in [`RecognitionMetadata`], which object structs embed with
//! `#[serde(flatten)]`, so they serialise as top-level `provenance`, `review` and `asBuilt`.

use crate::revision::ChangeState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Kind of source an object was created from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    /// Modelled by a person.
    #[serde(rename = "manual")]
    Manual,
    /// Imported from a CAD/engineering tool.
    #[serde(rename = "cad-import")]
    CadImport,
    /// Recognised from an isometric drawing.
    #[serde(rename = "iso-drawing")]
    IsoDrawing,
    /// Recognised from another 2D drawing (P&ID, GA, plan).
    #[serde(rename = "2d-drawing")]
    TwoDDrawing,
    /// Recognised from a point cloud / laser scan.
    #[serde(rename = "scan")]
    Scan,
    /// Recognised from a photo.
    #[serde(rename = "photo")]
    Photo,
}

/// Recognition software that produced an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recognizer {
    /// Recognizer name.
    pub name: String,
    /// Recognizer version.
    pub version: String,
}

/// Origin of a PMEF object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    /// Kind of source.
    pub source_type: SourceType,
    /// URI of the source document/page/region or point-cloud segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    /// Recognition software.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recognizer: Option<Recognizer>,
    /// Recognizer confidence 0..1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// When the object was created from the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

impl Provenance {
    /// Check the normative rules of spec 02 §2.9.1. Returns human-readable violations.
    pub fn violations(&self) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(c) = self.confidence {
            if !(0.0..=1.0).contains(&c) {
                v.push(format!("confidence {c} outside 0..1"));
            }
        }
        if self.recognizer.is_some() && self.confidence.is_none() {
            v.push("recognizer present but confidence missing".into());
        }
        if self.source_type != SourceType::Manual && self.source_ref.is_none() {
            v.push("sourceRef is required unless sourceType is manual".into());
        }
        v
    }
}

/// Human review state of an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewState {
    /// Created by a recogniser, not checked.
    Detected,
    /// Checked by a person, corrections applied, not yet accepted.
    Reviewed,
    /// Accepted for use.
    Approved,
    /// Judged wrong; kept for traceability.
    Rejected,
}

impl ReviewState {
    /// Whether this review state may be combined with the given CDE change state
    /// (MUST rules of spec 02 §2.9.2).
    pub fn allows(self, change_state: ChangeState) -> bool {
        match self {
            ReviewState::Detected => change_state == ChangeState::Wip,
            ReviewState::Rejected => change_state != ChangeState::Published,
            ReviewState::Reviewed | ReviewState::Approved => true,
        }
    }
}

/// Review block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatus {
    /// Review state.
    pub status: ReviewState,
    /// Reviewer (person or role). Required unless `detected`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    /// Time of review. Required unless `detected`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<DateTime<Utc>>,
}

/// Versioned reference to a catalog entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapRef {
    /// Catalog id, e.g. `profiles-en`.
    pub catalog_id: String,
    /// Catalog version, e.g. `2024.1`.
    pub catalog_version: String,
    /// Entry id within the catalog, e.g. `EN:HEB300`.
    pub entry_id: String,
}

/// Deviation of one fitted parameter from the catalog value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Residual {
    /// Parameter name, e.g. `outsideDiameter`, `h`.
    pub parameter: String,
    /// Measured value [mm].
    pub measured_mm: f64,
    /// Catalog value [mm].
    pub catalog_mm: f64,
    /// `measured_mm − catalog_mm` [mm].
    pub residual_mm: f64,
}

/// Catalog entry an object was snapped to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnap {
    /// Versioned catalog reference.
    pub catalog_ref: CatalogSnapRef,
    /// Designation, e.g. `HEB 300`, `DN 150 PN 40`.
    pub snapped_size: String,
    /// Per-parameter residuals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub residuals: Vec<Residual>,
}

/// As-built fit data.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsBuilt {
    /// Point cloud (segment) the object was fitted to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_cloud_ref: Option<String>,
    /// Fit method, e.g. `ransac-cylinder`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit_method: Option<String>,
    /// RMS deviation [mm].
    #[serde(rename = "deviationRms_mm", skip_serializing_if = "Option::is_none")]
    pub deviation_rms_mm: Option<f64>,
    /// Maximum deviation [mm].
    #[serde(rename = "deviationMax_mm", skip_serializing_if = "Option::is_none")]
    pub deviation_max_mm: Option<f64>,
    /// Catalog snap result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_snap: Option<CatalogSnap>,
}

/// The three optional RFC-0001 blocks, flattened into every object struct.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionMetadata {
    /// Origin of the object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    /// Human review status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewStatus>,
    /// As-built fit data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_built: Option<AsBuilt>,
}

impl RecognitionMetadata {
    /// True if none of the blocks is present.
    pub fn is_empty(&self) -> bool {
        self.provenance.is_none() && self.review.is_none() && self.as_built.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> serde_json::Value {
        json!({
            "provenance": {"sourceType": "scan", "sourceRef": "file:scan.e57#segment=7",
                           "recognizer": {"name": "r", "version": "1.0"}, "confidence": 0.9,
                           "createdAt": "2026-10-30T09:12:00Z"},
            "review": {"status": "approved", "reviewedBy": "rev", "reviewedAt": "2026-10-30T14:00:00Z"},
            "asBuilt": {"pointCloudRef": "file:scan.e57#segment=7", "fitMethod": "ransac-cylinder",
                        "deviationRms_mm": 1.2, "deviationMax_mm": 4.8,
                        "catalogSnap": {"catalogRef": {"catalogId": "piping-class-en-p1a",
                                                       "catalogVersion": "2024.1",
                                                       "entryId": "ENP1A-PIPE-150"},
                                        "snappedSize": "DN 150 PN 40",
                                        "residuals": [{"parameter": "outsideDiameter", "measured_mm": 169.1,
                                                       "catalog_mm": 168.3, "residual_mm": 0.8}]}}
        })
    }

    #[test]
    fn round_trip_preserves_json() {
        let v = sample();
        let m: RecognitionMetadata = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&m).unwrap(), v);
    }

    #[test]
    fn empty_metadata_serialises_to_nothing() {
        let m = RecognitionMetadata::default();
        assert!(m.is_empty());
        assert_eq!(serde_json::to_value(&m).unwrap(), json!({}));
    }

    #[test]
    fn source_type_wire_names() {
        for (t, s) in [(SourceType::TwoDDrawing, "2d-drawing"), (SourceType::CadImport, "cad-import"),
                       (SourceType::IsoDrawing, "iso-drawing")] {
            assert_eq!(serde_json::to_value(t).unwrap(), json!(s));
        }
    }

    #[test]
    fn provenance_rules() {
        let mut p: Provenance = serde_json::from_value(sample()["provenance"].clone()).unwrap();
        assert!(p.violations().is_empty());
        p.confidence = None;
        assert_eq!(p.violations().len(), 1);
        p.source_ref = None;
        assert_eq!(p.violations().len(), 2);
        p.source_type = SourceType::Manual;
        p.recognizer = None;
        assert!(p.violations().is_empty());
    }

    #[test]
    fn review_cde_rules() {
        assert!(ReviewState::Detected.allows(ChangeState::Wip));
        assert!(!ReviewState::Detected.allows(ChangeState::Shared));
        assert!(!ReviewState::Rejected.allows(ChangeState::Published));
        assert!(ReviewState::Rejected.allows(ChangeState::Archived));
        assert!(ReviewState::Approved.allows(ChangeState::Published));
    }
}
