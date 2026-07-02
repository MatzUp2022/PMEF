//! PMEF JSON Schema validation and conformance checking.
//!
//! Provides functions to validate PMEF JSON objects against their
//! corresponding JSON Schema definitions.

use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

pub mod topology;
pub use topology::{check_topology, TopologyIssue, TopologyIssueKind, TopologyReport};

/// A single validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// JSON Pointer path to the failing property (e.g. "/equipmentBasic/tagNumber").
    pub instance_path: String,
    /// Human-readable description of the error.
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.instance_path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.instance_path, self.message)
        }
    }
}

/// Map an entity `@type` string to the schema filename that governs it.
pub fn schema_for_type(type_str: &str) -> Option<&'static str> {
    match type_str {
        "pmef:PipingNetworkSystem" | "pmef:PipingSegment" | "pmef:Pipe" |
        "pmef:Elbow" | "pmef:Tee" | "pmef:Reducer" | "pmef:Flange" |
        "pmef:Valve" | "pmef:Olet" | "pmef:Gasket" | "pmef:Weld" |
        "pmef:PipeSupport" | "pmef:Spool"
            => Some("pmef-piping-component.schema.json"),
        "pmef:Vessel" | "pmef:Tank" | "pmef:Pump" | "pmef:Compressor" |
        "pmef:HeatExchanger" | "pmef:Column" | "pmef:Reactor" |
        "pmef:Filter" | "pmef:Turbine" | "pmef:GenericEquipment"
            => Some("pmef-equipment.schema.json"),
        "pmef:InstrumentObject" | "pmef:InstrumentLoop" | "pmef:PLCObject" |
        "pmef:CableObject" | "pmef:CableTrayRun" | "pmef:MTPModule"
            => Some("pmef-ei.schema.json"),
        "pmef:SteelSystem" | "pmef:SteelMember" | "pmef:SteelNode" | "pmef:SteelConnection"
            => Some("pmef-steel.schema.json"),
        "pmef:ParametricGeometry" => Some("pmef-geometry.schema.json"),
        "pmef:IsPartOf" | "pmef:IsConnectedTo" | "pmef:IsDerivedFrom" |
        "pmef:Supports" | "pmef:ControlledBy" | "pmef:IsDocumentedBy" |
        "pmef:IsRevisionOf" | "pmef:HasEquivalentIn" | "pmef:IsCollocatedWith" |
        "pmef:ReplacedBy"
            => Some("pmef-relationships.schema.json"),
        "pmef:FileHeader" | "pmef:Plant" | "pmef:Unit" | "pmef:Area"
            => Some("pmef-hierarchy.schema.json"),
        _ => None,
    }
}

/// Extracts the `$defs` key that corresponds to a given `@type`.
///
/// For example, `"pmef:Pump"` → `"Pump"` (used to look up the sub-schema
/// inside the schema file's `$defs` object).
fn defs_key_for_type(type_str: &str) -> Option<&str> {
    type_str.strip_prefix("pmef:")
}

/// A validator that holds parsed JSON schemas.
pub struct PmefValidator {
    /// Map from schema filename → parsed schema JSON value.
    schemas: HashMap<String, Value>,
    /// Map from schema `$id` → parsed schema JSON value, used to resolve
    /// cross-file `$ref`s without network access.
    by_id: HashMap<String, Value>,
    /// Cache of compiled validators keyed by entity `@type`, so each schema is
    /// compiled at most once across a whole package.
    compiled: std::sync::Mutex<HashMap<String, jsonschema::JSONSchema>>,
}

impl PmefValidator {
    /// Load every `*.schema.json` file from a directory on disk.
    ///
    /// All schemas are indexed by their `$id` so that cross-file `$ref`s
    /// (e.g. `pmef-base.schema.json#/$defs/PmefId`) resolve locally.
    pub fn from_directory(dir: &Path) -> Result<Self, String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

        let mut schemas = HashMap::new();
        let mut by_id = HashMap::new();
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(name) if name.ends_with(".schema.json") => name.to_string(),
                _ => continue,
            };
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let val: Value = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
            if let Some(id) = val.get("$id").and_then(Value::as_str) {
                by_id.insert(id.to_string(), val.clone());
            }
            schemas.insert(filename, val);
        }

        if schemas.is_empty() {
            return Err(format!("No schema files found in {}", dir.display()));
        }

        Ok(Self {
            schemas,
            by_id,
            compiled: std::sync::Mutex::new(HashMap::new()),
        })
    }

    /// Validate a single JSON object against the schema for its `@type`.
    ///
    /// Returns an empty `Vec` if the object is valid or if no schema is
    /// available for the given type.
    pub fn validate_object(&self, entity_type: &str, object: &Value) -> Vec<ValidationError> {
        let schema_file = match schema_for_type(entity_type) {
            Some(f) => f,
            None => return vec![],
        };

        let schema_val = match self.schemas.get(schema_file) {
            Some(v) => v,
            None => return vec![],
        };

        let mut cache = self.compiled.lock().expect("validator cache poisoned");

        // Compile once per entity type; subsequent objects reuse the validator.
        if !cache.contains_key(entity_type) {
            let schema_id = match schema_val.get("$id").and_then(Value::as_str) {
                Some(id) => id,
                None => {
                    return vec![ValidationError {
                        instance_path: String::new(),
                        message: format!("Schema {schema_file} is missing a $id"),
                    }];
                }
            };

            // Point at the entity-specific sub-schema in $defs when present,
            // otherwise validate against the whole schema document. An absolute
            // `$ref` establishes the base URI so internal (`#/$defs/...`) and
            // relative cross-file refs resolve against the registered docs.
            let ref_target = match defs_key_for_type(entity_type) {
                Some(key)
                    if schema_val.get("$defs").and_then(|d| d.get(key)).is_some() =>
                {
                    format!("{schema_id}#/$defs/{key}")
                }
                _ => schema_id.to_string(),
            };

            let root = serde_json::json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "$ref": ref_target,
            });

            let mut options = jsonschema::JSONSchema::options();
            for (id, doc) in &self.by_id {
                options.with_document(id.clone(), doc.clone());
            }

            let compiled = match options.compile(&root) {
                Ok(c) => c,
                Err(e) => {
                    return vec![ValidationError {
                        instance_path: String::new(),
                        message: format!("Schema compilation error for {}: {}", entity_type, e),
                    }];
                }
            };
            cache.insert(entity_type.to_string(), compiled);
        }

        let compiled = &cache[entity_type];
        let result = compiled.validate(object);
        let errors = match result {
            Ok(()) => vec![],
            Err(errs) => errs
                .map(|e| ValidationError {
                    instance_path: e.instance_path.to_string(),
                    message: e.to_string(),
                })
                .collect(),
        };
        errors
    }
}

