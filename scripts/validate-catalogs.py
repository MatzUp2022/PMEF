#!/usr/bin/env python3
"""
validate-catalogs.py — Validate all PMEF open catalog files.

For each JSON file in catalogs/:
1. Parse as JSON
2. Validate against schemas/pmef-catalog.schema.json (which refines the shape
   of each entry based on the catalog's 'catalogType')
3. Check that per-entry identifiers (catalogId) are unique within a catalog

Usage:
    python scripts/validate-catalogs.py
    python scripts/validate-catalogs.py --file catalogs/profiles-en.json
    python scripts/validate-catalogs.py --verbose
"""

import json
import sys
import argparse
from pathlib import Path

try:
    import jsonschema
    from jsonschema import Draft202012Validator
except ImportError:
    print("ERROR: jsonschema not installed. Run: pip install jsonschema", file=sys.stderr)
    sys.exit(1)

ROOT = Path(__file__).parent.parent
SCHEMAS_DIR = ROOT / "schemas"
CATALOGS_DIR = ROOT / "catalogs"
CATALOG_SCHEMA = SCHEMAS_DIR / "pmef-catalog.schema.json"


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def check_duplicate_ids(catalog: dict) -> list[str]:
    """Return warnings for duplicate catalogId values within a catalog."""
    seen: dict[str, int] = {}
    for entry in catalog.get("entries", []):
        if not isinstance(entry, dict):
            continue
        cid = entry.get("catalogId")
        if cid is None:
            continue
        seen[cid] = seen.get(cid, 0) + 1
    return [f"Duplicate catalogId '{cid}' ({n}×)" for cid, n in seen.items() if n > 1]


def validate_catalog(path: Path, validator: Draft202012Validator) -> list[str]:
    """Validate a single catalog file. Returns list of error messages."""
    try:
        catalog = load_json(path)
    except json.JSONDecodeError as e:
        return [f"Invalid JSON: {e}"]

    errors = []
    for error in validator.iter_errors(catalog):
        loc = " → ".join(str(p) for p in error.absolute_path) or "(root)"
        errors.append(f"{loc}: {error.message}")

    errors.extend(check_duplicate_ids(catalog))
    return errors


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate PMEF catalog files")
    parser.add_argument("--file", type=Path, help="Validate a specific file only")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    if not CATALOG_SCHEMA.exists():
        print(f"ERROR: catalog schema not found at {CATALOG_SCHEMA}", file=sys.stderr)
        sys.exit(1)

    schema = load_json(CATALOG_SCHEMA)
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)

    files = [args.file] if args.file else sorted(CATALOGS_DIR.glob("*.json"))
    if not files:
        print(f"No catalog files found in {CATALOGS_DIR}")
        sys.exit(0)

    print(f"Validating {len(files)} catalog file(s) against {CATALOG_SCHEMA.name}\n")

    total_errors = 0
    for path in files:
        errors = validate_catalog(path, validator)
        if errors:
            print(f"  ✗  {path.name}")
            for e in errors:
                print(f"       {e}")
            total_errors += len(errors)
        else:
            print(f"  ✓  {path.name}")

    print()
    if total_errors:
        print(f"FAILED — {total_errors} error(s) across {len(files)} file(s)")
        sys.exit(1)

    print(f"All {len(files)} catalog file(s) valid.")


if __name__ == "__main__":
    main()
