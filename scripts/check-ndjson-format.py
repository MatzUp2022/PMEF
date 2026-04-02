#!/usr/bin/env python3
"""
check-ndjson-format.py — Enforce NDJSON formatting rules.

Rules enforced:
1. Each non-comment, non-empty line must be valid, self-contained JSON
2. Each object must fit on one line (no multi-line objects)
3. Lines may start with // for comments (PMEF extension for annotated examples)
4. No trailing commas, no JavaScript-style comments inside JSON objects
5. File must end with a newline

Usage:
    python scripts/check-ndjson-format.py
    python scripts/check-ndjson-format.py examples/pump-skid-complete.ndjson
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
EXAMPLES_DIR = ROOT / "examples"

MAX_LINE_LENGTH = 4096  # bytes; warn if exceeded


def check_file(path: Path) -> list[str]:
    errors = []
    content = path.read_text(encoding="utf-8")

    if not content.endswith("\n"):
        errors.append("File does not end with newline")

    for line_no, line in enumerate(content.splitlines(), 1):
        stripped = line.rstrip("\n")

        # Empty lines: allowed
        if not stripped:
            continue

        # Comment lines: allowed (PMEF extension)
        if stripped.strip().startswith("//"):
            continue

        # Must be valid JSON
        try:
            obj = json.loads(stripped)
        except json.JSONDecodeError as e:
            errors.append(f"Line {line_no}: Invalid JSON — {e}")
            continue

        # Must be a JSON object (not array, string, number…)
        if not isinstance(obj, dict):
            errors.append(f"Line {line_no}: Expected JSON object, got {type(obj).__name__}")
            continue

        # Must have @type
        if "@type" not in obj:
            errors.append(f"Line {line_no}: Missing '@type' field (type: {type(obj).__name__})")

        # Line length warning
        if len(stripped.encode("utf-8")) > MAX_LINE_LENGTH:
            errors.append(
                f"Line {line_no}: Very long line ({len(stripped.encode())} bytes). "
                "Consider splitting large embedded geometry into a separate geometry file."
            )

    return errors


def main():
    if len(sys.argv) > 1:
        files = [Path(f) for f in sys.argv[1:]]
    else:
        files = sorted(EXAMPLES_DIR.glob("*.ndjson"))

    if not files:
        print("No .ndjson files to check")
        sys.exit(0)

    total_errors = 0
    for path in files:
        errors = check_file(path)
        if errors:
            print(f"✗  {path.name}:")
            for e in errors:
                print(f"     {e}")
            total_errors += len(errors)
        else:
            print(f"✓  {path.name}")

    if total_errors:
        print(f"\nFAILED — {total_errors} format error(s)")
        sys.exit(1)

    print(f"\nAll {len(files)} file(s) correctly formatted.")


if __name__ == "__main__":
    main()
