//! Stand-in values that a later revision replaces with real data (RFC-0012, spec 02 §2.10).
//!
//! An object keeps a valid value in the field (for example `damperType: "UNSPECIFIED"`) and
//! lists the field in `placeholders`. The replacement is an ordinary new revision of the same
//! `@id` that sets the real value and drops the entry.

use serde::{Deserialize, Serialize};

/// Why a field holds a stand-in value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlaceholderReason {
    /// The source has no value for the field.
    SourceMissing,
    /// The real value lies outside the PMEF enumeration; see the matching `*Custom` field.
    NotInEnum,
    /// A value is entered but not yet confirmed.
    ToBeConfirmed,
}

/// One field of an object that holds a stand-in value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Placeholder {
    /// JSON Pointer (RFC 6901) to the field, relative to the object, e.g. `/damperType`.
    pub path: String,
    /// Why the value is a stand-in.
    pub reason: PlaceholderReason,
    /// What is known and where the value will come from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Source or party expected to deliver the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_from: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let v = serde_json::json!({"path": "/damperType", "reason": "SOURCE_MISSING", "expectedFrom": "damper schedule"});
        let p: Placeholder = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(p.reason, PlaceholderReason::SourceMissing);
        assert_eq!(serde_json::to_value(&p).unwrap(), v);
    }
}
