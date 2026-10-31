#!/usr/bin/env python3
"""
validate-examples.py — Validate all PMEF NDJSON example files.

For each line in each .ndjson file (skipping comment lines starting with //):
1. Parse as JSON
2. Determine the PMEF type from "@type"
3. Validate against the appropriate JSON Schema

Usage:
    python scripts/validate-examples.py
    python scripts/validate-examples.py --file examples/pump-skid-complete.ndjson
    python scripts/validate-examples.py --verbose
"""

import json
import sys
import argparse
from pathlib import Path

try:
    import jsonschema
    from jsonschema import Draft202012Validator
    from jsonschema.validators import RefResolver
except ImportError:
    print("ERROR: jsonschema not installed. Run: pip install jsonschema", file=sys.stderr)
    sys.exit(1)

ROOT = Path(__file__).parent.parent
SCHEMAS_DIR = ROOT / "schemas"
EXAMPLES_DIR = ROOT / "examples"

def build_type_map(schemas: dict[str, dict]) -> dict[str, str]:
    """Derive @type → schema file from the schemas themselves.

    Every object definition declares its type as ``"@type": {"const": ...}``
    (or an ``enum``). Deriving the map avoids a second hand-maintained list that
    can drift from the schemas (it previously missed all steel and E&I types).
    """
    type_map: dict[str, str] = {}

    def walk(node, schema_file):
        if isinstance(node, dict):
            props = node.get("properties")
            if isinstance(props, dict) and isinstance(props.get("@type"), dict):
                t = props["@type"]
                for value in ([t["const"]] if "const" in t else []) + list(t.get("enum", [])):
                    other = type_map.setdefault(value, schema_file)
                    if other != schema_file:
                        raise SystemExit(f"@type {value} defined in {other} and {schema_file}")
            for v in node.values():
                walk(v, schema_file)
        elif isinstance(node, list):
            for v in node:
                walk(v, schema_file)

    for name, schema in sorted(schemas.items()):
        walk(schema, name)
    return type_map


def load_schemas() -> dict[str, dict]:
    """Load all schemas and build a store for RefResolver."""
    store = {}
    schemas = {}
    for path in SCHEMAS_DIR.glob("*.schema.json"):
        schema = json.loads(path.read_text(encoding="utf-8"))
        store[schema["$id"]] = schema
        schemas[path.name] = schema
    return schemas, store


def validate_object(obj: dict, schemas: dict, store: dict, type_map: dict) -> list[str]:
    """Validate a single PMEF object. Returns list of error messages."""
    type_ = obj.get("@type")
    if not type_:
        return ["Missing '@type' field"]

    schema_file = type_map.get(type_)
    if schema_file is None:
        # Unknown types are errors, never silently skipped.
        return [f"Unknown @type '{type_}': no schema defines it"]

    if schema_file not in schemas:
        return [f"Schema '{schema_file}' not found for type '{type_}'"]

    schema = schemas[schema_file]

    # Build resolver with all schemas in the store
    base_uri = schema["$id"]
    resolver = RefResolver(base_uri=base_uri, referrer=schema, store=store)
    validator = Draft202012Validator(schema, resolver=resolver)

    errors = []
    for error in validator.iter_errors(obj):
        path = " → ".join(str(p) for p in error.absolute_path) or "(root)"
        errors.append(f"{path}: {error.message}")
    return errors


def validate_file(ndjson_path: Path, schemas: dict, store: dict, type_map: dict, verbose: bool) -> int:
    """Validate all objects in an NDJSON file. Returns error count."""
    errors_total = 0
    obj_count = 0

    for line_no, line in enumerate(ndjson_path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue

        # Parse JSON
        try:
            obj = json.loads(stripped)
        except json.JSONDecodeError as e:
            print(f"  ✗  Line {line_no}: Invalid JSON — {e}")
            errors_total += 1
            continue

        obj_count += 1
        type_ = obj.get("@type", "?")

        # Validate against schema
        errors = validate_object(obj, schemas, store, type_map)
        if errors:
            print(f"  ✗  Line {line_no} [{type_}]:")
            for e in errors:
                print(f"       {e}")
            errors_total += len(errors)
        elif verbose:
            print(f"  ✓  Line {line_no} [{type_}]")

    if not verbose:
        print(f"  {obj_count} objects checked, {errors_total} error(s)")

    return errors_total


def main():
    parser = argparse.ArgumentParser(description="Validate PMEF NDJSON examples")
    parser.add_argument("--file", type=Path, help="Validate a specific file only")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    schemas, store = load_schemas()
    type_map = build_type_map(schemas)
    print(f"Loaded {len(schemas)} schema(s), {len(type_map)} object type(s)\n")

    files = [args.file] if args.file else sorted(EXAMPLES_DIR.glob("*.ndjson"))
    if not files:
        print(f"No .ndjson files found in {EXAMPLES_DIR}")
        sys.exit(0)

    total_errors = 0
    for ndjson_path in files:
        print(f"Validating {ndjson_path.name} ...")
        errors = validate_file(ndjson_path, schemas, store, type_map, args.verbose)
        total_errors += errors
        print()

    if total_errors:
        print(f"FAILED — {total_errors} validation error(s) across {len(files)} file(s)")
        sys.exit(1)

    print(f"All {len(files)} example file(s) valid.")


if __name__ == "__main__":
    main()
