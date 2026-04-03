//! PML (Programmable Macro Language) data structures and text-format parser.
//!
//! AVEVA E3D exports semantic data via PML scripts as text files.
//! The canonical export format is a structured text with one attribute per line:
//!
//! ```text
//! PIPE /EAF-LINE-3/SITE01/ZONE-CW/PIPE-CW-201
//!   BORE    200
//!   LINREF  8"-CW-201-A1A2
//!   TEMP    333.15
//!   PRES    1600000
//!   SPEC    /SPEC-A1A2
//!   DTXR    CW
//!
//!   BRAN /EAF-LINE-3/SITE01/ZONE-CW/PIPE-CW-201/BRAN1
//!     ELBOW
//!       ANGL    90.0
//!       RBOR    200
//!       RADI    304.8
//!       POS     11500.0 5400.0 850.0
//!       CONN    /PIPE-CW-201/BRAN1/PIPE1
//! ```

use std::collections::HashMap;
use thiserror::Error;

/// Errors during PML parsing.
#[derive(Debug, Error)]
pub enum PmlError {
    #[error("Parse error on line {line}: {msg}")]
    Parse { line: usize, msg: String },
    #[error("Required attribute '{attr}' missing on {element}")]
    MissingAttr { attr: &'static str, element: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ── PML value types ───────────────────────────────────────────────────────────

/// A scalar or vector PML attribute value.
#[derive(Debug, Clone, PartialEq)]
pub enum PmlValue {
    Text(String),
    Float(f64),
    Int(i64),
    Bool(bool),
    /// 3-component vector (E3D X Y Z).
    Vec3(f64, f64, f64),
    /// 3-component orientation (E3D direction vector).
    Dir3(f64, f64, f64),
    /// E3D database address: `/SITE/ZONE/ELEM/...`
    DbAddr(String),
}

impl PmlValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(s) | Self::DbAddr(s) => Some(s.as_str()),
            _ => None,
        }
    }
    pub fn as_vec3(&self) -> Option<(f64, f64, f64)> {
        match self {
            Self::Vec3(x, y, z) | Self::Dir3(x, y, z) => Some((*x, *y, *z)),
            _ => None,
        }
    }
}

// ── PML element types ────────────────────────────────────────────────────────

/// An E3D database element type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum E3dElementType {
    Site,
    Zone,
    /// Piping line (PIPE).
    Pipe,
    /// Piping branch (BRAN).
    Branch,
    /// Equipment element (EQUI).
    Equipment,
    /// Nozzle element (NOZZ).
    Nozzle,
    /// Straight pipe (PIPE within a branch).
    StraightPipe,
    /// Elbow (ELBOW).
    Elbow,
    /// Tee (TEE).
    Tee,
    /// Reducer (REDU).
    Reducer,
    /// Flange (FLAN).
    Flange,
    /// Valve (VALV).
    Valve,
    /// Gasket (GASK).
    Gasket,
    /// Weld (WELD).
    Weld,
    /// Pipe support (PSUP).
    PipeSupport,
    /// Cable tray (CTRAY).
    CableTray,
    /// Structural member (SREF / STRU).
    Structure,
    /// Unknown element type.
    Unknown(String),
}

