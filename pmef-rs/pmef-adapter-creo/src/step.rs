//! STEP AP214 / AP242 bounding box and product extractor.
//!
//! Creo exports geometry as STEP (ISO 10303) files. This module extracts:
//! - Product names and descriptions (PRODUCT_DEFINITION)
//! - Bounding boxes (ADVANCED_BREP_SHAPE_REPRESENTATION → min/max coordinates)
//! - Named coordinate systems (AXIS2_PLACEMENT_3D → nozzle positions)
//!
//! Full B-Rep parsing is not implemented here — that requires a dedicated
//! STEP kernel. This module provides:
//! 1. Fast bounding box estimation from CARTESIAN_POINT instances
//! 2. Named entity extraction for product identification
//! 3. Coordinate system extraction for nozzle positions

use thiserror::Error;
use std::collections::HashMap;

#[derive(Debug, Error)]
pub enum StepError {
    #[error("STEP parse error on line {line}: {msg}")]
    Parse { line: usize, msg: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A STEP entity (single record from the DATA section).
#[derive(Debug, Clone)]
pub struct StepEntity {
    /// Entity instance number (#N).
    pub id: u64,
    /// Entity type name (e.g. `"CARTESIAN_POINT"`, `"PRODUCT"`).
    pub type_name: String,
    /// Raw attribute string (comma-separated, not fully parsed).
    pub attributes: String,
}

/// Extracted bounding box from STEP CARTESIAN_POINT instances.
#[derive(Debug, Clone)]
pub struct StepBbox {
    pub x_min: f64, pub x_max: f64,
    pub y_min: f64, pub y_max: f64,
    pub z_min: f64, pub z_max: f64,
}

impl StepBbox {
    pub fn empty() -> Self {
        Self {
            x_min: f64::INFINITY,  x_max: f64::NEG_INFINITY,
            y_min: f64::INFINITY,  y_max: f64::NEG_INFINITY,
            z_min: f64::INFINITY,  z_max: f64::NEG_INFINITY,
        }
    }

    pub fn extend(&mut self, x: f64, y: f64, z: f64) {
        self.x_min = self.x_min.min(x); self.x_max = self.x_max.max(x);
        self.y_min = self.y_min.min(y); self.y_max = self.y_max.max(y);
        self.z_min = self.z_min.min(z); self.z_max = self.z_max.max(z);
    }

    pub fn is_valid(&self) -> bool {
        self.x_min.is_finite() && self.x_max.is_finite()
    }

    pub fn volume(&self) -> f64 {
        (self.x_max - self.x_min).max(0.0) *
        (self.y_max - self.y_min).max(0.0) *
        (self.z_max - self.z_min).max(0.0)
    }
}

/// Product record extracted from STEP.
#[derive(Debug, Clone)]
pub struct StepProduct {
    pub id: u64,
    pub product_id: String,
    pub name: String,
    pub description: String,
}

/// Named axis placement (coordinate system) from STEP.
#[derive(Debug, Clone)]
pub struct StepAxisPlacement {
    pub name: String,
    /// Origin [x, y, z].
    pub origin: [f64; 3],
    /// Z-axis direction (normal).
    pub z_axis: [f64; 3],
    /// X-axis direction (reference).
    pub x_axis: [f64; 3],
}

/// Results from parsing a STEP file.
#[derive(Debug, Default)]
pub struct StepParseResult {
    pub bbox: Option<StepBbox>,
    pub products: Vec<StepProduct>,
    pub axis_placements: Vec<StepAxisPlacement>,
    pub point_count: usize,
}

/// Parse a STEP file content (text format — ISO 10303-21).
///
/// Extracts bounding box from all CARTESIAN_POINT instances,
/// product names from PRODUCT entities, and named coordinate systems.
///
/// # Limitations
/// - Does not resolve B-Rep topology (no FACE/EDGE traversal)
/// - Bounding box is a conservative estimate (includes all control points,
///   not the exact solid boundary)
/// - Multi-line entities not supported (entities must be on a single line)
pub fn parse_step_file(content: &str) -> Result<StepParseResult, StepError> {
    let mut result = StepParseResult::default();
    let mut bbox = StepBbox::empty();
    let mut in_data = false;

    // Named coordinate systems: name → (origin, z-axis, x-axis)
    let mut axis_names: HashMap<u64, String> = HashMap::new();
    let mut axis_origins: HashMap<u64, u64> = HashMap::new();
    let mut axis_zdirs: HashMap<u64, u64> = HashMap::new();
    let mut axis_xdirs: HashMap<u64, u64> = HashMap::new();
    let mut cartesian_points: HashMap<u64, [f64; 3]> = HashMap::new();
    let mut directions: HashMap<u64, [f64; 3]> = HashMap::new();

    for (ln, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("/*") { continue; }

        if line == "DATA;" { in_data = true; continue; }
        if line == "ENDSEC;" { in_data = false; continue; }
        if !in_data { continue; }

        // Parse entity: #N=TYPE(attributes);
        if let Some(entity) = parse_step_entity(line) {
            match entity.type_name.as_str() {
                "CARTESIAN_POINT" => {
                    if let Some(pt) = extract_cartesian_point(&entity.attributes) {
                        bbox.extend(pt[0], pt[1], pt[2]);
                        cartesian_points.insert(entity.id, pt);
                        result.point_count += 1;
                    }
                }
                "DIRECTION" => {
                    if let Some(dir) = extract_direction(&entity.attributes) {
                        directions.insert(entity.id, dir);
                    }
                }
                "AXIS2_PLACEMENT_3D" => {
                    // AXIS2_PLACEMENT_3D('name', origin_ref, z_ref, x_ref)
                    let attrs = &entity.attributes;
                    let name = extract_string(attrs).unwrap_or_default();
                    let refs = extract_entity_refs(attrs);
                    axis_names.insert(entity.id, name);
                    if let Some(&origin_ref) = refs.get(0) {
                        axis_origins.insert(entity.id, origin_ref);
                    }
                    if let Some(&z_ref) = refs.get(1) {
                        axis_zdirs.insert(entity.id, z_ref);
                    }
                    if let Some(&x_ref) = refs.get(2) {
                        axis_xdirs.insert(entity.id, x_ref);
                    }
                }
                "PRODUCT" => {
                    // PRODUCT('id','name','description',...)
                    let strings = extract_all_strings(&entity.attributes);
                    if strings.len() >= 2 {
                        result.products.push(StepProduct {
                            id: entity.id,
                            product_id: strings[0].clone(),
                            name: strings[1].clone(),
                            description: strings.get(2).cloned().unwrap_or_default(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    // Resolve axis placements
    for (&axis_id, name) in &axis_names {
        if name.is_empty() { continue; }
        let origin = axis_origins.get(&axis_id)
            .and_then(|r| cartesian_points.get(r))
            .copied()
            .unwrap_or([0.0; 3]);
        let z_axis = axis_zdirs.get(&axis_id)
            .and_then(|r| directions.get(r))
            .copied()
            .unwrap_or([0.0, 0.0, 1.0]);
        let x_axis = axis_xdirs.get(&axis_id)
            .and_then(|r| directions.get(r))
            .copied()
            .unwrap_or([1.0, 0.0, 0.0]);

        result.axis_placements.push(StepAxisPlacement {
            name: name.clone(),
            origin, z_axis, x_axis,
        });
    }

    if bbox.is_valid() {
        result.bbox = Some(bbox);
    }

    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn parse_step_entity(line: &str) -> Option<StepEntity> {
    // Format: #N=TYPE_NAME(attributes);
    if !line.starts_with('#') { return None; }
    let line = line.trim_end_matches(';');
    let eq_pos = line.find('=')?;
    let id_str = &line[1..eq_pos];
    let id: u64 = id_str.parse().ok()?;
    let rest = &line[eq_pos+1..];
    let paren_pos = rest.find('(')?;
    let type_name = rest[..paren_pos].trim().to_uppercase();
    let attrs = rest[paren_pos+1..].trim_end_matches(')').to_owned();
    Some(StepEntity { id, type_name, attributes: attrs })
}

fn extract_cartesian_point(attrs: &str) -> Option<[f64; 3]> {
    // CARTESIAN_POINT('name',(x,y,z)) or CARTESIAN_POINT('name',x,y,z)
    // Find parenthesised tuple or comma-separated numbers
    let nums: Vec<f64> = extract_floats(attrs);
    if nums.len() >= 3 { Some([nums[0], nums[1], nums[2]]) } else { None }
}

fn extract_direction(attrs: &str) -> Option<[f64; 3]> {
    let nums = extract_floats(attrs);
    if nums.len() >= 3 { Some([nums[0], nums[1], nums[2]]) } else { None }
}

fn extract_floats(s: &str) -> Vec<f64> {
    // Extract all floating-point numbers from a string
    let mut result = Vec::new();
    let mut current = String::new();
    for c in s.chars() {
        match c {
            '0'..='9' | '.' | '-' | 'E' | 'e' | '+' => current.push(c),
            _ => {
                if !current.is_empty() {
                    if let Ok(v) = current.parse::<f64>() { result.push(v); }
                    current.clear();
                }
            }
        }
    }
    if let Ok(v) = current.parse::<f64>() { result.push(v); }
    result
}

fn extract_entity_refs(attrs: &str) -> Vec<u64> {
    let mut refs = Vec::new();
    let mut i = 0;
    let bytes = attrs.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'#' {
            let start = i + 1;
            let end = bytes[start..].iter()
                .position(|&b| !b.is_ascii_digit())
                .map(|p| start + p)
                .unwrap_or(bytes.len());
            if let Ok(id) = attrs[start..end].parse::<u64>() {
                refs.push(id);
            }
            i = end;
        } else {
            i += 1;
        }
    }
    refs
}

fn extract_string(attrs: &str) -> Option<String> {
    // Find first single-quoted string
    let start = attrs.find('\'')?;
    let inner = &attrs[start+1..];
    let end = inner.find('\'')?;
    Some(inner[..end].to_owned())
}

fn extract_all_strings(attrs: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut remaining = attrs;
    loop {
        match remaining.find('\'') {
            None => break,
            Some(start) => {
                let inner = &remaining[start+1..];
                match inner.find('\'') {
                    None => break,
                    Some(end) => {
                        result.push(inner[..end].to_owned());
                        remaining = &inner[end+1..];
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_STEP: &str = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('Creo export'),'2;1');
ENDSEC;
DATA;
#1=PRODUCT('P-201A','Centrifugal Pump P-201A','Cooling water pump',());
#10=CARTESIAN_POINT('',(-100.0,50.0,850.0));
#11=CARTESIAN_POINT('',(1200.0,600.0,1800.0));
#12=CARTESIAN_POINT('',(500.0,300.0,1200.0));
#20=DIRECTION('',(-1.0,0.0,0.0));
#30=CARTESIAN_POINT('NOZZLE_N1',(200.0,400.0,900.0));
#31=DIRECTION('NOZZLE_N1_Z',(-1.0,0.0,0.0));
#32=DIRECTION('NOZZLE_N1_X',(0.0,0.0,1.0));
#40=AXIS2_PLACEMENT_3D('CS_NOZZLE_N1',#30,#31,#32);
ENDSEC;
END-ISO-10303-21;
"#;

    #[test]
    fn test_parse_step_products() {
        let result = parse_step_file(SAMPLE_STEP).unwrap();
        assert_eq!(result.products.len(), 1);
        assert_eq!(result.products[0].product_id, "P-201A");
        assert_eq!(result.products[0].name, "Centrifugal Pump P-201A");
    }

    #[test]
    fn test_parse_step_bbox() {
        let result = parse_step_file(SAMPLE_STEP).unwrap();
        let bbox = result.bbox.unwrap();
        assert!(bbox.is_valid());
        assert!((bbox.x_min - (-100.0)).abs() < 0.001);
        assert!((bbox.x_max - 1200.0).abs() < 0.001);
        assert!((bbox.y_min - 50.0).abs() < 0.001);
        assert!((bbox.z_max - 1800.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_step_axis_placement() {
        let result = parse_step_file(SAMPLE_STEP).unwrap();
        let nozzle_axis = result.axis_placements.iter()
            .find(|a| a.name == "CS_NOZZLE_N1");
        assert!(nozzle_axis.is_some());
        let noz = nozzle_axis.unwrap();
        assert!((noz.origin[0] - 200.0).abs() < 0.001);
        assert!((noz.origin[2] - 900.0).abs() < 0.001);
        assert!((noz.z_axis[0] - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_parse_step_point_count() {
        let result = parse_step_file(SAMPLE_STEP).unwrap();
        assert!(result.point_count >= 4); // 4 CARTESIAN_POINTs
    }

    #[test]
    fn test_step_bbox_empty() {
        let bb = StepBbox::empty();
        assert!(!bb.is_valid());
    }

    #[test]
    fn test_step_bbox_extend() {
        let mut bb = StepBbox::empty();
        bb.extend(0., 0., 0.);
        bb.extend(100., 200., 300.);
        assert!(bb.is_valid());
        assert!((bb.x_max - 100.).abs() < 0.001);
        assert!((bb.volume() - 6_000_000.).abs() < 1.0);
    }

    #[test]
    fn test_extract_floats() {
        let floats = extract_floats("('name',(1.5,2.0,-3.14))");
        assert!(floats.contains(&1.5));
        assert!(floats.contains(&2.0));
        assert!(floats.iter().any(|&v| (v - (-3.14)).abs() < 0.001));
    }
}
