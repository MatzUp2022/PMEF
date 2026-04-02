//! Plant hierarchy entities: FileHeader, Plant, Unit, Area.
use crate::revision::RevisionMetadata;
use crate::types::PmefId;
use serde::{Deserialize,Serialize};

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct FileHeader {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub plant_id: PmefId,
    #[serde(skip_serializing_if="Option::is_none")]
    pub project_code: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub coordinate_system: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub description: Option<String>,
    pub revision_id: String,
    pub change_state: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub authoring_tool: Option<String>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Plant {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub name: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub epsg_code: Option<u32>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub revision: Option<RevisionMetadata>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Unit {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub name: String,
    pub is_part_of: PmefId,
    #[serde(skip_serializing_if="Option::is_none")]
    pub unit_number: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub process_type: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub revision: Option<RevisionMetadata>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Area {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    #[serde(skip_serializing_if="Option::is_none")]
    pub area_code: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub area_name: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub revision: Option<RevisionMetadata>,
}
