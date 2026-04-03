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

# Map @type prefix → schema file
TYPE_TO_SCHEMA: dict[str, str] = {
    "pmef:PipingNetworkSystem": "pmef-piping-component.schema.json",
    "pmef:PipingSegment":       "pmef-piping-component.schema.json",
    "pmef:Pipe":                "pmef-piping-component.schema.json",
    "pmef:Elbow":               "pmef-piping-component.schema.json",
    "pmef:Tee":                 "pmef-piping-component.schema.json",
    "pmef:Reducer":             "pmef-piping-component.schema.json",
    "pmef:Flange":              "pmef-piping-component.schema.json",
    "pmef:Valve":               "pmef-piping-component.schema.json",
    "pmef:Olet":                "pmef-piping-component.schema.json",
    "pmef:Gasket":              "pmef-piping-component.schema.json",
    "pmef:Weld":                "pmef-piping-component.schema.json",
    "pmef:PipeSupport":         "pmef-piping-component.schema.json",
    "pmef:Spool":               "pmef-piping-component.schema.json",
    "pmef:Vessel":              "pmef-equipment.schema.json",
    "pmef:Tank":                "pmef-equipment.schema.json",
    "pmef:Pump":                "pmef-equipment.schema.json",
    "pmef:Compressor":          "pmef-equipment.schema.json",
    "pmef:HeatExchanger":       "pmef-equipment.schema.json",
    "pmef:Column":              "pmef-equipment.schema.json",
    "pmef:Reactor":             "pmef-equipment.schema.json",
    "pmef:Filter":              "pmef-equipment.schema.json",
    "pmef:Turbine":             "pmef-equipment.schema.json",
    "pmef:GenericEquipment":    "pmef-equipment.schema.json",
    "pmef:ParametricGeometry":  "pmef-geometry.schema.json",
    # Header/hierarchy types — validate as free-form JSON objects
    "pmef:FileHeader": None,
    "pmef:Plant":      None,
    "pmef:Unit":       None,
    "pmef:Area":       None,
}


def load_schemas() -> dict[str, dict]:
    """Load all schemas and build a store for RefResolver."""
    store = {}
    schemas = {}
    for path in SCHEMAS_DIR.glob("*.schema.json"):
        schema = json.loads(path.read_text(encoding="utf-8"))
        store[schema["$id"]] = schema
        schemas[path.name] = schema
    return schemas, store


def validate_object(obj: dict, schemas: dict, store: dict) -> list[str]:
    """Validate a single PMEF object. Returns list of error messages."""
    type_ = obj.get("@type")
    if not type_:
        return ["Missing '@type' field"]

    schema_file = TYPE_TO_SCHEMA.get(type_)
    if schema_file is None:
        return []  # Hierarchy objects: skip schema validation

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


def validate_file(ndjson_path: Path, schemas: dict, store: dict, verbose: bool) -> int:
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
        errors = validate_object(obj, schemas, store)
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
    print(f"Loaded {len(schemas)} schema(s)\n")

    files = [args.file] if args.file else sorted(EXAMPLES_DIR.glob("*.ndjson"))
    if not files:
        print(f"No .ndjson files found in {EXAMPLES_DIR}")
        sys.exit(0)

    total_errors = 0
    for ndjson_path in files:
        print(f"Validating {ndjson_path.name} ...")
        errors = validate_file(ndjson_path, schemas, store, args.verbose)
        total_errors += errors
        print()

    if total_errors:
        print(f"FAILED — {total_errors} validation error(s) across {len(files)} file(s)")
        sys.exit(1)

    print(f"All {len(files)} example file(s) valid.")


if __name__ == "__main__":
    main()
