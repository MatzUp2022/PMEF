#!/usr/bin/env python3
"""Extract mermaid code blocks from a Markdown file and print them to stdout."""
import re
import sys

def main():
    if len(sys.argv) < 2:
        print("Usage: extract-mermaid.py <file.md>", file=sys.stderr)
        sys.exit(1)

    with open(sys.argv[1], encoding="utf-8") as f:
        text = f.read()

    blocks = re.findall(r"```mermaid\s*\n(.*?)```", text, re.DOTALL)
    if not blocks:
        # No mermaid blocks — emit a trivial valid diagram so mmdc doesn't fail
        print("graph LR\n  A-->B")
    else:
        print("\n".join(blocks))

if __name__ == "__main__":
    main()
