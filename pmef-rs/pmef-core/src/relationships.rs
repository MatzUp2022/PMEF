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
});
rel!(IsCollocatedWith {});
rel!(ReplacedBy {
    replacement_date: String,
    work_order_ref: String,
});
