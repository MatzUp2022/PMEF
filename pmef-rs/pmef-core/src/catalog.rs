//! Catalog references, ports, and document links.

use crate::types::{Coordinate3D, PmefId, UnitVector3D};
use serde::{Deserialize, Serialize};

/// Link from a PMEF object to a catalog entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogReference {
    pub catalog_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rdl_type_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eclass_irdi: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub vendor_mappings: Vec<VendorMapping>,
}

/// Vendor-specific object ID for round-trip identification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorMapping {
    pub vendor_system: String,
    pub vendor_id: String,
}

/// Link from a PMEF object to a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentLink {
    pub document_id: String,
    pub document_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Physical connection point on a piping component.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Port {
    pub port_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_type: Option<String>,
    pub coordinate: Coordinate3D,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<UnitVector3D>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nominal_diameter: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_type: Option<String>,
    /// `@id` of the adjacent component (or `@id#portId` for specific port).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_to: Option<String>,
}

impl Port {
    /// Returns the connected object's `@id`, stripping any `#portId` fragment.
    pub fn connected_to_id(&self) -> Option<PmefId> {
        self.connected_to.as_ref().map(|s| {
            let id_str = s.split('#').next().unwrap_or(s);
            PmefId::new_unchecked(id_str)
        })
    }
}
