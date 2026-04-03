//! Plant 3D IDF (Isometric Drawing File) parser.
//!
//! IDF is Plant 3D's native piping data exchange format — a text file
//! produced by the Isogen isometric generator. It contains more complete
//! data than PCF including weld numbers, material grades, and test data.
//!
//! IDF format is a superset of PCF with additional sections:
//! - PIPELINE-REFERENCE, SPOOL-REFERENCE
//! - COMPONENT-IDENTIFIER sections (with ITEM-CODE, MATERIAL)
//! - WELD-IDENTIFIER sections
//! - TEST-DATA section (test pressure, medium)
//! - ISOMETRIC-OPTION flags

use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdfError {
    #[error("Parse error on line {line}: {msg}")]
    Parse { line: usize, msg: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A parsed IDF file.
#[derive(Debug, Default)]
pub struct IdfFile {
    /// Units system (`"INCHES"` or `"MM"`).
    pub units: String,
    /// Full line number tag.
    pub pipeline_reference: String,
    /// Spool reference (if present).
    pub spool_reference: Option<String>,
    /// Test pressure [same units as bore].
    pub test_pressure: Option<f64>,
    /// Test medium.
    pub test_medium: Option<String>,
    /// Design pressure.
    pub design_pressure: Option<f64>,
    /// Design temperature.
    pub design_temperature: Option<f64>,
    /// All piping components.
    pub components: Vec<IdfComponent>,
    /// Weld records.
    pub welds: Vec<IdfWeld>,
}

/// A piping component in an IDF file.
#[derive(Debug, Clone)]
pub struct IdfComponent {
    /// PCF-style keyword (PIPE, ELBOW, VALVE, etc.).
    pub keyword: String,
    /// SKEY (8-char spec key).
    pub skey: Option<String>,
    /// Item code (catalog / BOM reference).
    pub item_code: Option<String>,
    /// Material grade.
    pub material: Option<String>,
    /// Connection end points [[x, y, z, bore], ...].
    pub end_points: Vec<[f64; 4]>,
    /// Tag number (for valves/instruments).
    pub tag_number: Option<String>,
    /// All other attributes.
    pub attrs: HashMap<String, String>,
}

impl IdfComponent {
    /// Get attribute value by keyword (case-insensitive).
    pub fn attr(&self, key: &str) -> Option<&str> {
        let upper = key.to_uppercase();
        self.attrs.get(&upper).map(|s| s.as_str())
    }

    /// Weight [same unit as input — typically lbs for US IDF].
    pub fn weight(&self) -> Option<f64> {
        self.attr("WEIGHT").and_then(|w| w.parse().ok())
    }
}

/// A weld record in an IDF file.
#[derive(Debug, Clone)]
pub struct IdfWeld {
    pub weld_number: String,
    pub weld_type: Option<String>,
    pub nde_method: Option<String>,
    pub pwht: bool,
    pub position: Option<[f64; 3]>,
}

/// Parse an IDF file from a string.
pub fn parse_idf(content: &str) -> Result<IdfFile, IdfError> {
    let mut idf = IdfFile::default();
    let mut current_comp: Option<IdfComponent> = None;
    let mut current_weld: Option<IdfWeld> = None;

    for (ln, raw_line) in content.lines().enumerate() {
        let line_num = ln + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('!') { continue; }

        let (kw, rest) = split_keyword(line);
        let kw_upper = kw.to_uppercase();

        match kw_upper.as_str() {
            // ── File-level attributes ─────────────────────────────────────────
            "UNITS-BORE" => {
                idf.units = rest.to_owned();
            }
            "PIPELINE-REFERENCE" => {
                idf.pipeline_reference = rest.to_owned();
            }
            "SPOOL-REFERENCE" => {
                idf.spool_reference = Some(rest.to_owned());
            }
            "TEST-PRESSURE" => {
                idf.test_pressure = rest.parse().ok();
            }
            "TEST-MEDIUM" => {
                idf.test_medium = Some(rest.to_owned());
            }
            "PIPELINE-DESIGN-PRESSURE" => {
                idf.design_pressure = rest.parse().ok();
            }
            "PIPELINE-DESIGN-TEMPERATURE" => {
                idf.design_temperature = rest.parse().ok();
            }

            // ── Component start ───────────────────────────────────────────────
            kw if is_component_keyword(kw) => {
                // Flush current
                flush_component(&mut current_comp, &mut idf.components);
                flush_weld(&mut current_weld, &mut idf.welds);
                current_comp = Some(IdfComponent {
                    keyword: kw_upper.to_owned(),
                    skey: None, item_code: None, material: None, tag_number: None,
                    end_points: Vec::new(),
                    attrs: HashMap::new(),
                });
            }

            // ── Weld start ────────────────────────────────────────────────────
            "WELD-IDENTIFIER" => {
                flush_component(&mut current_comp, &mut idf.components);
                flush_weld(&mut current_weld, &mut idf.welds);
                current_weld = Some(IdfWeld {
                    weld_number: rest.to_owned(),
                    weld_type: None, nde_method: None,
                    pwht: false, position: None,
                });
            }

            // ── Attributes ────────────────────────────────────────────────────
            "END-POINT" => {
                if let Some(ref mut comp) = current_comp {
                    if let Some(ep) = parse_endpoint(rest) {
                        comp.end_points.push(ep);
                    }
                }
            }
            "SKEY" => {
                if let Some(ref mut comp) = current_comp {
                    comp.skey = Some(rest.to_owned());
                }
            }
            "ITEM-CODE" => {
                if let Some(ref mut comp) = current_comp {
                    comp.item_code = Some(rest.to_owned());
                }
            }
            "MATERIAL-IDENTIFIER" | "MATERIAL" => {
                if let Some(ref mut comp) = current_comp {
                    comp.material = Some(rest.to_owned());
                }
            }
            "COMPONENT-TAG" | "ATTRIBUTE0" => {
                if let Some(ref mut comp) = current_comp {
                    comp.tag_number = Some(rest.to_owned());
                }
            }
            "WELD-TYPE" => {
                if let Some(ref mut w) = current_weld { w.weld_type = Some(rest.to_owned()); }
            }
            "NDE-METHOD" => {
                if let Some(ref mut w) = current_weld { w.nde_method = Some(rest.to_owned()); }
            }
            "PWHT" => {
                if let Some(ref mut w) = current_weld {
                    w.pwht = rest.to_uppercase() == "YES" || rest == "1";
                }
            }
            other => {
                // Store in attrs
                if let Some(ref mut comp) = current_comp {
                    comp.attrs.insert(other.to_owned(), rest.to_owned());
                }
            }
        }
    }

    flush_component(&mut current_comp, &mut idf.components);
    flush_weld(&mut current_weld, &mut idf.welds);

    Ok(idf)
}

fn split_keyword(line: &str) -> (&str, &str) {
    match line.find(' ') {
        Some(i) => (&line[..i], line[i+1..].trim()),
        None    => (line, ""),
    }
}

fn is_component_keyword(kw: &str) -> bool {
    matches!(kw,
        "PIPE" | "ELBOW" | "TEE" | "REDUCER-CONCENTRIC" | "REDUCER-ECCENTRIC" |
        "FLANGE" | "FLANGE-BLIND" | "VALVE" | "OLET" | "GASKET" |
        "SUPPORT" | "INSTRUMENT" | "BOLT-SET" | "SPECTACLE-BLIND"
    )
}

fn parse_endpoint(s: &str) -> Option<[f64; 4]> {
    let parts: Vec<f64> = s.split_whitespace()
        .filter_map(|p| p.parse().ok())
        .collect();
    if parts.len() >= 4 { Some([parts[0], parts[1], parts[2], parts[3]]) }
    else { None }
}

fn flush_component(current: &mut Option<IdfComponent>, list: &mut Vec<IdfComponent>) {
    if let Some(c) = current.take() { list.push(c); }
}
fn flush_weld(current: &mut Option<IdfWeld>, list: &mut Vec<IdfWeld>) {
    if let Some(w) = current.take() { list.push(w); }
}

/// Convert IDF bore value to mm.
pub fn idf_bore_to_mm(bore: f64, units: &str) -> f64 {
    if units.trim().eq_ignore_ascii_case("INCHES") { bore * 25.4 } else { bore }
}

/// Convert IDF coordinate value to mm.
pub fn idf_coord_to_mm(coord: f64, units: &str) -> f64 {
    if units.trim().eq_ignore_ascii_case("INCHES") { coord * 25.4 } else { coord }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_IDF: &str = r#"
UNITS-BORE INCHES
PIPELINE-REFERENCE 8"-CW-201-A1A2
PIPELINE-DESIGN-PRESSURE 217.6
PIPELINE-DESIGN-TEMPERATURE 140.0
TEST-PRESSURE 326.4
TEST-MEDIUM WATER
PIPE
    SKEY PIPW
    ITEM-CODE A106B-200-SCH40
    MATERIAL-IDENTIFIER A106B
    END-POINT 0.0 0.0 33.46 7.981
    END-POINT 98.43 0.0 33.46 7.981
ELBOW
    SKEY ELBWLR90
    MATERIAL-IDENTIFIER A234WPB
    END-POINT 98.43 0.0 33.46 7.981
    END-POINT 98.43 0.0 45.47 7.981
WELD-IDENTIFIER W001
    WELD-TYPE BW
    NDE-METHOD VT
    PWHT NO
"#;

    #[test]
    fn test_parse_idf_header() {
        let idf = parse_idf(SAMPLE_IDF).unwrap();
        assert_eq!(idf.units, "INCHES");
        assert_eq!(idf.pipeline_reference, "8\"-CW-201-A1A2");
        assert_eq!(idf.test_pressure, Some(326.4));
        assert_eq!(idf.test_medium, Some("WATER".to_owned()));
        assert_eq!(idf.design_pressure, Some(217.6));
        assert_eq!(idf.design_temperature, Some(140.0));
    }

    #[test]
    fn test_parse_idf_components() {
        let idf = parse_idf(SAMPLE_IDF).unwrap();
        assert_eq!(idf.components.len(), 2);
        assert_eq!(idf.components[0].keyword, "PIPE");
        assert_eq!(idf.components[0].skey, Some("PIPW".to_owned()));
        assert_eq!(idf.components[0].material, Some("A106B".to_owned()));
        assert_eq!(idf.components[0].end_points.len(), 2);
        assert_eq!(idf.components[1].keyword, "ELBOW");
    }

    #[test]
    fn test_parse_idf_welds() {
        let idf = parse_idf(SAMPLE_IDF).unwrap();
        assert_eq!(idf.welds.len(), 1);
        assert_eq!(idf.welds[0].weld_number, "W001");
        assert_eq!(idf.welds[0].weld_type, Some("BW".to_owned()));
        assert!(!idf.welds[0].pwht);
    }

    #[test]
    fn test_idf_bore_to_mm() {
        assert!((idf_bore_to_mm(7.981, "INCHES") - 202.72).abs() < 0.1);
        assert!((idf_bore_to_mm(200.0, "MM") - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_idf_coord_to_mm() {
        assert!((idf_coord_to_mm(98.43, "INCHES") - 2500.0).abs() < 1.0);
        // 98.43 in = 2500.1 mm
    }

    #[test]
    fn test_is_component_keyword() {
        assert!(is_component_keyword("PIPE"));
        assert!(is_component_keyword("ELBOW"));
        assert!(is_component_keyword("VALVE"));
        assert!(!is_component_keyword("WELD-IDENTIFIER"));
    }
}