impl E3dElementType {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "SITE"  => Self::Site,
            "ZONE"  => Self::Zone,
            "PIPE"  => Self::Pipe,
            "BRAN"  => Self::Branch,
            "EQUI"  => Self::Equipment,
            "NOZZ"  => Self::Nozzle,
            "STPIPE"| "STRAIG" => Self::StraightPipe,
            "ELBOW" => Self::Elbow,
            "TEE"   => Self::Tee,
            "REDU"  => Self::Reducer,
            "FLAN"  => Self::Flange,
            "VALV"  => Self::Valve,
            "GASK"  => Self::Gasket,
            "WELD"  => Self::Weld,
            "PSUP"  => Self::PipeSupport,
            "CTRAY" => Self::CableTray,
            "STRU" | "SREF" => Self::Structure,
            other   => Self::Unknown(other.to_owned()),
        }
    }

    /// Returns the PMEF `@type` for this element type, if known.
    pub fn pmef_type(&self) -> Option<&'static str> {
        match self {
            Self::Pipe        => Some("pmef:PipingNetworkSystem"),
            Self::Branch      => Some("pmef:PipingSegment"),
            Self::StraightPipe=> Some("pmef:Pipe"),
            Self::Elbow       => Some("pmef:Elbow"),
            Self::Tee         => Some("pmef:Tee"),
            Self::Reducer     => Some("pmef:Reducer"),
            Self::Flange      => Some("pmef:Flange"),
            Self::Valve       => Some("pmef:Valve"),
            Self::Gasket      => Some("pmef:Gasket"),
            Self::Weld        => Some("pmef:Weld"),
            Self::PipeSupport => Some("pmef:PipeSupport"),
            Self::Equipment   => Some("pmef:GenericEquipment"),
            Self::Nozzle      => None, // embedded in equipment
            Self::Structure   => Some("pmef:SteelMember"),
            _                 => None,
        }
    }
}

// ── PML element ──────────────────────────────────────────────────────────────

/// A parsed PML element with its attributes and child elements.
#[derive(Debug, Clone)]
pub struct PmlElement {
    /// E3D element type.
    pub element_type: E3dElementType,
    /// Full E3D database address (e.g. `/SITE01/ZONE-CW/PIPE-CW-201`).
    pub db_address: String,
    /// Attributes keyed by PML keyword (uppercase).
    pub attributes: HashMap<String, PmlValue>,
    /// Child elements.
    pub children: Vec<PmlElement>,
}

impl PmlElement {
    pub fn new(element_type: E3dElementType, db_address: String) -> Self {
        Self {
            element_type,
            db_address,
            attributes: HashMap::new(),
            children: Vec::new(),
        }
    }

    /// Get an attribute value by keyword.
    pub fn attr(&self, key: &str) -> Option<&PmlValue> {
        self.attributes.get(&key.to_uppercase())
    }

    /// Get an attribute as f64.
    pub fn attr_f64(&self, key: &str) -> Option<f64> {
        self.attr(key)?.as_f64()
    }

    /// Get an attribute as string.
    pub fn attr_str(&self, key: &str) -> Option<&str> {
        self.attr(key)?.as_str()
    }

    /// The local name (last component of the DB address).
    pub fn local_name(&self) -> &str {
        self.db_address.rsplit('/').next().unwrap_or(&self.db_address)
    }
}

// ── PML text parser ──────────────────────────────────────────────────────────

