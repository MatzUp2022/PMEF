#!/usr/bin/env python3
"""
bundle-schemas.py — Bundle all PMEF JSON Schema files into a single file.

The bundled file contains all $defs from all schema files under a single root
object, enabling validation without cross-file $ref resolution.

Usage:
    python scripts/bundle-schemas.py --output dist/pmef-schemas-bundle.json
    python scripts/bundle-schemas.py --pretty
"""

import json
import argparse
from pathlib import Path

ROOT = Path(__file__).parent.parent
SCHEMAS_DIR = ROOT / "schemas"


def bundle_schemas(schemas_dir: Path) -> dict:
    """Merge all schema $defs into a single bundled schema."""
    all_defs = {}
    metadata = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://pmef.org/schemas/bundle/latest/pmef-bundle.schema.json",
        "title": "PMEF Schema Bundle",
        "description": "All PMEF JSON Schema definitions bundled into a single file for offline validation.",
        "$defs": {},
    }

    for schema_path in sorted(schemas_dir.glob("*.schema.json")):
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        file_defs = schema.get("$defs", {})
        for def_name, definition in file_defs.items():
            if def_name in all_defs:
                print(f"  WARNING: duplicate $def '{def_name}' in {schema_path.name}")
            all_defs[def_name] = definition

        print(f"  Bundled {len(file_defs)} $defs from {schema_path.name}")

    metadata["$defs"] = all_defs
    print(f"\nTotal: {len(all_defs)} $defs bundled from {len(list(schemas_dir.glob('*.schema.json')))} files")
    return metadata


def main():
    parser = argparse.ArgumentParser(description="Bundle PMEF schemas")
    parser.add_argument("--output", type=Path, default=Path("dist/pmef-bundle.schema.json"))
    parser.add_argument("--pretty", action="store_true", help="Pretty-print output")
    args = parser.parse_args()

    args.output.parent.mkdir(parents=True, exist_ok=True)

    print(f"Bundling schemas from {SCHEMAS_DIR}/")
    bundle = bundle_schemas(SCHEMAS_DIR)

    indent = 2 if args.pretty else None
    args.output.write_text(
        json.dumps(bundle, indent=indent, ensure_ascii=False),
        encoding="utf-8"
    )
    print(f"\nBundle written to {args.output} ({args.output.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
