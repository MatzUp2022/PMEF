"""PMEF conformance test runner (spec 06 §5–§7).

Executes the catalogue in manifest.json against
  * a validator  — reference (repository schemas) or an external command, and
  * a SUT        — the implementation under test, used as a PMEF → tool → PMEF round trip
                   (`identity` = built-in pass-through, used to self-test the suite).
"""
from __future__ import annotations

import argparse
import datetime as _dt
import importlib.util
import json
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONF = Path(__file__).resolve().parent
KINDS = {
    "validator": "validates PMEF files (unit/negative tests only)",
    "converter": "reads and writes PMEF (unit, integration, round-trip)",
    "adapter": "converter that exports from an engineering tool (all tests, incl. INTG-006)",
}
LEVEL_NAMES = {0: "none", 1: "PMEF-Basic", 2: "PMEF-Full", 3: "PMEF-RoundTrip"}
ID_RE = re.compile(r"^urn:pmef:[a-z0-9\-]+:[a-z0-9\-]+:[a-zA-Z0-9\-\._]+$")
EXEMPT_PREFIXES = ("geometry", "customAttributes")

# ── I/O ─────────────────────────────────────────────────────────────────────


def read_ndjson(path: Path) -> list[dict]:
    objs = []
    for line in path.read_text(encoding="utf-8").splitlines():
        s = line.strip()
        if s and not s.startswith("//"):
            objs.append(json.loads(s))
    return objs


def by_id(objs: list[dict]) -> dict[str, dict]:
    return {o.get("@id"): o for o in objs}


# ── validator ───────────────────────────────────────────────────────────────


class ReferenceValidator:
    name = "reference (repository schemas, Python jsonschema)"

    def __init__(self) -> None:
        spec = importlib.util.spec_from_file_location("pmef_validate_examples", ROOT / "scripts" / "validate-examples.py")
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)  # type: ignore[union-attr]
        self._m = mod
        self._schemas, self._store = mod.load_schemas()
        self._types = mod.build_type_map(self._schemas)

    def errors(self, path: Path) -> list[str]:
        out = []
        for o in read_ndjson(path):
            out += self._m.validate_object(o, self._schemas, self._store, self._types)
        return out


class CommandValidator:
    def __init__(self, template: str) -> None:
        self.name = template
        self._tpl = template

    def errors(self, path: Path) -> list[str]:
        cmd = self._tpl.replace("{file}", shlex.quote(str(path)))
        r = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        return [] if r.returncode == 0 else [f"exit {r.returncode}: {(r.stdout + r.stderr).strip()[:300]}"]


# ── system under test ───────────────────────────────────────────────────────


def run_sut(sut: str, src: Path, work: Path) -> tuple[list[dict] | None, str]:
    out = work / f"out-{src.stem}.ndjson"
    if sut == "identity":
        shutil.copyfile(src, out)
    else:
        cmd = sut.replace("{input}", shlex.quote(str(src))).replace("{output}", shlex.quote(str(out)))
        r = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        if r.returncode != 0:
            return None, f"SUT exited {r.returncode}: {(r.stdout + r.stderr).strip()[:300]}"
    if not out.exists():
        return None, "SUT wrote no output file"
    try:
        return read_ndjson(out), ""
    except (json.JSONDecodeError, OSError) as e:
        return None, f"SUT output is not valid NDJSON: {e}"


# ── comparison helpers ──────────────────────────────────────────────────────


def tolerance(key: str) -> float:
    k = key.lower()
    if "pressure" in k:
        return 1.0          # Pa
    if "temperature" in k:
        return 0.01         # K
    if "flow" in k:
        return 0.001        # m³/h
    if "efficiency" in k:
        return 0.0001       # 0.01 % as fraction
    if "angle" in k or "direction" in k or "axis" in k:
        return 0.001        # degrees / unit vector
    return 0.01             # mm (lengths) and everything else


