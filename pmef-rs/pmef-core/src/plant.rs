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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_state: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub authoring_tool: Option<String>,
    /// Person or organisation that created this file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// ISO 8601 timestamp when this file was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Name of the authoring/export tool, e.g. 'PMEF Reference Generator'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Version of the authoring/export tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_version: Option<String>,
    /// Human-readable project name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    /// Base linear unit for all coordinates. Default: millimetres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Plant {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub epsg_code: Option<u32>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    /// Human-readable plant name, e.g. 'Electric Arc Furnace Line 3'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant_name: Option<String>,
    /// Project origin in the EPSG coordinate system (easting, northing, elevation in mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<crate::types::Coordinate3D>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Unit {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub is_part_of: PmefId,
    #[serde(skip_serializing_if="Option::is_none")]
    pub unit_number: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub process_type: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    /// Human-readable unit name, e.g. 'Cooling Water Unit'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_name: Option<String>,
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
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}
