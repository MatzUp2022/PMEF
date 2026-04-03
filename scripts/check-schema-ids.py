#!/usr/bin/env python3
"""
check-schema-ids.py — Verify that each schema's $id matches its file path.

Ensures that the $id field in each JSON Schema file follows the convention:
    https://pmef.org/schemas/0.9/<filename>

Usage:
    python scripts/check-schema-ids.py
"""

import json
import sys
from pathlib import Path

SCHEMAS_DIR = Path(__file__).parent.parent / "schemas"
BASE_URI = "https://pmef.org/schemas/0.9/"


def main():
    schema_files = sorted(SCHEMAS_DIR.glob("*.schema.json"))

    if not schema_files:
        print(f"ERROR: No schema files found in {SCHEMAS_DIR}", file=sys.stderr)
        sys.exit(1)

    print(f"Checking $id fields in {len(schema_files)} schema(s)...\n")

    errors = []
    for schema_path in schema_files:
        with open(schema_path, encoding="utf-8") as f:
            schema = json.load(f)

        schema_id = schema.get("$id", "")
        expected_id = BASE_URI + schema_path.name

        if not schema_id:
            errors.append(f"  {schema_path.name}: missing $id field")
        elif schema_id != expected_id:
            errors.append(
                f"  {schema_path.name}: $id mismatch\n"
                f"    expected: {expected_id}\n"
                f"    actual:   {schema_id}"
            )
        else:
            print(f"  ✓  {schema_path.name}")

    print()

    if errors:
        print(f"FAILED — {len(errors)} error(s):")
        for e in errors:
            print(e)
        sys.exit(1)

    print(f"All {len(schema_files)} schema $id fields match file paths.")


if __name__ == "__main__":
    main()
