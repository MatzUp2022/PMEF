#!/usr/bin/env python3
"""RFC-0012 stand-in values: the example must validate, and every rule must reject its
broken variant (UNSPECIFIED without a placeholders entry for /damperType, CUSTOM without
damperTypeCustom, damperTypeCustom with another damperType, malformed placeholder entries). The piping example
(valves as VALVE_UNSPECIFIED with placeholders) must validate as well, and malformed entries on a
piping component must be rejected.

    python tests/placeholders/run.py
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
EXAMPLE = ROOT / "examples/hvac-damper-placeholder.ndjson"
VALIDATE = [sys.executable, str(ROOT / "scripts/validate-examples.py"), "--file"]
BASE = [json.loads(line) for line in EXAMPLE.read_text(encoding="utf-8").splitlines() if line.startswith("{")]
PIPING = ROOT / "examples/piping-valve-placeholder.ndjson"
PBASE = [json.loads(line) for line in PIPING.read_text(encoding="utf-8").splitlines() if line.startswith("{")]


def valid(objs) -> bool:
    with tempfile.NamedTemporaryFile("w", suffix=".ndjson", delete=False, encoding="utf-8") as f:
        f.write("\n".join(json.dumps(o) for o in objs) + "\n")
    try:
        return subprocess.run(VALIDATE + [f.name], capture_output=True, text=True).returncode == 0
    finally:
        Path(f.name).unlink()


def damper(objs, i):
    return [o for o in objs if o["@type"] == "pmef:DuctDamper"][i]


CASES = {
    "UNSPECIFIED without placeholders": lambda o: damper(o, 0).pop("placeholders"),
    "UNSPECIFIED, placeholder for another field": lambda o: damper(o, 0)["placeholders"][0].update(path="/length"),
    "CUSTOM without damperTypeCustom": lambda o: damper(o, 1).pop("damperTypeCustom"),
    "damperTypeCustom with damperType FIRE": lambda o: damper(o, 1).update(damperType="FIRE"),
    "path is not a JSON Pointer": lambda o: damper(o, 0)["placeholders"][0].update(path="damperType"),
    "unknown reason": lambda o: damper(o, 0)["placeholders"][0].update(reason="LATER"),
    "unknown field in placeholder": lambda o: damper(o, 0)["placeholders"][0].update(owner="x"),
    "damperTypeCustom without namespace": lambda o: damper(o, 1).update(damperTypeCustom="BLAST"),
}

def valve(objs, i):
    return [o for o in objs if o["@type"] == "pmef:Valve"][i]


PCASES = {
    "valve: empty placeholders": lambda o: valve(o, 0).update(placeholders=[]),
    "valve: path is not a JSON Pointer": lambda o: valve(o, 0)["placeholders"][0].update(path="componentClass"),
    "valve: unknown reason": lambda o: valve(o, 1)["placeholders"][0].update(reason="GUESS"),
}

fail = 0
for base, label in ((BASE, "damper"), (PBASE, "valve")):
    if not valid(json.loads(json.dumps(base))):
        print(f"FAIL {label} example does not validate")
        fail += 1
for name, mod in list(CASES.items()) + [(n, (m, PBASE)) for n, m in PCASES.items()]:
    mod, base = mod if isinstance(mod, tuple) else (mod, BASE)
    objs = json.loads(json.dumps(base))
    mod(objs)
    ok = not valid(objs)
    print(("ok   " if ok else "FAIL ") + "rejected: " + name)
    fail += not ok
sys.exit(1 if fail else 0)
