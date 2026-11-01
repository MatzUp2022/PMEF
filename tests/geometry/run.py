#!/usr/bin/env python3
"""Golden test for scripts/check-geometry.py: every equipment/steel rule must fire on the
negative fixture exactly as recorded in expected.txt (and nothing else, so the correct
counterparts V-OK, P-BOX#OK, N-OK, CN-OK and line L-1 stay clean).

    python tests/geometry/run.py            # compare
    python tests/geometry/run.py --update   # re-record after an intended change
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
run = subprocess.run([sys.executable, str(ROOT / "scripts/check-geometry.py"),
                      str(HERE / "equipment-steel-negative.ndjson")], capture_output=True, text=True)
expected = HERE / "expected.txt"
if "--update" in sys.argv:
    expected.write_text(run.stdout, encoding="utf-8")
    sys.exit(0)
if run.returncode != 1 or run.stdout != expected.read_text(encoding="utf-8"):
    print("check-geometry output differs from tests/geometry/expected.txt:\n" + run.stdout)
    sys.exit(1)
print(f"✓ check-geometry negative fixture: {run.stdout.count(chr(10)) - 1} findings as expected")
