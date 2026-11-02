#!/usr/bin/env python3
"""Golden test for scripts/check-catalogs.py: every rule must fire on the deliberately broken
catalogs in tests/catalogs/broken/ exactly as recorded in expected.txt, the catalog schema must
reject malformed flange fields, and the real catalogs/ must pass.

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
# the catalog schema must reject malformed flange fields (types, enums, bolt-hole count)
import json  # noqa: E402
from jsonschema import Draft202012Validator  # noqa: E402

schema = json.loads((ROOT / "schemas/pmef-catalog.schema.json").read_text(encoding="utf-8"))
entry = Draft202012Validator({"$defs": schema["$defs"], "$ref": "#/$defs/flangeDimensionEntry"})
good = {"catalogId": "B165-150-DN50", "nominalDiameter": 50, "pressureClass": 150, "flangeOD_mm": 152.4,
        "boltHoles": 4, "boltDiameter_mm": 15.9, "boltHoleDiameter_mm": 19.1, "facingType": "RF"}
bad = [dict(good, boltHoles=6), dict(good, facingType="XX"), dict(good, flangeThickness_mm=-1),
       dict(good, boltDiameter_mm="5/8")]
if list(entry.iter_errors(good)) or any(not list(entry.iter_errors(b)) for b in bad):
    print("pmef-catalog schema: flange entry fields not enforced as expected")
    sys.exit(1)

real = subprocess.run(CHECK, capture_output=True, text=True)
if real.returncode != 0:
    print(real.stdout)
    sys.exit(1)
print(f"✓ check-catalogs: {run.stdout.count('✗') - 1} findings on the broken fixture as expected, catalogs/ clean")