/// Convenience function: validate a single object without pre-loading schemas.
///
/// Loads the required schema file from `schemas_dir` on each call.
/// For bulk validation, prefer [`PmefValidator`].
pub fn validate_object(
    entity_type: &str,
    object: &Value,
    schemas_dir: &Path,
) -> Vec<ValidationError> {
    match PmefValidator::from_directory(schemas_dir) {
        Ok(v) => v.validate_object(entity_type, object),
        Err(msg) => vec![ValidationError {
            instance_path: String::new(),
            message: msg,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_schema_for_type_piping() {
        assert_eq!(
            schema_for_type("pmef:Pipe"),
            Some("pmef-piping-component.schema.json")
        );
        assert_eq!(
            schema_for_type("pmef:Elbow"),
            Some("pmef-piping-component.schema.json")
        );
    }

    #[test]
    fn test_schema_for_type_equipment() {
        assert_eq!(
            schema_for_type("pmef:Pump"),
            Some("pmef-equipment.schema.json")
        );
    }

    #[test]
    fn test_schema_for_type_hierarchy() {
        assert_eq!(
            schema_for_type("pmef:FileHeader"),
            Some("pmef-hierarchy.schema.json")
        );
        assert_eq!(
            schema_for_type("pmef:Plant"),
            Some("pmef-hierarchy.schema.json")
        );
    }

    #[test]
    fn test_schema_for_type_relationships() {
        assert_eq!(
            schema_for_type("pmef:IsDerivedFrom"),
            Some("pmef-relationships.schema.json")
        );
        assert_eq!(
            schema_for_type("pmef:HasEquivalentIn"),
            Some("pmef-relationships.schema.json")
        );
    }

    #[test]
    fn test_schema_for_type_unknown() {
        assert_eq!(schema_for_type("pmef:Unknown"), None);
        assert_eq!(schema_for_type("foo:Bar"), None);
    }

    #[test]
    fn test_defs_key() {
        assert_eq!(defs_key_for_type("pmef:Pump"), Some("Pump"));
        assert_eq!(defs_key_for_type("pmef:FileHeader"), Some("FileHeader"));
        assert_eq!(defs_key_for_type("unknown"), None);
    }

    #[test]
    fn test_validate_object_no_schema_returns_empty() {
        let validator = PmefValidator {
            schemas: HashMap::new(),
            by_id: HashMap::new(),
            compiled: std::sync::Mutex::new(HashMap::new()),
        };
        let obj = json!({"@type": "pmef:Unknown", "@id": "test-1"});
        let errors = validator.validate_object("pmef:Unknown", &obj);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_against_simple_schema() {
        // Create a minimal schema that requires @type and @id
        let schema = json!({
            "type": "object",
            "required": ["@type", "@id"],
            "properties": {
                "@type": { "type": "string" },
                "@id": { "type": "string" }
            }
        });

        let equip = json!({
            "$id": "https://pmef.test/equipment.schema.json",
            "$defs": {
                "Pump": schema
            }
        });
        let mut schemas = HashMap::new();
        schemas.insert("pmef-equipment.schema.json".to_string(), equip.clone());
        let mut by_id = HashMap::new();
        by_id.insert("https://pmef.test/equipment.schema.json".to_string(), equip);

        let validator = PmefValidator {
            schemas,
            by_id,
            compiled: std::sync::Mutex::new(HashMap::new()),
        };

        // Valid object
        let valid = json!({"@type": "pmef:Pump", "@id": "pump-001"});
        assert!(validator.validate_object("pmef:Pump", &valid).is_empty());

        // Invalid object: missing @id
        let invalid = json!({"@type": "pmef:Pump"});
        let errs = validator.validate_object("pmef:Pump", &invalid);
        assert!(!errs.is_empty(), "Expected validation errors for missing @id");
    }

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError {
            instance_path: "/equipmentBasic/tagNumber".to_string(),
            message: "value is required".to_string(),
        };
        assert_eq!(
            format!("{}", err),
            "/equipmentBasic/tagNumber: value is required"
        );

        let err_no_path = ValidationError {
            instance_path: String::new(),
            message: "missing required field".to_string(),
        };
        assert_eq!(format!("{}", err_no_path), "missing required field");
    }
}
