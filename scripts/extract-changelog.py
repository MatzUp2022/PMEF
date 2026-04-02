#!/usr/bin/env python3
"""
extract-changelog.py — Extract release notes for a given version from CHANGELOG.md.

Usage:
    python scripts/extract-changelog.py --version v0.9.0-rc --output /tmp/release-notes.md
"""

import argparse
import re
import sys
from pathlib import Path

CHANGELOG = Path(__file__).parent.parent / "CHANGELOG.md"


def extract_section(text: str, version: str) -> str:
    """Extract the changelog section for the given version tag."""
    # Strip leading 'v' if present to match header like [0.9.0-rc]
    ver = version.lstrip("v")

    # Match from ## [<version>] to the next ## [ or end of file
    pattern = rf"(## \[{re.escape(ver)}\].*?)(?=\n## \[|\Z)"
    match = re.search(pattern, text, re.DOTALL)

    if match:
        return match.group(1).strip()
    return ""


def main():
    parser = argparse.ArgumentParser(description="Extract changelog section for a release")
    parser.add_argument("--version", required=True, help="Version tag (e.g. v0.9.0-rc)")
    parser.add_argument("--output", required=True, help="Output file path")
    args = parser.parse_args()

    if not CHANGELOG.exists():
        print(f"ERROR: {CHANGELOG} not found", file=sys.stderr)
        sys.exit(1)

    text = CHANGELOG.read_text(encoding="utf-8")
    section = extract_section(text, args.version)

    if not section:
        print(f"WARNING: No changelog section found for {args.version}", file=sys.stderr)
        section = f"Release {args.version}\n\nNo changelog entry found."

    Path(args.output).write_text(section, encoding="utf-8")
    print(f"Release notes written to {args.output}")


if __name__ == "__main__":
    main()
