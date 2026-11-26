//! HVAC domain (RFC-0006): air duct networks, duct components and their cross-sections.
//!
//! Mirrors `schemas/pmef-hvac.schema.json`: `DuctNetworkSystem` → `DuctSegment` → duct
//! components (`Duct`, `DuctBend`, `DuctTransition`, `DuctBranch`, `DuctDamper`,
//! `DuctAccessDoor`, `DuctTestPort`). Lengths in mm, angles in degrees, air flow in m³/h,
//! pressure in Pa.

use crate::catalog::{CatalogReference, DocumentLink};
use crate::geometry::GeometryReference;
use crate::revision::RevisionMetadata;
use crate::types::{Coordinate3D, Iec81346Designation, PmefId, RdlUri, UnitVector3D};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::PI;

/// Cross-section shape of a duct or duct port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DuctShape {
    /// Circular, `diameter`.
    Round,
    /// Square, `width` (= height).
    Square,
    /// Rectangular, `width` × `height`.
    Rectangular,
    /// Flat oval: two semicircles of diameter `height` joined by straight sides; `width` is the
    /// major axis.
    FlatOval,
}

/// Inside cross-section of a duct [mm].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct DuctCrossSection {
    pub shape: DuctShape,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diameter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
}

impl DuctCrossSection {
    /// A round section of diameter `d` [mm].
    pub fn round(d: f64) -> Self {
        Self { shape: DuctShape::Round, diameter: Some(d), width: None, height: None }
    }

    /// A rectangular section `w` × `h` [mm].
    pub fn rectangular(w: f64, h: f64) -> Self {
        Self { shape: DuctShape::Rectangular, diameter: None, width: Some(w), height: Some(h) }
    }

    /// Width and height of the section [mm]; `None` if a dimension the shape needs is missing.
    fn dims(&self) -> Option<(f64, f64)> {
        match self.shape {
            DuctShape::Round => self.diameter.map(|d| (d, d)),
            DuctShape::Square => self.width.map(|w| (w, self.height.unwrap_or(w))),
            DuctShape::Rectangular | DuctShape::FlatOval => Some((self.width?, self.height?)),
        }
    }

    /// Inside area [mm²].
    pub fn area_mm2(&self) -> Option<f64> {
        let (a, b) = self.dims()?;
        Some(match self.shape {
            DuctShape::Round => PI * a * a / 4.0,
            DuctShape::Square | DuctShape::Rectangular => a * b,
            DuctShape::FlatOval => PI * b * b / 4.0 + b * (a - b),
        })
    }

    /// Inside perimeter [mm].
    pub fn perimeter_mm(&self) -> Option<f64> {
        let (a, b) = self.dims()?;
        Some(match self.shape {
            DuctShape::Round => PI * a,
            DuctShape::Square | DuctShape::Rectangular => 2.0 * (a + b),
            DuctShape::FlatOval => PI * b + 2.0 * (a - b),
        })
    }

    /// Hydraulic diameter 4·A/P [mm], used for pressure-drop calculations.
    pub fn hydraulic_diameter_mm(&self) -> Option<f64> {
        Some(4.0 * self.area_mm2()? / self.perimeter_mm()?)
    }

    /// Mean air velocity [m/s] for a volume flow [m³/h].
    pub fn velocity_m_s(&self, airflow_m3_h: f64) -> Option<f64> {
        Some(airflow_m3_h / 3600.0 / (self.area_mm2()? * 1e-6))
    }
}

/// A connection end of a duct component.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct DuctPort {
    pub port_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_type: Option<String>,
    pub coordinate: Coordinate3D,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<UnitVector3D>,
    pub cross_section: DuctCrossSection,
    /// FLANGED, WELDED, SLIP_JOINT, SOCKET_SPIGOT, CLAMPED or PLAIN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flange_profile: Option<String>,
    /// `@id` of the adjacent component (optionally `#portId`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_to: Option<String>,
}

