#!/usr/bin/env python3
"""Golden test for scripts/check-catalogs.py: every rule must fire on the deliberately broken
catalogs in tests/catalogs/broken/ exactly as recorded in expected.txt, and the real
catalogs/ must pass.

    python tests/catalogs/run.py            # compare
    python tests/catalogs/run.py --update   # re-record after an intended change
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CHECK = [sys.executable, str(ROOT / "scripts/check-catalogs.py")]
run = subprocess.run(CHECK + ["--dir", str(HERE / "broken")], capture_output=True, text=True)
expected = HERE / "expected.txt"
if "--update" in sys.argv:
    expected.write_text(run.stdout, encoding="utf-8")
    sys.exit(0)
if run.returncode != 1 or run.stdout != expected.read_text(encoding="utf-8"):
    print("check-catalogs output differs from tests/catalogs/expected.txt:\n" + run.stdout)
    sys.exit(1)
real = subprocess.run(CHECK, capture_output=True, text=True)
if real.returncode != 0:
    print(real.stdout)
    sys.exit(1)
print(f"✓ check-catalogs: {run.stdout.count('✗') - 1} findings on the broken fixture as expected, catalogs/ clean")
