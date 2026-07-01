#!/usr/bin/env python3
"""
check-topology.py — Topology / connectivity integrity checks for PMEF NDJSON.

For each self-contained NDJSON dataset, this validates the properties that must
hold *within* a single dataset (as opposed to cross-layer links such as
'isPartOf' → unit or 'targetId' → functional, which legitimately point outside
the file):

1. Unique '@id'          — no object identifier appears twice.
2. 'connectedTo' resolves — every port-level or object-level 'connectedTo'
                            references an '@id' defined in the same dataset.
3. Port reciprocity      — if object A connects to object B (and B is in the
                            dataset), then B must connect back to A.

Cross-dataset connectivity is supported via --allow-external, which downgrades
unresolved 'connectedTo' targets to warnings (boundary connections).

Usage:
    python scripts/check-topology.py
    python scripts/check-topology.py --file examples/pump-skid-complete.ndjson
    python scripts/check-topology.py --allow-external
    python scripts/check-topology.py --verbose
"""

import json
import sys
import argparse
from pathlib import Path

ROOT = Path(__file__).parent.parent
EXAMPLES_DIR = ROOT / "examples"


def load_objects(path: Path):
    """Yield (line_no, object) for each non-comment JSON line."""
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        try:
            yield line_no, json.loads(stripped)
        except json.JSONDecodeError as e:
            yield line_no, e


def collect_connections(body) -> list[str]:
    """Return every 'connectedTo' string value found anywhere within an object."""
    found: list[str] = []

    def walk(node):
        if isinstance(node, dict):
            for key, value in node.items():
                if key == "connectedTo" and isinstance(value, str):
                    found.append(value)
                else:
                    walk(value)
        elif isinstance(node, list):
            for item in node:
                walk(item)

    walk(body)
    return found


def check_file(path: Path, allow_external: bool, verbose: bool) -> tuple[int, int]:
    """Return (error_count, warning_count) for a single dataset."""
    ids: set[str] = set()
    duplicate_ids: set[str] = set()
    edges: list[tuple[str, str]] = []  # (source @id, connectedTo target)
    errors: list[str] = []
    warnings: list[str] = []

    objects: list[tuple[int, dict]] = []
    for line_no, obj in load_objects(path):
        if isinstance(obj, json.JSONDecodeError):
            errors.append(f"Line {line_no}: invalid JSON — {obj}")
            continue
        oid = obj.get("@id")
        if isinstance(oid, str):
            if oid in ids:
                duplicate_ids.add(oid)
            ids.add(oid)
        objects.append((line_no, obj))

    for oid in sorted(duplicate_ids):
        errors.append(f"Duplicate @id: {oid}")

    # Build edges from every object's connectedTo values.
    for _line_no, obj in objects:
        oid = obj.get("@id")
        if not isinstance(oid, str):
            continue
        for target in collect_connections(obj):
            if target == oid:
                continue  # self-reference — ignore
            edges.append((oid, target))

    edge_set = set(edges)

    for source, target in edges:
        if target not in ids:
            msg = f"connectedTo target not found: {source} → {target}"
            if allow_external:
                warnings.append(msg + " (treated as external boundary)")
            else:
                errors.append(msg)
            continue
        # Reciprocity: both endpoints are in the dataset, so B must link back.
        if (target, source) not in edge_set:
            errors.append(f"Non-reciprocal connection: {source} → {target} "
                          f"(no return connection from {target})")

    if verbose or errors or warnings:
        status = "✗" if errors else ("!" if warnings else "✓")
        print(f"  {status}  {path.name}  "
              f"({len(ids)} objects, {len(edges)} connections)")
        for e in errors:
            print(f"       ERROR: {e}")
        for w in warnings:
            print(f"       warn:  {w}")
    else:
        print(f"  ✓  {path.name}  ({len(ids)} objects, {len(edges)} connections)")

    return len(errors), len(warnings)


def main() -> None:
    parser = argparse.ArgumentParser(description="PMEF topology integrity checks")
    parser.add_argument("--file", type=Path, help="Check a specific file only")
    parser.add_argument("--allow-external", action="store_true",
                        help="Treat unresolved connectedTo targets as warnings")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    files = [args.file] if args.file else sorted(EXAMPLES_DIR.glob("*.ndjson"))
    if not files:
        print(f"No .ndjson files found in {EXAMPLES_DIR}")
        sys.exit(0)

    print(f"Checking topology of {len(files)} dataset(s)\n")

    total_errors = 0
    total_warnings = 0
    for path in files:
        errors, warnings = check_file(path, args.allow_external, args.verbose)
        total_errors += errors
        total_warnings += warnings

    print()
    if total_errors:
        print(f"FAILED — {total_errors} topology error(s), "
              f"{total_warnings} warning(s) across {len(files)} file(s)")
        sys.exit(1)

    msg = f"All {len(files)} dataset(s) topologically consistent."
    if total_warnings:
        msg += f" ({total_warnings} warning(s))"
    print(msg)


if __name__ == "__main__":
    main()
