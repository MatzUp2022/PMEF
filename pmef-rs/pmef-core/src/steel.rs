//! Structural steel domain: SteelSystem, SteelMember, SteelPlate, SteelNode, SteelConnection.
use crate::revision::RevisionMetadata;
use crate::types::{Coordinate3D, PmefId};
use serde::{Deserialize,Serialize};
use std::collections::HashMap;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelMaterial { pub grade:String,
    /// Yield strength [MPa].
    #[serde(default, skip_serializing_if="Option::is_none")] pub fy:Option<f64>,
    /// Ultimate tensile strength [MPa].
    #[serde(default, skip_serializing_if="Option::is_none")] pub fu:Option<f64>,
    /// [kg/m³], schema default 7850.
    #[serde(default, skip_serializing_if="Option::is_none")] pub density:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub standard:Option<String> }

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelMember {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_mark: Option<String>,
    pub member_type: String,
    /// Profile ID, e.g. `"EN:HEA200"` or `"AISC:W12x53"`.
    pub profile_id: String,
    pub start_point: Coordinate3D,
    pub end_point: Coordinate3D,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<SteelMaterial>,
    #[serde(skip_serializing_if="Option::is_none")] pub roll_angle:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub weight:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub finish:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub cis2_ref:Option<String>,
    #[serde(rename="teklaGUID", skip_serializing_if="Option::is_none")] pub tekla_guid:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if="Option::is_none")] pub custom_attributes:Option<HashMap<String,serde_json::Value>>,
    /// Nominal member length [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    /// Fire protection specification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire_protection: Option<serde_json::Value>,
    /// Structural analysis results (from FEM/stress analysis)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_results: Option<serde_json::Value>,
    /// Reference designation according to IEC 81346. Prefix defines the aspect: '=' function, '-' product, '+' location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<crate::types::Iec81346Designation>,
    /// URI pointing to a class in ISO 15926-4 (PCA-RDL), CFIHOS-RDL or a project-level catalog. Must be resolvable via SPARQL endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    /// Link from a PMEF object to one or more geometry representations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<crate::geometry::GeometryReference>,
    /// Schema property `documents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_number: Option<u32>,
    pub coordinate: Coordinate3D,
    #[serde(default)]
    pub member_ids: Vec<PmefId>,
    #[serde(skip_serializing_if="Option::is_none")] pub support_type:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    /// SteelConnection at this node
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<crate::types::PmefId>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelConnection {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub is_part_of: PmefId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_mark: Option<String>,
    pub connection_type: String,
    pub member_ids: Vec<PmefId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinate: Option<Coordinate3D>,
    #[serde(skip_serializing_if="Option::is_none")] pub utilisation_ratio:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub tekla_connection_number:Option<u32>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    /// Bolt specification (if bolted connection)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bolt_spec: Option<serde_json::Value>,
    /// Weld specification (if welded connection)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weld_spec: Option<serde_json::Value>,
    /// Design capacity of the connection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_capacity: Option<serde_json::Value>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelSystem {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub system_name: String,
    /// Level in a nested structure (RFC-0013): `STRUCTURE`, `FRAMEWORK`, `SUBFRAMEWORK`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_type: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub design_code:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub steel_grade:Option<String>,
    #[serde(default)]
    pub member_ids: Vec<PmefId>,
    /// SteelPlate objects of this system (RFC-0013).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plate_ids: Vec<PmefId>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

/// Plate, grating, chequered plate or panel (RFC-0013): a planar outline in the local x/y plane
/// (`origin`, `ref_direction` = x, `normal` = z) extruded by `thickness` along +normal.
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SteelPlate {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plate_mark: Option<String>,
    /// `PLATE`, `GRATING`, `CHEQUERED`, `PANEL`, `GENERIC`.
    pub plate_type: String,
    pub origin: Coordinate3D,
    pub normal: crate::types::UnitVector3D,
    pub ref_direction: crate::types::UnitVector3D,
    /// Outer contour in local x/y [mm], first point not repeated.
    pub outline: Vec<[f64; 2]>,
    /// Openings, each a contour in local x/y [mm].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<[f64; 2]>>,
    /// Thickness, grating bar height or panel thickness [mm].
    pub thickness: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<SteelMaterial>,
    #[serde(skip_serializing_if="Option::is_none")] pub weight:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub finish:Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<crate::types::Iec81346Designation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<crate::types::RdlUri>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<crate::geometry::GeometryReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<crate::catalog::DocumentLink>>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if="Option::is_none")] pub custom_attributes:Option<HashMap<String,serde_json::Value>>,
}

impl SteelPlate {
    /// Area of the outline minus the holes [mm²] (shoelace formula).
    pub fn area_mm2(&self) -> f64 {
        fn shoelace(p: &[[f64; 2]]) -> f64 {
            (0..p.len())
                .map(|i| {
                    let (a, b) = (p[i], p[(i + 1) % p.len()]);
                    a[0] * b[1] - b[0] * a[1]
                })
                .sum::<f64>()
                .abs()
                / 2.0
        }
        shoelace(&self.outline) - self.holes.iter().map(|h| shoelace(h)).sum::<f64>()
    }

    /// Steel volume [mm³].
    pub fn volume_mm3(&self) -> f64 {
        self.area_mm2() * self.thickness
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_only_material_round_trips() {
        let m: SteelMaterial = serde_json::from_str(r#"{"grade":"S355JR","density":7850}"#).unwrap();
        assert_eq!((m.fy, m.density), (None, Some(7850.0)));
        assert_eq!(serde_json::to_string(&m).unwrap(), r#"{"grade":"S355JR","density":7850.0}"#);
    }
}
