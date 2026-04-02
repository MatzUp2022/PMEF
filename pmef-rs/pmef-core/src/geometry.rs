//! Geometry reference and LOD types.
use serde::{Deserialize, Serialize};
use crate::types::PmefId;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct GeometryReference {
    #[serde(rename="type")]
    pub layer: GeometryLayer,
    #[serde(skip_serializing_if="Option::is_none")]
    pub ref_uri: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub lod: Option<Lod>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub bounding_box: Option<BoundingBox>,
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
