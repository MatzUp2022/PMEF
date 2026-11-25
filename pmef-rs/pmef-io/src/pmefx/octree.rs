//! Loose octree over object bounding boxes (spec 03 §3.7).
//!
//! Each object is stored in exactly one node: the deepest node whose *cell* (regular octant
//! subdivision of the root box) fully contains the object's box. A node's `bbox` is the union
//! of all object boxes in its subtree, so it may extend beyond its cell. Construction is
//! deterministic (objects sorted by `@id`).

use super::model::{BBox, Octree, OctreeNode};

/// Default subdivision limits.
pub const DEFAULT_MAX_DEPTH: u32 = 8;
pub const DEFAULT_MAX_OBJECTS_PER_NODE: u32 = 64;

/// Builds an octree over `(id, bbox)` pairs. Returns `None` if there are no objects.
pub fn build(items: &[(String, BBox)], max_depth: u32, max_objects_per_node: u32) -> Option<Octree> {
    let mut items: Vec<&(String, BBox)> = items.iter().collect();
    if items.is_empty() {
        return None;
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let cell = items.iter().skip(1).fold(items[0].1, |acc, (_, b)| acc.union(b));
    let root = node(cell, items, 0, max_depth, max_objects_per_node as usize);
    Some(Octree { max_depth, max_objects_per_node, root })
}

fn octant(cell: &BBox, i: usize) -> BBox {
    let c = cell.center();
    let (x_min, x_max) = if i & 1 == 0 { (cell.x_min, c.x) } else { (c.x, cell.x_max) };
    let (y_min, y_max) = if i & 2 == 0 { (cell.y_min, c.y) } else { (c.y, cell.y_max) };
    let (z_min, z_max) = if i & 4 == 0 { (cell.z_min, c.z) } else { (c.z, cell.z_max) };
    BBox { x_min, x_max, y_min, y_max, z_min, z_max }
}

fn node(cell: BBox, items: Vec<&(String, BBox)>, depth: u32, max_depth: u32, max_per_node: usize) -> OctreeNode {
    let loose = items.iter().skip(1).fold(items[0].1, |acc, (_, b)| acc.union(b));
    let mut here: Vec<&(String, BBox)> = Vec::new();
    let mut buckets: [Vec<&(String, BBox)>; 8] = Default::default();
    if items.len() <= max_per_node || depth >= max_depth {
        here = items;
    } else {
        let cells: Vec<BBox> = (0..8).map(|i| octant(&cell, i)).collect();
        for it in items {
            match cells.iter().position(|c| c.contains(&it.1)) {
                Some(i) => buckets[i].push(it),
                None => here.push(it),
            }
        }
    }
    let mut children = Vec::new();
    for (i, bucket) in buckets.into_iter().enumerate() {
        if !bucket.is_empty() {
            children.push(node(octant(&cell, i), bucket, depth + 1, max_depth, max_per_node));
        }
    }
    OctreeNode { bbox: loose, objects: here.into_iter().map(|(id, _)| id.clone()).collect(), children }
}

/// Visits every node depth-first.
pub fn walk<'a>(node: &'a OctreeNode, f: &mut dyn FnMut(&'a OctreeNode)) {
    f(node);
    for c in &node.children {
        walk(c, f);
    }
}