def equal(a, b, key: str = "") -> bool:
    if isinstance(a, bool) or isinstance(b, bool):
        return a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) <= tolerance(key)
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(equal(a[k], b[k], k) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(equal(x, y, key) for x, y in zip(a, b))
    return a == b


def leaves(v, path: str = ""):
    if isinstance(v, dict):
        for k, x in v.items():
            yield from leaves(x, f"{path}.{k}" if path else k)
    elif isinstance(v, list) and v:
        for i, x in enumerate(v):
            yield from leaves(x, f"{path}[{i}]")
    else:
        yield path, v


def get(o: dict, path: str):
    cur = o
    for part in re.findall(r"[^.\[\]]+|\[\d+\]", path):
        if part.startswith("["):
            i = int(part[1:-1])
            if not isinstance(cur, list) or i >= len(cur):
                return _MISSING
            cur = cur[i]
        else:
            if not isinstance(cur, dict) or part not in cur:
                return _MISSING
            cur = cur[part]
    return cur


_MISSING = object()

IDENTITY_FIELDS = {
    None: ["@id", "revision.revisionId", "revision.changeState"],
    "pmef:PipingNetworkSystem": ["lineNumber"],
    "pmef:InstrumentObject": ["tagNumber"],
    "pmef:Weld": ["weldSpec.weldNumber"],
    "pmef:SteelMember": ["memberMark"],
    "pmef:Spool": ["spoolMark"],
}


def fidelity(inp: list[dict], out: list[dict]) -> dict:
    """Spec 06 §6: aggregate and identity-field round-trip fidelity."""
    out_ids = by_id(out)
    total = passed = id_total = id_passed = 0
    failures = []
    for o in inp:
        if o.get("@type") == "pmef:ParametricGeometry":
            continue  # geometry exempt (§6.5)
        p = out_ids.get(o.get("@id"))
        ident = set(IDENTITY_FIELDS[None]) | set(IDENTITY_FIELDS.get(o.get("@type"), []))
        if "equipmentBasic" in o:
            ident.add("equipmentBasic.tagNumber")
        for path, val in leaves(o):
            if path.startswith(EXEMPT_PREFIXES):
                continue
            ok = p is not None and (got := get(p, path)) is not _MISSING and equal(val, got, path.rsplit(".", 1)[-1])
            total += 1
            passed += ok
            if path in ident:
                id_total += 1
                id_passed += ok
            if not ok and len(failures) < 20:
                failures.append(f"{o.get('@id')} {path}")
    return {"aggregate": passed / total if total else 1.0, "identity": id_passed / id_total if id_total else 1.0,
            "attributes": total, "failures": failures}


# ── integration checks (on SUT output) ──────────────────────────────────────

PHYSICAL_ID = "urn:pmef:obj:"


# claimed domain -> schema files whose object types belong to it (INTG-006 checks exported objects only)
DOMAIN_SCHEMAS = {
    "piping": {"pmef-piping-component.schema.json"},
    "equipment": {"pmef-equipment.schema.json"},
    "ei": {"pmef-ei.schema.json"},
    "steel": {"pmef-steel.schema.json"},
}


def domain_types(domains: list[str]) -> set[str]:
    """@types belonging to the claimed domains (derived from the schemas)."""
    spec = importlib.util.spec_from_file_location("pmef_validate_examples", ROOT / "scripts" / "validate-examples.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)  # type: ignore[union-attr]
    schemas, _ = mod.load_schemas()
    files = set().union(*(DOMAIN_SCHEMAS.get(d, set()) for d in domains)) if domains else set()
    return {t for t, f in mod.build_type_map(schemas).items() if f in files}


def intg(tid: str, objs: list[dict], exported_types: set[str] | None = None) -> list[str]:
    ids = by_id(objs)
    if tid == "INTG-001":
        headers = [o for o in objs if o.get("@type") == "pmef:FileHeader"]
        p = []
        if len(headers) != 1:
            p.append(f"{len(headers)} FileHeader objects")
        if objs and objs[0].get("@type") != "pmef:FileHeader":
            p.append(f"first object is {objs[0].get('@type')}")
        return p
    if tid == "INTG-002":
        seen, p = set(), []
        for o in objs:
            i = o.get("@id", "")
            if i in seen:
                p.append(f"duplicate {i}")
            if not ID_RE.match(i):
                p.append(f"malformed {i}")
            seen.add(i)
        return p
    if tid == "INTG-003":
        return [f"{o['@id']} → {o['isPartOf']}" for o in objs
                if isinstance(o.get("isPartOf"), str) and o["isPartOf"] not in ids]
    if tid == "INTG-004":
        p = []
        for o in objs:
            for port in o.get("ports", []) or []:
                t = port.get("connectedTo")
                if not t:
                    continue
                if t not in ids:
                    p.append(f"{o['@id']}.{port.get('portId')} → {t} unresolved")
                elif not any(q.get("connectedTo") == o["@id"] for q in ids[t].get("ports", []) or []):
                    p.append(f"{o['@id']} → {t} not reciprocal")
        return p
    if tid == "INTG-005":
        return [o["@id"] for o in objs if o.get("@type") != "pmef:FileHeader"
                and not ({"revisionId", "changeState"} <= set((o.get("revision") or {}).keys()))]
    if tid == "INTG-006":
        heq = {o.get("sourceId") for o in objs if o.get("@type") == "pmef:HasEquivalentIn"}
        return [o["@id"] for o in objs if o.get("@id", "").startswith(PHYSICAL_ID) and o["@id"] not in heq
                and (exported_types is None or o.get("@type") in exported_types)]
    raise KeyError(tid)


# ── round-trip checks ───────────────────────────────────────────────────────


def _cmp_field(inp, out, types, path, exact=True) -> list[str]:
    p = []
    for o in inp:
        if types and o.get("@type") not in types:
            continue
        a = get(o, path)
        if a is _MISSING:
            continue
        q = out.get(o["@id"])
        b = get(q, path) if q is not None else _MISSING
        if b is _MISSING or (a != b if exact else not equal(a, b, path.rsplit(".", 1)[-1])):
            p.append(f"{o['@id']} {path}")
    return p


EQUIPMENT = None  # objects carrying equipmentBasic


def rt(tid: str, inp: list[dict], out_list: list[dict], level: int, fid: dict, extra_fid: list[dict]) -> list[str]:
    out = by_id(out_list)
    pipes = {"pmef:PipingNetworkSystem", "pmef:PipingSegment"}
    if tid == "RT-001":
        return [i for i in by_id(inp) if i not in out]
    if tid == "RT-002":
        return [f"{o['@id']}: {o['@type']} → {out[o['@id']].get('@type')}" for o in inp
                if o["@id"] in out and out[o["@id"]].get("@type") != o["@type"]]
    if tid == "RT-003":
        return _cmp_field(inp, out, {"pmef:PipingNetworkSystem"}, "lineNumber")
    if tid == "RT-004":
        return _cmp_field(inp, out, None, "equipmentBasic.tagNumber")
    if tid == "RT-005":
        return _cmp_field(inp, out, None, "revision.revisionId") + _cmp_field(inp, out, None, "revision.changeState")
    if tid == "RT-006":
        p = []
        for o in inp:
            if "nozzles" not in o:
                continue
            q = out.get(o["@id"], {})
            a = [(n.get("nozzleId"), n.get("nominalDiameter")) for n in o["nozzles"]]
            b = [(n.get("nozzleId"), n.get("nominalDiameter")) for n in q.get("nozzles", [])]
            if not equal(a, b, "nominalDiameter"):
                p.append(o["@id"])
        return p
    if tid == "RT-007":
        return _cmp_field(inp, out, None, "isPartOf")
    if tid == "RT-008":
        return _cmp_field(inp, out, pipes, "designConditions", exact=False)
    if tid == "RT-009":
        p = []
        for o in inp:
            for i, port in enumerate(o.get("ports", []) or []):
                q = out.get(o["@id"], {})
                qp = (q.get("ports") or [])
                match = next((x for x in qp if x.get("portId") == port.get("portId")), None)
                if match is None or not equal(port.get("coordinate"), match.get("coordinate"), "coordinate") \
                        or not equal(port.get("direction"), match.get("direction"), "direction"):
                    p.append(f"{o['@id']}.{port.get('portId')}")
        return p
    if tid == "RT-010":
        return _cmp_field(inp, out, {"pmef:Weld"}, "weldSpec.weldNumber")
    if tid == "RT-011":
        return _cmp_field(inp, out, None, "pumpSpec", exact=False)
    if tid == "RT-012":
        return _cmp_field(inp, out, None, "vesselDesign", exact=False)
    if tid == "RT-013":
        return _cmp_field(inp, out, pipes, "specification", exact=False)
    if tid == "RT-014":
        return _cmp_field(inp, out, None, "catalogRef", exact=False)
    if tid == "RT-015":
        rels = {o["@type"] for o in inp if "sourceId" in o}
        return _cmp_field(inp, out, rels, "sourceId") + _cmp_field(inp, out, rels, "targetId")
    if tid == "RT-016":
        known = [o for o in inp if not o["@type"].startswith("pmef:Future")]
        p = [f"{o['@id']} missing" for o in known if o["@id"] not in out]
        for o in known:
            for k in o:
                if k.startswith("x-") and (o["@id"] not in out or out[o["@id"]].get(k) != o[k]):
                    p.append(f"{o['@id']} unknown property {k} not preserved")
        return p
    if tid == "RT-017":
        return [o["@id"] for o in inp if "customAttributes" in o
                and out.get(o["@id"], {}).get("customAttributes") != o["customAttributes"]]
    if tid == "RT-018":
        need = THRESHOLDS.get(str(max(level, 2)), 0.95)
        p = []
        if fid["aggregate"] < need:
            p.append(f"DS-01 aggregate fidelity {fid['aggregate']:.4f} < {need}")
        if fid["identity"] < 1.0:
            p.append(f"identity-field fidelity {fid['identity']:.4f} < 1.0")
        if level >= 3 and not any(f["aggregate"] >= need for f in extra_fid):
            p.append("no domain benchmark (DS-02/03/04) reaches the L3 threshold")
        return p
    raise KeyError(tid)


THRESHOLDS: dict[str, float] = {}

# ── main ────────────────────────────────────────────────────────────────────


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="run_tests.py", description="PMEF conformance test suite (spec 06 §5)")
    ap.add_argument("--level", type=int, choices=[1, 2, 3], default=3, help="conformance level to test (default 3)")
    ap.add_argument("--report", choices=["text", "json"], default="text")
    ap.add_argument("--output", type=Path, help="write the report to this file instead of stdout")
    ap.add_argument("--kind", choices=sorted(KINDS), default="converter", help="kind of implementation under test")
    ap.add_argument("--sut", default="identity",
                    help="round-trip command with {input} and {output} placeholders, or 'identity'")
    ap.add_argument("--validator", default="reference",
                    help="validation command with a {file} placeholder (exit 0 = valid), or 'reference'")
    ap.add_argument("--name", default="pmef-reference-identity", help="implementationName for the report")
    ap.add_argument("--version", dest="impl_version", default="0.10.0", help="implementationVersion for the report")
    ap.add_argument("--domains", default="piping,equipment,ei,steel", help="claimed domains (comma-separated)")
    args = ap.parse_args(argv)

    manifest = json.loads((CONF / "manifest.json").read_text(encoding="utf-8"))
    THRESHOLDS.update(manifest["thresholds"])
    validator = ReferenceValidator() if args.validator == "reference" else CommandValidator(args.validator)
    tests = [t for t in manifest["tests"] if t["level"] <= args.level]
    exported = domain_types([d for d in args.domains.split(",") if d])
    results, ds_fid, extra_fid = [], None, []

    with tempfile.TemporaryDirectory(prefix="pmef-conformance-") as tmp:
        work = Path(tmp)
        outputs: dict[str, tuple[list[dict], list[dict] | None, str]] = {}

        def round_trip(key: str) -> tuple[list[dict], list[dict] | None, str]:
            if key not in outputs:
                src = ROOT / manifest["datasets"][key] if key in manifest["datasets"] else CONF / key
                out, err = run_sut(args.sut, src, work)
                outputs[key] = (read_ndjson(src), out, err)
            return outputs[key]

        if args.kind != "validator":
            inp, out, err = round_trip("PMEF-DS-01")
            ds_fid = fidelity(inp, out) if out is not None else {"aggregate": 0.0, "identity": 0.0,
                                                                  "attributes": 0, "failures": [err]}
            if args.level >= 3:
                for name, path in [("PMEF-DS-02", "examples/heat-exchanger-station.ndjson"),
                                   ("PMEF-DS-03", "examples/compressor-train.ndjson"),
                                   ("PMEF-DS-04", "examples/ei-loop-fic-10101.ndjson")]:
                    manifest["datasets"].setdefault(name, path)
                    i2, o2, _ = round_trip(name)
                    f = fidelity(i2, o2) if o2 is not None else {"aggregate": 0.0, "identity": 0.0}
                    extra_fid.append({"dataset": name, **{k: f[k] for k in ("aggregate", "identity")}})

        for t in tests:
            t0 = time.perf_counter()
            status, detail = "PASS", ""
            try:
                if t["category"] in ("unit", "negative"):
                    errs = validator.errors(CONF / t["fixture"])
                    got = "invalid" if errs else "valid"
                    if got != t["expect"]:
                        status, detail = "FAIL", f"expected {t['expect']}, got {got}: {'; '.join(errs[:2])}"
                elif args.kind == "validator":
                    status, detail = "SKIP", "not applicable to kind 'validator'"
                elif t["id"] == "INTG-006" and args.kind != "adapter":
                    status, detail = "SKIP", "applies to adapters exporting from a tool only (CL2-05)"
                else:
                    inp, out, err = round_trip(t["dataset"])
                    if out is None:
                        status, detail = "FAIL", err
                    else:
                        problems = intg(t["id"], out, exported) if t["category"] == "integration" else \
                            rt(t["id"], inp, out, args.level, ds_fid, extra_fid)
                        if problems:
                            status, detail = "FAIL", f"{len(problems)} issue(s): " + "; ".join(problems[:5])
            except Exception as e:  # a crashing check is a failed test, never a silent pass
                status, detail = "FAIL", f"check error: {e!r}"
            results.append({"id": t["id"], "name": t["name"], "category": t["category"], "level": t["level"],
                            "status": status, "duration_ms": round((time.perf_counter() - t0) * 1000, 2),
                            **({"detail": detail} if detail else {})})

    achieved = 0
    for lvl in (1, 2, 3):
        if lvl > args.level:
            break
        relevant = [r for r in results if r["level"] <= lvl]
        if any(r["status"] == "FAIL" for r in relevant) or args.kind == "validator":
            break
        if lvl >= 2 and ds_fid and ds_fid["aggregate"] < float(THRESHOLDS[str(lvl)]):
            break
        achieved = lvl

    summary = {s: sum(1 for r in results if r["status"] == k) for s, k in
               (("passed", "PASS"), ("failed", "FAIL"), ("skipped", "SKIP"))}
    validator_ok = args.kind == "validator" and summary["failed"] == 0
    report = {
        "pmefVersion": manifest["suiteVersion"],
        "implementationName": args.name,
        "implementationVersion": args.impl_version,
        "implementationKind": args.kind,
        "conformanceLevel": achieved,
        "conformanceName": "validator (unit/negative tests)" if args.kind == "validator" else LEVEL_NAMES[achieved],
        **({"validatorConformance": validator_ok} if args.kind == "validator" else {}),
        "levelTested": args.level,
        "domains": [d for d in args.domains.split(",") if d],
        "testDate": _dt.date.today().isoformat(),
        "sut": args.sut,
        "validator": validator.name,
        "summary": {"total": len(results), **summary},
        "fidelity": {"dataset": "PMEF-DS-01",
                     "aggregateFidelity": round(ds_fid["aggregate"], 4) if ds_fid else None,
                     "identityFieldFidelity": round(ds_fid["identity"], 4) if ds_fid else None,
                     "attributesTested": ds_fid.get("attributes") if ds_fid else None,
                     "additional": [{**f, "aggregate": round(f["aggregate"], 4),
                                     "identity": round(f["identity"], 4)} for f in extra_fid]},
        "tests": results,
    }

    check_report(report)
    if args.report == "json":
        text = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    else:
        text = render_text(report, ds_fid)
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)
    if args.kind == "validator":
        return 0 if validator_ok else 1
    return 0 if achieved >= args.level else 1