/// Parse a PML export text file into a tree of [`PmlElement`]s.
///
/// The PML export format uses:
/// - `TYPE /DB/ADDRESS` — element declaration (starts a new element)
/// - `  ATTR VALUE`     — attribute assignment (2-space indent per level)
/// - Blank lines are separators
///
/// # Example
/// ```
/// use pmef_adapter_e3d::pml::parse_pml_text;
/// let pml = "PIPE /SITE1/ZONE1/PIPE-CW-201\n  BORE 200\n  LINREF 8\"-CW-201-A1A2\n";
/// let elements = parse_pml_text(pml).unwrap();
/// assert_eq!(elements.len(), 1);
/// ```
pub fn parse_pml_text(text: &str) -> Result<Vec<PmlElement>, PmlError> {
    let mut roots: Vec<PmlElement> = Vec::new();
    // Stack: (indent_level, element)
    let mut stack: Vec<(usize, PmlElement)> = Vec::new();

    for (ln, raw_line) in text.lines().enumerate() {
        let line_num = ln + 1;

        // Count leading spaces for indent level
        let indent = raw_line.len() - raw_line.trim_start().len();
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('!') || line.starts_with("--") {
            continue; // comment or blank
        }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let keyword = parts[0].to_uppercase();
        let rest = parts.get(1).map(|s| s.trim()).unwrap_or("");

        // Check if this is an element declaration (rest starts with '/')
        let is_element = rest.starts_with('/') || is_element_keyword(&keyword);

        if is_element || (rest.is_empty() && is_component_type(&keyword)) {
            // Pop stack elements that are at a deeper or equal indent
            while let Some(&(si, _)) = stack.last() {
                if si >= indent {
                    let (_, elem) = stack.pop().unwrap();
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.children.push(elem);
                    } else {
                        roots.push(elem);
                    }
                } else {
                    break;
                }
            }

            let db_addr = if rest.starts_with('/') {
                rest.to_owned()
            } else {
                format!("/{keyword}-{line_num}") // synthetic address for component elements
            };

            let elem_type = E3dElementType::from_str(&keyword);
            let elem = PmlElement::new(elem_type, db_addr);
            stack.push((indent, elem));
        } else if let Some((_, elem)) = stack.last_mut() {
            // This is an attribute line — parse the value
            let value = parse_pml_value(rest);
            elem.attributes.insert(keyword, value);
        }
    }

    // Flush remaining stack
    while let Some((_, elem)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(elem);
        } else {
            roots.push(elem);
        }
    }

    Ok(roots)
}

/// Returns true if this keyword starts a structural element (PIPE, EQUI, etc.).
fn is_element_keyword(kw: &str) -> bool {
    matches!(kw, "SITE" | "ZONE" | "PIPE" | "BRAN" | "EQUI" | "STRU" | "SREF" | "CTRAY")
}

/// Returns true if this keyword starts a piping component element.
fn is_component_type(kw: &str) -> bool {
    matches!(kw,
        "ELBOW" | "TEE" | "REDU" | "FLAN" | "VALV" | "GASK" |
        "WELD"  | "PSUP" | "NOZZ" | "STPIPE" | "STRAIG"
    )
}

/// Parse a PML attribute value string.
fn parse_pml_value(s: &str) -> PmlValue {
    let s = s.trim();

    // Database address
    if s.starts_with('/') {
        return PmlValue::DbAddr(s.to_owned());
    }

    // 3-component vector (three numbers)
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 3 {
        let nums: Vec<f64> = parts.iter().filter_map(|p| p.parse().ok()).collect();
        if nums.len() == 3 {
            return PmlValue::Vec3(nums[0], nums[1], nums[2]);
        }
    }

    // Boolean
    match s.to_uppercase().as_str() {
        "TRUE" | "ON" | "YES" => return PmlValue::Bool(true),
        "FALSE" | "OFF" | "NO" => return PmlValue::Bool(false),
        _ => {}
    }

    // Integer
    if let Ok(i) = s.parse::<i64>() {
        return PmlValue::Int(i);
    }

    // Float
    if let Ok(f) = s.parse::<f64>() {
        return PmlValue::Float(f);
    }

    // String (strip surrounding quotes if present)
    let cleaned = s.trim_matches(|c| c == '\'' || c == '"');
    PmlValue::Text(cleaned.to_owned())
}

// ── E3D attribute name constants ─────────────────────────────────────────────
// These are the canonical PML attribute names used in AVEVA E3D exports.

