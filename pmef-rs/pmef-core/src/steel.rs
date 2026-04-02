//! Structural steel domain: SteelSystem, SteelMember, SteelNode, SteelConnection.
use crate::revision::RevisionMetadata;
use crate::types::{Coordinate3D, PmefId};
use serde::{Deserialize,Serialize};
use std::collections::HashMap;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelMaterial { pub grade:String, pub fy:f64, pub fu:f64,
    #[serde(skip_serializing_if="Option::is_none")] pub standard:Option<String> }

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelMember {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub member_mark: String,
    pub member_type: String,
    /// Profile ID, e.g. `"EN:HEA200"` or `"AISC:W12x53"`.
    pub profile_id: String,
    pub start_point: Coordinate3D,
    pub end_point: Coordinate3D,
    pub material: SteelMaterial,
    #[serde(skip_serializing_if="Option::is_none")] pub roll_angle:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub weight:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub finish:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub cis2_ref:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub tekla_guid:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    #[serde(skip_serializing_if="Option::is_none")] pub custom_attributes:Option<HashMap<String,serde_json::Value>>,
}
impl SteelMember {
    pub fn length_mm(&self) -> f64 { self.start_point.distance_to(&self.end_point) }
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelNode {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub is_part_of: PmefId,
    pub node_number: u32,
    pub coordinate: Coordinate3D,
    pub member_ids: Vec<PmefId>,
    #[serde(skip_serializing_if="Option::is_none")] pub support_type:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelConnection {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub is_part_of: PmefId,
    pub connection_mark: String,
    pub connection_type: String,
    pub member_ids: Vec<PmefId>,
    pub coordinate: Coordinate3D,
    #[serde(skip_serializing_if="Option::is_none")] pub utilisation_ratio:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub tekla_connection_number:Option<u32>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelSystem {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub system_name: String,
    pub system_type: String,
    #[serde(skip_serializing_if="Option::is_none")] pub design_code:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub steel_grade:Option<String>,
    pub member_ids: Vec<PmefId>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
}
