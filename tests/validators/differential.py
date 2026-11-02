#!/usr/bin/env python3
"""Differential test: the Rust validator (`pmef validate`) and the Python reference
(scripts/validate-examples.py, jsonschema Draft 2020-12) must reach the same verdict on every
object — the reference instances and systematic mutations of them (docs/AUDIT.md P3.8).

Mutations per object (deterministic): delete a property, wrong JSON type, invalid string for
enums/consts/patterns/formats, out-of-range number, extra property, emptied array, unknown
@type. Only the verdict (valid / invalid) is compared, not the messages. Both validators run
strictly (unknown @type = error); `format` is an annotation in both.

    python tests/validators/differential.py --pmef pmef-rs/target/debug/pmef
    python tests/validators/differential.py --pmef ... --show 20   # print divergent cases
"""
import argparse
import importlib.util
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
_spec = importlib.util.spec_from_file_location("ve", ROOT / "scripts" / "validate-examples.py")
ve = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ve)

SOURCES = ["examples", "tests/diff", "tests/merge", "tests/conformance/fixtures"]
PER_OBJECT = 12  # mutations sampled per object


def objects():
    seen = set()
    for d in SOURCES:
        for f in sorted((ROOT / d).rglob("*.ndjson")):
            for line in f.read_text(encoding="utf-8").splitlines():
                if line.startswith("{") and line not in seen:
                    seen.add(line)
                    yield json.loads(line)


def paths(node, prefix=()):
    """All (path, value) pairs below the root, depth-first."""
    if isinstance(node, dict):
        for k, v in node.items():
            yield prefix + (k,), v
            yield from paths(v, prefix + (k,))
    elif isinstance(node, list):
        for i, v in enumerate(node[:2]):
            yield prefix + (i,), v
            yield from paths(v, prefix + (i,))


def setp(obj, path, value=None, delete=False):
    o = json.loads(json.dumps(obj))
    cur = o
    for p in path[:-1]:
        cur = cur[p]
    if delete:
        if isinstance(cur, dict):
            del cur[path[-1]]
        else:
            cur.pop(path[-1])
    else:
        cur[path[-1]] = value
    return o


def mutations(obj, rnd):
    out = [("type", setp(obj, ("@type",), "pmef:NoSuchType")), ("extra", setp(obj, ("zzUnknown",), 1))]
    cand = []
    for path, v in paths(obj):
        if path == ("@type",):
            continue
        cand.append(("delete", path, None))
        if isinstance(v, bool):
            cand.append(("wrongtype", path, "true"))
        elif isinstance(v, (int, float)):
            cand += [("wrongtype", path, "x"), ("negative", path, -1e9)]
        elif isinstance(v, str):
            cand += [("wrongtype", path, 123), ("badstring", path, "not valid ¤"), ("datetime", path, "2026-13-45T99:00")]
        elif isinstance(v, list):
            cand += [("emptylist", path, []), ("wrongtype", path, {})]
        elif isinstance(v, dict):
            cand += [("extra", path + ("zzUnknown",), 1), ("wrongtype", path, [])]
    for kind, path, val in rnd.sample(cand, min(PER_OBJECT, len(cand))):
        out.append((f"{kind}:{'/'.join(map(str, path))}",
                    setp(obj, path, delete=True) if kind == "delete" else setp(obj, path, val)))
    return out


def python_verdicts(cases):
    schemas, store = ve.load_schemas()
    type_map = ve.build_type_map(schemas)
    return [not ve.validate_object(o, schemas, store, type_map, strict=True) for _, o in cases]


def rust_verdicts(cases, pmef):
    with tempfile.TemporaryDirectory() as d:
        f = Path(d) / "corpus.ndjson"
        f.write_text("".join(json.dumps(o) + "\n" for _, o in cases), encoding="utf-8")
        run = subprocess.run([pmef, "validate", str(f), "--schemas", str(ROOT / "schemas"), "--format", "json",
                              "--strict"],
                             capture_output=True, text=True)
        rep = json.loads(run.stdout)
    bad = {int(e.split(":")[0].split()[1]) for e in rep["errors"]}
    return [i + 1 not in bad for i in range(len(cases))]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pmef", required=True)
    ap.add_argument("--show", type=int, default=0)
    args = ap.parse_args()
    rnd = random.Random(20261101)
    cases = []
    for o in objects():
        cases.append(("original", o))
        cases += mutations(o, rnd)
    py, rs = python_verdicts(cases), rust_verdicts(cases, args.pmef)
    diff = [(c, p, r) for c, p, r in zip(cases, py, rs) if p != r]
    by_kind = {}
    for (kind, o), p, r in diff:
        key = (kind.split(":")[0], o.get("@type"), "py-valid" if p else "rs-valid")
        by_kind[key] = by_kind.get(key, 0) + 1
    print(f"{len(cases)} cases ({sum(py)} valid per Python), {len(diff)} divergent")
    for k, n in sorted(by_kind.items(), key=lambda x: -x[1]):
        print(f"  {n:5d}  {k[0]:10s} {k[1]!s:28s} only {k[2]}")
    for (kind, o), p, r in diff[: args.show]:
        print(f"--- {kind} {o.get('@type')} {o.get('@id')}: Python {'valid' if p else 'invalid'}, Rust {'valid' if r else 'invalid'}")
        if not p:
            schemas, store = ve.load_schemas()
            print("    Python:", ve.validate_object(o, schemas, store, ve.build_type_map(schemas), True)[:2])
    return 1 if diff else 0


if __name__ == "__main__":
    sys.exit(main())