/// Design conditions of a duct system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct DuctDesignConditions {
    /// [m³/h]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub airflow_rate: Option<f64>,
    /// [Pa] relative to ambient.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_pressure: Option<f64>,
    /// [°C]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_temperature: Option<f64>,
    /// [m/s]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_velocity: Option<f64>,
    /// EN 1507 / EN 12237 class A–D.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air_tightness_class: Option<String>,
}

/// Material and construction of a duct component.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct DuctSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_grade: Option<String>,
    /// [mm]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet_thickness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard: Option<String>,
    /// [kg]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

/// Insulation of a duct component.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct DuctInsulation {
    /// THERMAL, CONDENSATION, ACOUSTIC or FIRE.
    pub insulation_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// [mm]
    pub thickness: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cladding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire_rating: Option<String>,
}

/// A complete air distribution system (supply, extract, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct DuctNetworkSystem {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub pmef_version: String,
    pub is_part_of: PmefId,
    pub system_number: String,
    /// EN 16798-3 air type: SUPPLY, EXTRACT, OUTDOOR, EXHAUST, …
    pub air_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_conditions: Option<DuctDesignConditions>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<PmefId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iec81346: Option<Iec81346Designation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rdl_type: Option<RdlUri>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub documents: Vec<DocumentLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
}

/// A contiguous run of a duct system with uniform specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct DuctSegment {
    #[serde(rename = "@type")]
    pub entity_type: String,
    #[serde(rename = "@id")]
    pub id: PmefId,
    pub is_part_of: PmefId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duct_spec: Option<DuctSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insulation: Option<DuctInsulation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<PmefId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<RevisionMetadata>,
    /// RFC-0001 provenance / review / asBuilt blocks.
    #[serde(flatten, default)]
    pub recognition: crate::provenance::RecognitionMetadata,
}