def check_report(report: dict) -> None:
    """The runner's own output must satisfy report.schema.json (spec 06 §7.1)."""
    from jsonschema import Draft202012Validator

    schema = json.loads((CONF / "report.schema.json").read_text(encoding="utf-8"))
    errors = [e.message for e in Draft202012Validator(schema).iter_errors(report)]
    if errors:
        raise SystemExit("internal error — report violates report.schema.json: " + "; ".join(errors[:3]))


def render_text(r: dict, fid: dict | None) -> str:
    line = "─" * 64
    s = r["summary"]
    out = [f"PMEF Conformance Test Suite  (v{r['pmefVersion']})", line,
           f"  Implementation: {r['implementationName']} {r['implementationVersion']}  [{r['implementationKind']}]",
           f"  Total: {s['total']}  │  ✓ {s['passed']}  ✗ {s['failed']}  ○ {s['skipped']}",
           f"  Level tested: {r['levelTested']}   Domains: {', '.join(r['domains'])}"]
    if fid:
        f = r["fidelity"]
        out += ["", "  Round-Trip Fidelity (PMEF-DS-01):",
                f"    Aggregate:        {100 * f['aggregateFidelity']:7.2f}%  ({f['attributesTested']} attributes)",
                f"    Identity fields:  {100 * f['identityFieldFidelity']:7.2f}%"]
        for a in f["additional"]:
            out.append(f"    {a['dataset']}:       {100 * a['aggregate']:7.2f}%")
    fails = [t for t in r["tests"] if t["status"] == "FAIL"]
    if fails:
        out += ["", "  Failed tests:"] + [f"    ✗ {t['id']}  {t['name']} — {t.get('detail', '')}" for t in fails]
    if r["implementationKind"] == "validator":
        ok = r["validatorConformance"]
        verdict = f"  Conformance: {r['conformanceName']} {'✓' if ok else '✗'}  (levels require read + write)"
    else:
        ok = r["conformanceLevel"] >= r["levelTested"]
        verdict = f"  Conformance: {r['conformanceName']} (Level {r['conformanceLevel']}) {'✓' if ok else '✗'}"
    out += ["", verdict, line]
    return "\n".join(out) + "\n"