/// PIPE element attributes.
pub mod pipe_attrs {
    pub const BORE:    &str = "BORE";    // Nominal bore [mm]
    pub const LINREF:  &str = "LINREF";  // Line reference / line number tag
    pub const TEMP:    &str = "TEMP";    // Design temperature [K]
    pub const PRES:    &str = "PRES";    // Design pressure [Pa]
    pub const OPTEMP:  &str = "OPTEMP"; // Operating temperature [K]
    pub const OPPRES:  &str = "OPPRES"; // Operating pressure [Pa]
    pub const TPRES:   &str = "TPRES";  // Test pressure [Pa]
    pub const SPEC:    &str = "SPEC";   // Pipe spec reference
    pub const DTXR:    &str = "DTXR";   // Fluid/service code
    pub const INSUL:   &str = "INSUL";  // Insulation type
    pub const DIAM:    &str = "DIAM";   // Outside diameter [mm]
    pub const WTHK:    &str = "WTHK";   // Wall thickness [mm]
    pub const MAT:     &str = "MAT";    // Material
    pub const CORRA:   &str = "CORRA";  // Corrosion allowance [mm]
}

/// ELBOW element attributes.
pub mod elbow_attrs {
    pub const ANGL:    &str = "ANGL";   // Bend angle [degrees]
    pub const RBOR:    &str = "RBOR";   // Bore [mm]
    pub const RADI:    &str = "RADI";   // Bend radius [mm]
    pub const POS:     &str = "POS";    // Position [mm]
    pub const ORI:     &str = "ORI";    // Orientation (direction vector)
}

/// Equipment element attributes.
pub mod equi_attrs {
    pub const TAG:     &str = "TAG";    // Equipment tag number
    pub const DTYP:    &str = "DTYP";   // Equipment type
    pub const DESC:    &str = "DESC";   // Description
    pub const POS:     &str = "POS";    // Position [mm]
    pub const ORI:     &str = "ORI";    // Orientation
    pub const WGHT:    &str = "WGHT";   // Weight [kg]
    pub const DVOL:    &str = "DVOL";   // Design volume [m³]
    pub const DTEMP:   &str = "DTEMP";  // Design temperature [K]
    pub const DPRES:   &str = "DPRES";  // Design pressure [Pa]
    pub const MATI:    &str = "MATI";   // Material
    pub const DESCD:   &str = "DESCD";  // Design code
}

/// Nozzle element attributes.
pub mod nozz_attrs {
    pub const HBOR:    &str = "HBOR";   // Nominal bore [mm]
    pub const TAG:     &str = "TAG";    // Nozzle mark / tag
    pub const NFTYP:   &str = "NFTYP";  // Flange type
    pub const NFRAT:   &str = "NFRAT";  // Flange rating
    pub const POS:     &str = "POS";    // Position [mm]
    pub const HDIR:    &str = "HDIR";   // Direction vector (outward)
    pub const CREF:    &str = "CREF";   // Connected pipe reference
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pml_value_float() {
        assert_eq!(parse_pml_value("200.0"), PmlValue::Float(200.0));
        assert_eq!(parse_pml_value("333.15"), PmlValue::Float(333.15));
    }

    #[test]
    fn test_parse_pml_value_int() {
        assert_eq!(parse_pml_value("200"), PmlValue::Int(200));
    }

    #[test]
    fn test_parse_pml_value_vec3() {
        let v = parse_pml_value("11500.0 5400.0 850.0");
        assert_eq!(v, PmlValue::Vec3(11500.0, 5400.0, 850.0));
    }

    #[test]
    fn test_parse_pml_value_dbaddr() {
        let v = parse_pml_value("/SITE01/ZONE-CW/PIPE-CW-201");
        assert_eq!(v, PmlValue::DbAddr("/SITE01/ZONE-CW/PIPE-CW-201".to_owned()));
    }

    #[test]
    fn test_parse_pml_value_string() {
        let v = parse_pml_value("'8\"-CW-201-A1A2'");
        assert!(matches!(v, PmlValue::Text(s) if s == "8\"-CW-201-A1A2"));
    }

    #[test]
    fn test_e3d_element_type_from_str() {
        assert_eq!(E3dElementType::from_str("PIPE"), E3dElementType::Pipe);
        assert_eq!(E3dElementType::from_str("elbow"), E3dElementType::Elbow);
        assert_eq!(E3dElementType::from_str("EQUI"), E3dElementType::Equipment);
        assert_eq!(E3dElementType::from_str("NOZZ"), E3dElementType::Nozzle);
    }

