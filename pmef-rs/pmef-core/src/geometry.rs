//! Geometry reference and LOD types.
use serde::{Deserialize, Serialize};
use crate::types::PmefId;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct GeometryReference {
    #[serde(rename="type")]
    pub layer: GeometryLayer,
    #[serde(rename = "ref", skip_serializing_if="Option::is_none")]
    pub ref_uri: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub lod: Option<Lod>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub bounding_box: Option<BoundingBox>,
    /// Units, up axis and origin of a `mesh_ref` file (spec 04 §2.3); absent = glTF convention.
    #[serde(skip_serializing_if="Option::is_none")]
    pub mesh_frame: Option<MeshFrame>,
}

/// Frame of a referenced mesh file (spec 04 §2.3).
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct MeshFrame {
    /// `"m"` (glTF default) or `"mm"`.
    #[serde(skip_serializing_if="Option::is_none")]
    pub unit: Option<String>,
    /// `"Y"` (glTF default) or `"Z"`.
    #[serde(skip_serializing_if="Option::is_none")]
    pub up_axis: Option<String>,
    /// Project coordinates [mm] of the mesh origin.
    #[serde(skip_serializing_if="Option::is_none")]
    pub origin: Option<[f64; 3]>,
}

impl MeshFrame {
    /// Convert a mesh vertex to project coordinates [mm]. `None` frame = glTF convention.
    pub fn to_project(frame: Option<&MeshFrame>, p: [f64; 3]) -> [f64; 3] {
        let unit = frame.and_then(|f| f.unit.as_deref()).unwrap_or("m");
        let s = if unit == "mm" { 1.0 } else { 1000.0 };
        let z_up = frame.and_then(|f| f.up_axis.as_deref()) == Some("Z");
        let q = if z_up { [p[0], p[1], p[2]] } else { [p[0], -p[2], p[1]] };
        let o = frame.and_then(|f| f.origin).unwrap_or([0.0; 3]);
        [o[0] + q[0] * s, o[1] + q[1] * s, o[2] + q[2] * s]
    }
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum GeometryLayer { Parametric, MeshRef, StepRef, UsdRef, None }

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum Lod { BboxOnly, Lod1Coarse, Lod2Medium, Lod3Fine, Lod4Fabrication }

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct BoundingBox {
    pub x_min: f64, pub x_max: f64,
    pub y_min: f64, pub y_max: f64,
    pub z_min: f64, pub z_max: f64,
}
impl BoundingBox {
    pub fn contains(&self, x:f64, y:f64, z:f64) -> bool {
        x>=self.x_min&&x<=self.x_max&&y>=self.y_min&&y<=self.y_max&&z>=self.z_min&&z<=self.z_max
    }
    pub fn volume(&self) -> f64 {
        (self.x_max-self.x_min).max(0.)*(self.y_max-self.y_min).max(0.)*(self.z_max-self.z_min).max(0.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_ref_round_trip_and_frame() {
        let j = serde_json::json!({"type": "mesh_ref", "ref": "HA-PIPE.glb#/nodes/34;extras.RefNo=16791/39427",
            "lod": "LOD3_FINE", "meshFrame": {"unit": "m", "upAxis": "Z", "origin": [1000.0, 2000.0, 0.0]}});
        let g: GeometryReference = serde_json::from_value(j.clone()).unwrap();
        assert_eq!(serde_json::to_value(&g).unwrap(), j);
        let f = g.mesh_frame.as_ref();
        assert_eq!(MeshFrame::to_project(f, [1.0, 2.0, 3.0]), [2000.0, 4000.0, 3000.0]);
        // glTF default: metres, Y-up (spec 04 §5.3)
        assert_eq!(MeshFrame::to_project(None, [1.0, 2.0, 3.0]), [1000.0, -3000.0, 2000.0]);
    }
}
