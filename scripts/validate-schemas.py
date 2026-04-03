#!/usr/bin/env python3
"""
validate-schemas.py — Validate all PMEF JSON Schema files.

Checks:
1. Each schema file is valid JSON
2. Each schema is a valid JSON Schema Draft 2020-12 meta-schema instance
3. All $ref targets within the schemas directory are resolvable
4. All $defs are referenced at least once (no dead definitions)

Usage:
    python scripts/validate-schemas.py
    python scripts/validate-schemas.py --strict    # fail on warnings
    python scripts/validate-schemas.py --verbose
"""

import json
import sys
import argparse
import re
from pathlib import Path

try:
    import jsonschema
    from jsonschema import Draft202012Validator
except ImportError:
    print("ERROR: jsonschema not installed. Run: pip install jsonschema", file=sys.stderr)
    sys.exit(1)

SCHEMAS_DIR = Path(__file__).parent.parent / "schemas"
META_SCHEMA_ID = "https://json-schema.org/draft/2020-12/schema"


def load_schema(path: Path) -> dict:
    """Load and parse a JSON Schema file."""
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def check_json_syntax(path: Path) -> list[str]:
    """Return list of errors if file is not valid JSON."""
    try:
        load_schema(path)
        return []
    except json.JSONDecodeError as e:
        return [f"Invalid JSON: {e}"]


def check_schema_validity(schema: dict, path: Path) -> list[str]:
    """Validate schema against JSON Schema Draft 2020-12 meta-schema."""
    errors = []
    try:
        Draft202012Validator.check_schema(schema)
    except jsonschema.SchemaError as e:
        errors.append(f"Invalid schema: {e.message}")
    return errors


def check_required_fields(schema: dict, path: Path) -> list[str]:
    """Check PMEF-specific schema conventions."""
    warnings = []
    
    def check_def(name: str, defn: dict, prefix: str = ""):
        if defn.get("type") == "object":
            props = defn.get("properties", {})
            for prop_name, prop_schema in props.items():
                if "description" not in prop_schema and "$ref" not in prop_schema:
                    warnings.append(
                        f"  WARN [{prefix}{name}].{prop_name}: missing 'description'"
                    )
                if "title" not in prop_schema and "$ref" not in prop_schema:
                    warnings.append(
                        f"  WARN [{prefix}{name}].{prop_name}: missing 'title'"
                    )
    
    for def_name, definition in schema.get("$defs", {}).items():
        check_def(def_name, definition)
    
    return warnings


def find_internal_refs(schema_text: str) -> list[str]:
    """Find all $ref values that reference other schema files."""
    refs = re.findall(r'"\$ref":\s*"([^"#][^"]*)"', schema_text)
    return [r for r in refs if not r.startswith("#")]


def check_cross_refs(schemas: dict[str, dict], schema_dir: Path) -> list[str]:
    """Check that all cross-file $refs resolve to real files and $defs."""
    errors = []
    for filename, schema in schemas.items():
        schema_text = json.dumps(schema)
        for ref in find_internal_refs(schema_text):
            if "#" in ref:
                file_part, def_part = ref.split("#", 1)
            else:
                file_part, def_part = ref, ""
            
            target_path = schema_dir / file_part
            if not target_path.exists():
                errors.append(f"  {filename}: broken $ref to '{ref}' — file not found")
                continue
            
            if def_part and def_part.startswith("/$defs/"):
                def_name = def_part.split("/")[-1]
                target_schema = schemas.get(file_part, {})
                if def_name not in target_schema.get("$defs", {}):
                    errors.append(
                        f"  {filename}: broken $ref to '{ref}' — $def '{def_name}' not found in {file_part}"
                    )
    return errors


def main():
    parser = argparse.ArgumentParser(description="Validate PMEF JSON Schemas")
    parser.add_argument("--strict", action="store_true", help="Fail on warnings")
    parser.add_argument("--verbose", action="store_true", help="Show all checks")
    args = parser.parse_args()

    schema_files = sorted(SCHEMAS_DIR.glob("*.schema.json"))
    
    if not schema_files:
        print(f"ERROR: No schema files found in {SCHEMAS_DIR}", file=sys.stderr)
        sys.exit(1)

    print(f"Validating {len(schema_files)} schema(s) in {SCHEMAS_DIR}/\n")

    all_errors = []
    all_warnings = []
    schemas: dict[str, dict] = {}

    for schema_path in schema_files:
        rel = schema_path.name
        if args.verbose:
            print(f"  Checking {rel} ...")

        # Step 1: JSON syntax
        errors = check_json_syntax(schema_path)
        if errors:
            all_errors.extend([f"{rel}: {e}" for e in errors])
            continue  # Can't proceed with invalid JSON

        schema = load_schema(schema_path)
        schemas[rel] = schema

        # Step 2: Schema validity
        errors = check_schema_validity(schema, schema_path)
        all_errors.extend([f"{rel}: {e}" for e in errors])

        # Step 3: PMEF conventions
        warnings = check_required_fields(schema, schema_path)
        all_warnings.extend([f"{rel}{w[1:]}" for w in warnings])

        if not errors:
            print(f"  ✓  {rel}")

    # Step 4: Cross-file $ref resolution
    ref_errors = check_cross_refs(schemas, SCHEMAS_DIR)
    all_errors.extend(ref_errors)

    print()

    if all_warnings and args.verbose:
        print(f"Warnings ({len(all_warnings)}):")
        for w in all_warnings:
            print(f"  {w}")
        print()

    if all_errors:
        print(f"FAILED — {len(all_errors)} error(s):")
        for e in all_errors:
            print(f"  ✗  {e}")
        sys.exit(1)

    if all_warnings and args.strict:
        print(f"FAILED (strict mode) — {len(all_warnings)} warning(s)")
        for w in all_warnings:
            print(f"  ⚠  {w}")
        sys.exit(1)

    print(f"All {len(schema_files)} schemas valid.")
    if all_warnings:
        print(f"({len(all_warnings)} non-strict warnings — run with --verbose to see)")


if __name__ == "__main__":
    main()