    #[test]
    fn test_e3d_element_type_pmef_type() {
        assert_eq!(E3dElementType::Pipe.pmef_type(), Some("pmef:PipingNetworkSystem"));
        assert_eq!(E3dElementType::Elbow.pmef_type(), Some("pmef:Elbow"));
        assert_eq!(E3dElementType::Nozzle.pmef_type(), None); // embedded
    }

    #[test]
    fn test_parse_pml_text_pipe() {
        let pml = "\
PIPE /SITE01/ZONE-CW/PIPE-CW-201
  BORE 200
  LINREF '8\"-CW-201-A1A2'
  TEMP 333.15
  PRES 1600000
  SPEC /SPEC-A1A2
  DTXR CW
";
        let elements = parse_pml_text(pml).unwrap();
        assert_eq!(elements.len(), 1);
        let pipe = &elements[0];
        assert_eq!(pipe.element_type, E3dElementType::Pipe);
        assert_eq!(pipe.attr_f64("BORE"), Some(200.0));
        assert_eq!(pipe.attr_f64("TEMP"), Some(333.15));
        assert_eq!(pipe.attr_f64("PRES"), Some(1600000.0));
        assert_eq!(pipe.attr_str("DTXR"), Some("CW"));
    }

    #[test]
    fn test_parse_pml_text_pipe_with_elbow() {
        let pml = "\
PIPE /S/Z/PIPE-001
  BORE 200
  LINREF 'CW-201'
  BRAN /S/Z/PIPE-001/BRAN1
    BORE 200
    ELBOW
      ANGL 90.0
      RBOR 200
      RADI 304.8
      POS 11500.0 5400.0 850.0
";
        let elements = parse_pml_text(pml).unwrap();
        assert_eq!(elements.len(), 1);
        let pipe = &elements[0];
        assert_eq!(pipe.children.len(), 1); // BRAN
        let bran = &pipe.children[0];
        assert_eq!(bran.element_type, E3dElementType::Branch);
        assert_eq!(bran.children.len(), 1); // ELBOW
        let elbow = &bran.children[0];
        assert_eq!(elbow.element_type, E3dElementType::Elbow);
        assert_eq!(elbow.attr_f64("ANGL"), Some(90.0));
        assert_eq!(elbow.attr_f64("RADI"), Some(304.8));
        let pos = elbow.attr("POS").unwrap().as_vec3().unwrap();
        assert_eq!(pos, (11500.0, 5400.0, 850.0));
    }

    #[test]
    fn test_parse_pml_text_equipment() {
        let pml = "\
EQUI /SITE01/ZONE-U100/EQUI-P-201A
  TAG 'P-201A'
  DTYP 'CENTRIFUGAL_PUMP'
  DESC 'Cooling water pump'
  POS 10200.0 5400.0 1000.0
  WGHT 1850.0
  NOZZ /SITE01/ZONE-U100/EQUI-P-201A/NOZZ-SUCTION
    HBOR 200
    TAG 'SUCTION'
    POS 10200.0 5400.0 850.0
    HDIR -1.0 0.0 0.0
";
        let elements = parse_pml_text(pml).unwrap();
        assert_eq!(elements.len(), 1);
        let equi = &elements[0];
        assert_eq!(equi.element_type, E3dElementType::Equipment);
        assert_eq!(equi.attr_str("TAG"), Some("P-201A"));
        assert_eq!(equi.attr_f64("WGHT"), Some(1850.0));
        assert_eq!(equi.children.len(), 1);
        let nozz = &equi.children[0];
        assert_eq!(nozz.element_type, E3dElementType::Nozzle);
        assert_eq!(nozz.attr_f64("HBOR"), Some(200.0));
        assert_eq!(nozz.attr_str("TAG"), Some("SUCTION"));
    }
}
