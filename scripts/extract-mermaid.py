#!/usr/bin/env python3
"""Extract mermaid code blocks from a Markdown file and write each to a separate file.

Usage:
    python scripts/extract-mermaid.py <file.md> <output-dir>

Writes each mermaid block to <output-dir>/block-0.mmd, block-1.mmd, etc.
Exits with code 0 and prints the number of blocks found.
"""
import re
import sys
from pathlib import Path


def main():
    if len(sys.argv) < 3:
        print("Usage: extract-mermaid.py <file.md> <output-dir>", file=sys.stderr)
        sys.exit(1)

    md_path = sys.argv[1]
    out_dir = Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)

    with open(md_path, encoding="utf-8") as f:
        text = f.read()

    blocks = re.findall(r"```mermaid\s*\n(.*?)```", text, re.DOTALL)

    for i, block in enumerate(blocks):
        out_file = out_dir / f"block-{i}.mmd"
        out_file.write_text(block, encoding="utf-8")

    print(len(blocks))


if __name__ == "__main__":
    main()
