#!/usr/bin/env python3
"""PMEF conformance test suite — entry point (spec 06 §5.1).

    python tests/run_tests.py                       # all tests (level 3), text summary
    python tests/run_tests.py --level 1             # Level 1 tests only
    python tests/run_tests.py --report json > conformance-report.json
    python tests/run_tests.py --kind adapter --sut "my-tool roundtrip {input} {output}"

See tests/README.md.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "conformance"))

from runner import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
