//! PMEF geometry: bounding boxes and clash detection.
#[derive(Debug,Clone,Copy,PartialEq)]
pub struct Vec3 { pub x:f64, pub y:f64, pub z:f64 }
impl Vec3 {
    pub fn new(x:f64,y:f64,z:f64)->Self{Self{x,y,z}}
    pub fn distance_to(&self,o:&Self)->f64{let dx=self.x-o.x;let dy=self.y-o.y;let dz=self.z-o.z;(dx*dx+dy*dy+dz*dz).sqrt()}
}
#[derive(Debug,Clone,PartialEq)]
pub struct Aabb{pub min:Vec3,pub max:Vec3}
impl Aabb{
    pub fn new(min:Vec3,max:Vec3)->Self{Self{min,max}}
    pub fn intersects(&self,o:&Aabb)->bool{self.min.x<=o.max.x&&self.max.x>=o.min.x&&self.min.y<=o.max.y&&self.max.y>=o.min.y&&self.min.z<=o.max.z&&self.max.z>=o.min.z}
}
pub fn cylinder_aabb(center:Vec3,axis:[f64;3],radius:f64,length:f64)->Aabb{
    let end=Vec3::new(center.x+axis[0]*length,center.y+axis[1]*length,center.z+axis[2]*length);
    let ex=radius*(1.0-axis[0]*axis[0]).sqrt();let ey=radius*(1.0-axis[1]*axis[1]).sqrt();let ez=radius*(1.0-axis[2]*axis[2]).sqrt();
    Aabb::new(Vec3::new(center.x.min(end.x)-ex,center.y.min(end.y)-ey,center.z.min(end.z)-ez),Vec3::new(center.x.max(end.x)+ex,center.y.max(end.y)+ey,center.z.max(end.z)+ez))
}
