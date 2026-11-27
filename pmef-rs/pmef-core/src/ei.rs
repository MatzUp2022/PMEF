//! E&I domain: InstrumentObject, InstrumentLoop, PLCObject, CableObject, MtpModule.
use crate::revision::RevisionMetadata;
use crate::types::{Coordinate3D, Iec81346Designation, PmefId};
use serde::{Deserialize,Serialize};
use std::collections::HashMap;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct MeasuredRange { pub min:f64, pub max:f64, pub unit:String }

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SafetySpec {
    pub safety_integrity_level: u8,
    pub safety_function: String,
    pub architecture_type: String,
    #[serde(skip_serializing_if="Option::is_none")] pub pfh:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub pfd:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub proof_test_interval:Option<u32>,
    #[serde(skip_serializing_if="Option::is_none")] pub safe_state:Option<String>,
}
impl SafetySpec { pub fn is_sil_rated(&self) -> bool { self.safety_integrity_level > 0 } }

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ConnectionSpec {
    pub signal_type: String,
    pub fail_safe: bool,
    pub loop_powered: bool,
    #[serde(skip_serializing_if="Option::is_none")] pub intrinsic_safe:Option<bool>,
    #[serde(skip_serializing_if="Option::is_none")] pub hazardous_area:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub ip_rating:Option<String>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct InstrumentObject {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub tag_number: String,
    pub instrument_class: String,
    #[serde(skip_serializing_if="Option::is_none")] pub service_description:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub process_variable:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub loop_number:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub measured_range:Option<MeasuredRange>,
    #[serde(skip_serializing_if="Option::is_none")] pub safety_spec:Option<SafetySpec>,
    #[serde(skip_serializing_if="Option::is_none")] pub connection_spec:Option<ConnectionSpec>,
    #[serde(skip_serializing_if="Option::is_none")] pub iec81346:Option<Iec81346Designation>,
    #[serde(skip_serializing_if="Option::is_none")] pub comos_cuid:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub eplan_bkz:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub tia_plc_address:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(skip_serializing_if="Option::is_none")] pub custom_attributes:Option<HashMap<String,serde_json::Value>>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct InstrumentLoop {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub loop_number: String,
    pub loop_type: String,
    pub is_part_of: PmefId,
    pub member_ids: Vec<PmefId>,
    #[serde(skip_serializing_if="Option::is_none")] pub controller_tag_id:Option<PmefId>,
    #[serde(skip_serializing_if="Option::is_none")] pub final_element_tag_id:Option<PmefId>,
    #[serde(skip_serializing_if="Option::is_none")] pub sil_level:Option<u8>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct PlcObject {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub is_part_of: PmefId,
    pub plc_class: String,
    pub vendor: String,
    pub family: String,
    #[serde(skip_serializing_if="Option::is_none")] pub article_number:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub rack:Option<u32>,
    #[serde(skip_serializing_if="Option::is_none")] pub slot:Option<u32>,
    #[serde(skip_serializing_if="Option::is_none")] pub ip_address:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub safety_cpu:Option<bool>,
    #[serde(skip_serializing_if="Option::is_none")] pub aml_ref:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct CableObject {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub is_part_of: PmefId,
    pub cable_number: String,
    pub cable_type: String,
    pub cross_section: f64,
    pub number_of_cores: u32,
    pub from_id: PmefId,
    pub to_id: PmefId,
    #[serde(skip_serializing_if="Option::is_none")] pub route_length:Option<f64>,
    /// Cable type from a CABLE_TYPES catalog (RFC-0006).
    #[serde(default, skip_serializing_if="Option::is_none")] pub catalog_ref:Option<crate::catalog::CatalogReference>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct CableTrayRun {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub is_part_of: PmefId,
    pub tray_mark: String,
    #[serde(default, skip_serializing_if="Option::is_none")] pub tray_type: Option<String>,
    /// [mm]
    #[serde(default, skip_serializing_if="Option::is_none")] pub width: Option<f64>,
    /// Side-rail height [mm].
    #[serde(default, skip_serializing_if="Option::is_none")] pub height: Option<f64>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub material: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub fill_level:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")] pub reserve_capacity:Option<f64>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub structural_support_id: Option<PmefId>,
    /// Centreline start and end, bottom of the tray (RFC-0015) [mm].
    #[serde(default, skip_serializing_if="Option::is_none")] pub start_point: Option<Coordinate3D>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub end_point: Option<Coordinate3D>,
    /// Centreline polyline for runs that are not straight (RFC-0015) [mm].
    #[serde(default, skip_serializing_if="Vec::is_empty")] pub path: Vec<Coordinate3D>,
    /// Connection ends (RFC-0015).
    #[serde(default, skip_serializing_if="Vec::is_empty")] pub ports: Vec<TrayPort>,
    /// Source element type as `<tool>:<type>` (RFC-0015).
    #[serde(default, skip_serializing_if="Option::is_none")] pub native_type: Option<String>,
    /// RFC-0012 stand-in values.
    #[serde(default, skip_serializing_if="Vec::is_empty")] pub placeholders: Vec<crate::placeholder::Placeholder>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub geometry: Option<crate::geometry::GeometryReference>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

impl CableTrayRun {
    /// Centreline points: `path`, else `startPoint`/`endPoint`, else the port coordinates.
    pub fn centreline(&self) -> Vec<Coordinate3D> {
        if !self.path.is_empty() {
            return self.path.clone();
        }
        if let (Some(a), Some(b)) = (self.start_point, self.end_point) {
            return vec![a, b];
        }
        self.ports.iter().map(|p| p.coordinate).collect()
    }

    /// Centreline length [mm] (0 without a route).
    pub fn length_mm(&self) -> f64 {
        self.centreline().windows(2).map(|w| w[0].distance_to(&w[1])).sum()
    }
}

/// Connection end of a cable tray run or fitting (RFC-0015).
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct TrayPort {
    /// e.g. `P1`, `P2`, `BRANCH`.
    pub port_id: String,
    /// Centre of the tray bottom at this end [mm].
    pub coordinate: Coordinate3D,
    /// Pointing away from the run or fitting.
    #[serde(default, skip_serializing_if="Option::is_none")] pub direction: Option<crate::types::UnitVector3D>,
    /// [mm]
    #[serde(default, skip_serializing_if="Option::is_none")] pub width: Option<f64>,
    /// [mm]
    #[serde(default, skip_serializing_if="Option::is_none")] pub height: Option<f64>,
    /// Run or fitting joined at this end.
    #[serde(default, skip_serializing_if="Option::is_none")] pub connected_to: Option<PmefId>,
}

/// Bend, tee, cross, reducer, riser or end piece joining cable tray runs (RFC-0006).
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
#[allow(missing_docs)]
pub struct CableTrayFitting {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub is_part_of: PmefId,
    pub fitting_mark: String,
    /// HORIZONTAL_BEND, VERTICAL_BEND_UP/DOWN, TEE, CROSS, REDUCER, OFFSET, RISER, END_PIECE.
    pub fitting_type: String,
    #[serde(default, skip_serializing_if="Option::is_none")] pub tray_type:Option<String>,
    /// Main width [mm].
    pub width: f64,
    #[serde(default, skip_serializing_if="Option::is_none")] pub height:Option<f64>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub branch_width:Option<f64>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub angle:Option<f64>,
    /// Inner bend radius [mm].
    #[serde(default, skip_serializing_if="Option::is_none")] pub bend_radius:Option<f64>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub material:Option<String>,
    #[serde(default, skip_serializing_if="Vec::is_empty")] pub connected_tray_ids:Vec<PmefId>,
    /// Connection ends (RFC-0015).
    #[serde(default, skip_serializing_if="Vec::is_empty")] pub ports: Vec<TrayPort>,
    /// Source element type as `<tool>:<type>` (RFC-0015).
    #[serde(default, skip_serializing_if="Option::is_none")] pub native_type: Option<String>,
    /// RFC-0012 stand-in values.
    #[serde(default, skip_serializing_if="Vec::is_empty")] pub placeholders: Vec<crate::placeholder::Placeholder>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub catalog_ref:Option<crate::catalog::CatalogReference>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub geometry:Option<crate::geometry::GeometryReference>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct MtpModule {
    #[serde(rename="@type")] pub entity_type: String,
    #[serde(rename="@id")] pub id: PmefId,
    pub is_part_of: PmefId,
    pub module_name: String,
    pub mtp_version: String,
    pub pol_endpoint: String,
    pub member_ids: Vec<PmefId>,
    #[serde(skip_serializing_if="Option::is_none")] pub revision:Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_run_route_and_ports_round_trip() {
        let json = r#"{"@type":"pmef:CableTrayRun","@id":"urn:pmef:obj:demo-tray:CT-302","isPartOf":"urn:pmef:unit:demo-tray:U-300",
            "trayMark":"CT-302","trayType":"LADDER","width":400,"height":100,
            "path":[[6500,500,3000],[6500,4000,3000],[6500,7500,2800]],
            "ports":[{"portId":"P1","coordinate":[6500,500,3000],"direction":[0,-1,0],"width":400,"connectedTo":"urn:pmef:obj:demo-tray:CT-301-B01"}],
            "nativeType":"E3D:PIPE","customAttributes":{"e3dSpec":"/OE100SS"},
            "placeholders":[{"path":"/height","reason":"SOURCE_MISSING"}]}"#;
        let run: CableTrayRun = serde_json::from_str(json).unwrap();
        assert!((run.length_mm() - (3500.0 + (3500f64.powi(2) + 200f64.powi(2)).sqrt())).abs() < 1e-6);
        assert_eq!(run.ports[0].connected_to.as_ref().map(|i| i.as_str()), Some("urn:pmef:obj:demo-tray:CT-301-B01"));
        let back: serde_json::Value = serde_json::to_value(&run).unwrap();
        assert_eq!(back["placeholders"][0]["path"], "/height");
        assert_eq!(back["nativeType"], "E3D:PIPE");
        // straight run from start/end point, no path
        let s: CableTrayRun = serde_json::from_str(r#"{"@type":"pmef:CableTrayRun","@id":"urn:pmef:obj:x:CT-1","isPartOf":"urn:pmef:unit:x:U","trayMark":"CT-1","startPoint":[0,0,0],"endPoint":[6000,0,0]}"#).unwrap();
        assert_eq!(s.length_mm(), 6000.0);
    }
}