macro_rules! duct_component {
    ($(#[$meta:meta])* $name:ident { $($(#[$field_meta:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(missing_docs)]
        pub struct $name {
            #[serde(rename = "@type")]
            pub entity_type: String,
            #[serde(rename = "@id")]
            pub id: PmefId,
            pub pmef_version: String,
            pub is_part_of: PmefId,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub tag_number: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub item_number: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub duct_spec: Option<DuctSpec>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub insulation: Option<DuctInsulation>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub catalog_ref: Option<CatalogReference>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub iec81346: Option<Iec81346Designation>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub rdl_type: Option<RdlUri>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub ports: Vec<DuctPort>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub geometry: Option<GeometryReference>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub documents: Vec<DocumentLink>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub revision: Option<RevisionMetadata>,
            /// RFC-0001 provenance / review / asBuilt blocks.
            #[serde(flatten, default)]
            pub recognition: crate::provenance::RecognitionMetadata,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub custom_attributes: Option<HashMap<String, serde_json::Value>>,
            $( $(#[$field_meta])* pub $field: $ty ),*
        }
    };
}

duct_component!(
    /// Straight duct.
    Duct {
        /// [mm]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<f64>,
    }
);

duct_component!(
    /// Duct bend / elbow.
    DuctBend {
        /// [°]
        angle: f64,
        /// RADIUS, SEGMENTED or MITRED.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bend_type: Option<String>,
        /// Centreline radius [mm].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radius: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        segments: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turning_vanes: Option<bool>,
        /// HARD or EASY (rectangular ducts).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bend_plane: Option<String>,
    }
);

duct_component!(
    /// Change of cross-section size and/or shape; the two ports carry the two sections.
    DuctTransition {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transition_type: Option<String>,
        /// [mm]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<f64>,
    }
);

duct_component!(
    /// T-piece, lateral, Y-piece, cross or saddle tap.
    DuctBranch {
        branch_type: String,
        /// [°]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        branch_angle: Option<f64>,
    }
);

duct_component!(
    /// Volume-control, shut-off, non-return, fire or smoke damper.
    DuctDamper {
        damper_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actuation: Option<String>,
        /// EN 13501-3 classification, e.g. `EI 90 S`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fire_resistance: Option<String>,
        /// [mm]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instrument_id: Option<PmefId>,
    }
);

duct_component!(
    /// Inspection or cleaning opening mounted on a host duct component.
    DuctAccessDoor {
        door_type: String,
        host_id: PmefId,
        position: Coordinate3D,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        normal: Option<UnitVector3D>,
        /// [mm]
        opening_width: f64,
        /// [mm]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opening_height: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        insulated: Option<bool>,
    }
);

duct_component!(
    /// Measuring socket for pressure, flow or temperature, mounted on a host duct component.
    DuctTestPort {
        purpose: String,
        host_id: PmefId,
        position: Coordinate3D,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        normal: Option<UnitVector3D>,
        /// [mm]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port_diameter: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instrument_id: Option<PmefId>,
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn cross_section_areas_and_hydraulic_diameter() {
        let r = DuctCrossSection::round(400.0);
        close(r.area_mm2().unwrap(), PI * 40_000.0);
        close(r.hydraulic_diameter_mm().unwrap(), 400.0);

        let s = DuctCrossSection { shape: DuctShape::Square, diameter: None, width: Some(400.0), height: None };
        close(s.area_mm2().unwrap(), 160_000.0);
        close(s.hydraulic_diameter_mm().unwrap(), 400.0);

        let q = DuctCrossSection::rectangular(300.0, 200.0);
        close(q.hydraulic_diameter_mm().unwrap(), 4.0 * 60_000.0 / 1000.0);

        // flat oval 400 x 150: two half circles of 150 plus a 250 x 150 rectangle
        let o = DuctCrossSection { shape: DuctShape::FlatOval, diameter: None, width: Some(400.0), height: Some(150.0) };
        close(o.area_mm2().unwrap(), PI * 150.0 * 150.0 / 4.0 + 150.0 * 250.0);
        close(o.perimeter_mm().unwrap(), PI * 150.0 + 500.0);

        // a flat oval with equal axes is a circle
        let c = DuctCrossSection { shape: DuctShape::FlatOval, diameter: None, width: Some(200.0), height: Some(200.0) };
        close(c.area_mm2().unwrap(), DuctCrossSection::round(200.0).area_mm2().unwrap());

        let missing = DuctCrossSection { shape: DuctShape::Rectangular, diameter: None, width: Some(300.0), height: None };
        assert!(missing.area_mm2().is_none());
    }

    #[test]
    fn velocity_from_airflow() {
        // 3600 m³/h through 0.16 m² = 6.25 m/s
        let s = DuctCrossSection::rectangular(400.0, 400.0);
        close(s.velocity_m_s(3600.0).unwrap(), 6.25);
    }

    #[test]
    fn transition_round_to_square_round_trips() {
        let line = r#"{"@type":"pmef:DuctTransition","@id":"urn:pmef:obj:demo:T1","pmefVersion":"0.10.0","isPartOf":"urn:pmef:seg:demo:S1","ports":[{"portId":"P1","coordinate":[0.0,0.0,0.0],"crossSection":{"shape":"ROUND","diameter":400.0},"connectionType":"SOCKET_SPIGOT"},{"portId":"P2","coordinate":[0.0,500.0,0.0],"crossSection":{"shape":"SQUARE","width":400.0},"connectionType":"FLANGED","flangeProfile":"MEZ 20"}],"transitionType":"CONCENTRIC","length":500.0}"#;
        let t: DuctTransition = serde_json::from_str(line).unwrap();
        assert_eq!(t.ports[0].cross_section.shape, DuctShape::Round);
        assert_eq!(t.ports[1].cross_section.shape, DuctShape::Square);
        let back: serde_json::Value = serde_json::to_value(&t).unwrap();
        assert_eq!(back, serde_json::from_str::<serde_json::Value>(line).unwrap());
    }
}
