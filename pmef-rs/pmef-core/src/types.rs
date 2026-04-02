//! Primitive value types shared across the PMEF information model.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

// ────────────────────────────────────────────────────────────────────────────
// PmefId
// ────────────────────────────────────────────────────────────────────────────

/// Globally unique, stable identifier for a PMEF object.
///
/// Format: `urn:pmef:<domain>:<project>:<local-id>`
///
/// # Examples
/// ```
/// use pmef_core::PmefId;
/// let id: PmefId = "urn:pmef:obj:eaf-2026:P-201A".parse().unwrap();
/// assert_eq!(id.domain(), "obj");
/// assert_eq!(id.project(), "eaf-2026");
/// assert_eq!(id.local_id(), "P-201A");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PmefId(String);

#[derive(Debug, Error)]
#[error("Invalid PmefId '{0}': must match urn:pmef:<domain>:<project>:<local-id>")]
pub struct PmefIdError(String);

impl PmefId {
    /// Create a new `PmefId` without validation (use in trusted contexts only).
    pub fn new_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The domain segment (e.g. `"obj"`, `"line"`, `"geom"`).
    pub fn domain(&self) -> &str {
        self.0.splitn(6, ':').nth(2).unwrap_or("")
    }

    /// The project segment (e.g. `"eaf-2026"`).
    pub fn project(&self) -> &str {
        self.0.splitn(6, ':').nth(3).unwrap_or("")
    }

    /// The local identifier (e.g. `"P-201A"`).
    pub fn local_id(&self) -> &str {
        self.0.splitn(6, ':').nth(4).unwrap_or(&self.0)
    }

    /// Full URI string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for PmefId {
    type Err = PmefIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Pattern: urn:pmef:<domain>:<project>:<local>
        let parts: Vec<&str> = s.splitn(6, ':').collect();
        if parts.len() < 5
            || parts[0] != "urn"
            || parts[1] != "pmef"
            || parts[2].is_empty()
            || parts[3].is_empty()
            || parts[4].is_empty()
        {
            return Err(PmefIdError(s.to_owned()));
        }
        // Validate allowed chars
        let valid = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_');
        if !parts[2].chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            || !parts[3].chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            || !parts[4].chars().all(valid)
        {
            return Err(PmefIdError(s.to_owned()));
        }
        Ok(Self(s.to_owned()))
    }
}

impl fmt::Display for PmefId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// PmefVersion
// ────────────────────────────────────────────────────────────────────────────

/// PMEF specification version (SemVer).
///
/// # Example
/// ```
/// use pmef_core::PmefVersion;
/// let v = PmefVersion::new(0, 9, 0);
/// assert_eq!(v.to_string(), "0.9.0");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PmefVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl PmefVersion {
    pub const CURRENT: Self = Self { major: 0, minor: 9, patch: 0 };

    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Returns true if this version is compatible with `other`
    /// (same major, minor ≤ other.minor).
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major && self.minor <= other.minor
    }
}

impl fmt::Display for PmefVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for PmefVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Expected MAJOR.MINOR.PATCH, got '{s}'"));
        }
        Ok(Self {
            major: parts[0].parse().map_err(|_| format!("Invalid major: {s}"))?,
            minor: parts[1].parse().map_err(|_| format!("Invalid minor: {s}"))?,
            patch: parts[2].parse().map_err(|_| format!("Invalid patch: {s}"))?,
        })
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Coordinate3D / UnitVector3D
// ────────────────────────────────────────────────────────────────────────────

/// 3D coordinate in the project coordinate system (Z-up, mm).
///
/// # Example
/// ```
/// use pmef_core::Coordinate3D;
/// let p = Coordinate3D::new(100.0, 200.0, 850.0);
/// assert_eq!(p.z(), 850.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f64; 3]", into = "[f64; 3]")]
pub struct Coordinate3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Coordinate3D {
    pub fn new(x: f64, y: f64, z: f64) -> Self { Self { x, y, z } }
    pub fn origin() -> Self { Self::new(0.0, 0.0, 0.0) }
    pub fn x(&self) -> f64 { self.x }
    pub fn y(&self) -> f64 { self.y }
    pub fn z(&self) -> f64 { self.z }

    pub fn distance_to(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
}

impl From<[f64; 3]> for Coordinate3D {
    fn from(a: [f64; 3]) -> Self { Self::new(a[0], a[1], a[2]) }
}
impl From<Coordinate3D> for [f64; 3] {
    fn from(c: Coordinate3D) -> Self { [c.x, c.y, c.z] }
}

/// A unit direction vector (not validated, caller responsibility).
pub type UnitVector3D = Coordinate3D;

// ────────────────────────────────────────────────────────────────────────────
// RdlUri / Iec81346Designation
// ────────────────────────────────────────────────────────────────────────────

/// URI pointing to a class in the PCA-RDL (ISO 15926-4) or CFIHOS-RDL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RdlUri(pub String);

impl RdlUri {
    pub fn new(uri: impl Into<String>) -> Self { Self(uri.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl fmt::Display for RdlUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}

/// IEC 81346 reference designation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Iec81346Designation {
    /// Functional aspect, e.g. `"=U100.M01.A"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functional_aspect: Option<String>,
    /// Product aspect, e.g. `"-P201A"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_aspect: Option<String>,
    /// Location aspect, e.g. `"+F01.P01"`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_aspect: Option<String>,
}
