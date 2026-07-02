//! Topology / connectivity integrity checks for PMEF packages.
//!
//! These checks validate the structural invariants that must hold *within* a
//! single self-contained dataset, complementing the per-object JSON Schema
//! validation in [`crate::PmefValidator`]:
//!
//! 1. **Unique `@id`** — no object identifier appears more than once.
//! 2. **`connectedTo` resolution** — every port-level or object-level
//!    `connectedTo` references an `@id` defined in the same dataset.
//! 3. **Port reciprocity** — if object A connects to object B (and B is in the
//!    dataset), then B must connect back to A.
//!
//! Cross-layer references such as `isPartOf` → unit or `targetId` → functional
//! legitimately point outside a single dataset and are intentionally *not*
//! checked here. Cross-dataset connectivity is supported via `allow_external`,
//! which downgrades unresolved `connectedTo` targets to warnings.
//!
//! This mirrors the reference `scripts/check-topology.py` tool so the Rust
//! implementation and the CI tooling stay in lockstep.

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// The category of a topology issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyIssueKind {
    /// The same `@id` was defined by more than one object.
    DuplicateId,
    /// A `connectedTo` target does not resolve to any `@id` in the dataset.
    DanglingConnection,
    /// A connection A → B exists but B does not connect back to A.
    NonReciprocal,
}

/// A single topology finding.
#[derive(Debug, Clone)]
pub struct TopologyIssue {
    /// The category of the issue.
    pub kind: TopologyIssueKind,
    /// Human-readable description.
    pub message: String,
    /// `true` if this is a hard error, `false` if it is a downgraded warning
    /// (only dangling connections under `allow_external`).
    pub is_error: bool,
}

/// The result of a topology check over a dataset.
#[derive(Debug, Default)]
pub struct TopologyReport {
    /// Number of distinct objects (by `@id`) inspected.
    pub object_count: usize,
    /// Number of `connectedTo` edges found.
    pub connection_count: usize,
    /// All issues, in discovery order.
    pub issues: Vec<TopologyIssue>,
}

impl TopologyReport {
    /// Number of hard errors.
    pub fn error_count(&self) -> usize {
        self.issues.iter().filter(|i| i.is_error).count()
    }

    /// Number of downgraded warnings.
    pub fn warning_count(&self) -> usize {
        self.issues.iter().filter(|i| !i.is_error).count()
    }

    /// `true` when there are no hard errors.
    pub fn is_ok(&self) -> bool {
        self.error_count() == 0
    }
}

