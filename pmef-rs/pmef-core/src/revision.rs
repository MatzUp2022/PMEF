//! Revision metadata and CDE workflow state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// ISO 19650 Common Data Environment workflow state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChangeState {
    #[default]
    Wip,
    Shared,
    Published,
    Archived,
}

impl ChangeState {
    /// Returns true if the object can still be modified.
    pub fn is_mutable(&self) -> bool {
        matches!(self, ChangeState::Wip | ChangeState::Shared)
    }
}

/// Revision metadata block carried by every PMEF physical object.
///
/// Provides ISO 19650 CDE integration and round-trip identity support.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionMetadata {
    /// Unique revision identifier. Format: `r<YYYY>-<MM>-<DD>-<NNN>`.
    pub revision_id: String,

    /// CDE workflow state.
    pub change_state: ChangeState,

    /// RevisionId of the predecessor revision, or `None` for initial.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_revision_id: Option<String>,

    /// Human-readable reason for this revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_reason: Option<String>,

    /// Person or system that created this revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_by: Option<String>,

    /// Timestamp of the revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_at: Option<DateTime<Utc>>,

    /// Source tool name and version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authoring_tool: Option<String>,

    /// Native object ID in the authoring tool (enables round-trip re-import).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authoring_tool_object_id: Option<String>,

    /// SHA-256 checksum of the serialised object (excluding this field).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

impl RevisionMetadata {
    /// Create a minimal WIP revision.
    pub fn wip(revision_id: impl Into<String>, tool: impl Into<String>) -> Self {
        Self {
            revision_id: revision_id.into(),
            change_state: ChangeState::Wip,
            parent_revision_id: None,
            change_reason: None,
            changed_by: None,
            changed_at: Some(Utc::now()),
            authoring_tool: Some(tool.into()),
            authoring_tool_object_id: None,
            checksum: None,
        }
    }

    /// Create a SHARED revision (ready for review).
    pub fn shared(
        revision_id: impl Into<String>,
        parent: Option<String>,
        tool: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            revision_id: revision_id.into(),
            change_state: ChangeState::Shared,
            parent_revision_id: parent,
            change_reason: Some(reason.into()),
            changed_by: None,
            changed_at: Some(Utc::now()),
            authoring_tool: Some(tool.into()),
            authoring_tool_object_id: None,
            checksum: None,
        }
    }
}
