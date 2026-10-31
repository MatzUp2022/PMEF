//! Typed relationship objects.
use crate::revision::RevisionMetadata;
use crate::types::PmefId;
use serde::{Deserialize,Serialize};

macro_rules! rel {
    ($name:ident { $($f:ident : $t:ty),* $(,)? }) => {
        #[derive(Debug,Clone,Serialize,Deserialize)]
        #[serde(rename_all="camelCase")]
        pub struct $name {
            #[serde(rename="@type")] pub entity_type: String,
            #[serde(rename="@id")] pub id: PmefId,
            pub relation_type: String,
            pub source_id: PmefId,
            pub target_id: PmefId,
            #[serde(skip_serializing_if="Option::is_none")] pub confidence:Option<f64>,
            #[serde(skip_serializing_if="Option::is_none")] pub derived_by:Option<String>,
            #[serde(skip_serializing_if="Option::is_none")] pub notes:Option<String>,
            #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
            /// RFC-0001 provenance / review / asBuilt blocks.
            #[serde(flatten, default)]
            pub recognition: crate::provenance::RecognitionMetadata,
            $( #[serde(skip_serializing_if="Option::is_none")] pub $f: Option<$t> ),*
        }
    }
}

rel!(HasEquivalentIn {
    target_system: String,
    target_system_id: String,
    mapping_type: String,
});
rel!(IsDerivedFrom {
    source_standard: String,
    mapping_version: String,
});
rel!(IsPartOf {});
rel!(IsConnectedTo {
    connection_medium: String,
    connection_point_source: String,
    connection_point_target: String,
});
rel!(Supports {
    load_transferred: serde_json::Value,
});
rel!(ControlledBy {
    control_mode: String,
    signal_path: String,
});
rel!(IsDocumentedBy {
    document_type: String,
    document_id: String,
    document_uri: String,
    document_revision: String,
});
rel!(IsRevisionOf {
    change_reason: String,
    change_type: String,
    correspondence: AsBuiltCorrespondence,
});

/// Evidence for an as-built ↔ design correspondence (RFC-0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CorrespondenceMethod {
    /// Same engineering identifier.
    Tag,
    /// Same native tool object (`pmef:HasEquivalentIn`).
    Equivalent,
    /// Geometric proximity of anchor points.
    Geometry,
    /// Derived from matched owner / relationship endpoints.
    Reference,
    /// Assigned by a person.
    Manual,
}

/// How an as-built object (`sourceId`) was matched to its design object (`targetId`).
/// Carried by `pmef:IsRevisionOf` with `changeType = AS_BUILT`; confirmation lives in the
/// relationship's `review` block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AsBuiltCorrespondence {
    /// Evidence used.
    pub method: CorrespondenceMethod,
    /// Mean anchor distance [mm] (geometry matches).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_mm: Option<f64>,
    /// A competing candidate was nearly as close.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambiguous: Option<bool>,
}
rel!(IsCollocatedWith {});
rel!(ReplacedBy {
    replacement_date: String,
    work_order_ref: String,
});