/// Recursively collect every `connectedTo` string value within an object.
///
/// Handles both a single string and an array of strings, matching the PMEF
/// serialisation where `connectedTo` sits on `ports[]` entries.
fn collect_connections(node: &Value, out: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "connectedTo" {
                    match value {
                        Value::String(s) => out.push(s.clone()),
                        Value::Array(arr) => {
                            for item in arr {
                                if let Value::String(s) = item {
                                    out.push(s.clone());
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    collect_connections(value, out);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_connections(item, out);
            }
        }
        _ => {}
    }
}

/// Run all topology checks over a slice of full PMEF objects.
///
/// Each `Value` is expected to be a top-level object carrying an `@id`.
/// When `allow_external` is set, unresolved `connectedTo` targets are reported
/// as warnings rather than errors (federated / cross-dataset boundaries).
pub fn check_topology(objects: &[Value], allow_external: bool) -> TopologyReport {
    let mut report = TopologyReport::default();

    // Count @id occurrences to detect duplicates.
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for obj in objects {
        if let Some(id) = obj.get("@id").and_then(|v| v.as_str()) {
            *counts.entry(id).or_insert(0) += 1;
        }
    }
    let ids: HashSet<&str> = counts.keys().copied().collect();
    report.object_count = ids.len();

    let mut duplicates: Vec<(&str, usize)> =
        counts.iter().filter(|(_, n)| **n > 1).map(|(id, n)| (*id, *n)).collect();
    duplicates.sort_unstable();
    for (id, n) in duplicates {
        report.issues.push(TopologyIssue {
            kind: TopologyIssueKind::DuplicateId,
            is_error: true,
            message: format!("Duplicate @id: {id} ({n}\u{00d7})"),
        });
    }

    // Build the directed connectedTo edge set (source @id → target).
    let mut edges: Vec<(String, String)> = Vec::new();
    for obj in objects {
        let owner = match obj.get("@id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let mut conns = Vec::new();
        collect_connections(obj, &mut conns);
        for target in conns {
            if target == owner {
                continue; // ignore self-references
            }
            edges.push((owner.to_string(), target));
        }
    }
    report.connection_count = edges.len();

    // Deduplicate identical edges so a repeated connectedTo target does not
    // produce duplicate dangling/non-reciprocal findings.
    edges.sort_unstable();
    edges.dedup();

    let edge_set: HashSet<(&str, &str)> =
        edges.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();

    for (source, target) in &edges {
        if !ids.contains(target.as_str()) {
            let base = format!("connectedTo target not found: {source} \u{2192} {target}");
            report.issues.push(if allow_external {
                TopologyIssue {
                    kind: TopologyIssueKind::DanglingConnection,
                    is_error: false,
                    message: format!("{base} (treated as external boundary)"),
                }
            } else {
                TopologyIssue {
                    kind: TopologyIssueKind::DanglingConnection,
                    is_error: true,
                    message: base,
                }
            });
        } else if !edge_set.contains(&(target.as_str(), source.as_str())) {
            report.issues.push(TopologyIssue {
                kind: TopologyIssueKind::NonReciprocal,
                is_error: true,
                message: format!(
                    "Non-reciprocal connection: {source} \u{2192} {target} \
                     (no return connection from {target})"
                ),
            });
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clean_reciprocal_dataset_passes() {
        let objects = vec![
            json!({"@id": "a", "ports": [{"connectedTo": "b"}]}),
            json!({"@id": "b", "ports": [{"connectedTo": "a"}]}),
        ];
        let report = check_topology(&objects, false);
        assert!(report.is_ok(), "issues: {:?}", report.issues);
        assert_eq!(report.object_count, 2);
        assert_eq!(report.connection_count, 2);
    }

    #[test]
    fn dangling_connection_is_error() {
        let objects = vec![json!({"@id": "a", "ports": [{"connectedTo": "missing"}]})];
        let report = check_topology(&objects, false);
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.issues[0].kind, TopologyIssueKind::DanglingConnection);
    }

    #[test]
    fn dangling_connection_downgraded_when_external_allowed() {
        let objects = vec![json!({"@id": "a", "ports": [{"connectedTo": "missing"}]})];
        let report = check_topology(&objects, true);
        assert_eq!(report.error_count(), 0);
        assert_eq!(report.warning_count(), 1);
        assert!(report.is_ok());
    }

    #[test]
    fn non_reciprocal_connection_is_error() {
        let objects = vec![
            json!({"@id": "a", "ports": [{"connectedTo": "b"}]}),
            json!({"@id": "b"}),
        ];
        let report = check_topology(&objects, false);
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.issues[0].kind, TopologyIssueKind::NonReciprocal);
    }

    #[test]
    fn duplicate_id_is_error() {
        let objects = vec![json!({"@id": "a"}), json!({"@id": "a"})];
        let report = check_topology(&objects, false);
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.issues[0].kind, TopologyIssueKind::DuplicateId);
    }

    #[test]
    fn list_valued_connected_to_is_supported() {
        let objects = vec![
            json!({"@id": "a", "connectedTo": ["b"]}),
            json!({"@id": "b", "connectedTo": ["a"]}),
        ];
        let report = check_topology(&objects, false);
        assert!(report.is_ok(), "issues: {:?}", report.issues);
        assert_eq!(report.connection_count, 2);
    }
}
