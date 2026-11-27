#!/usr/bin/env python3
"""Reference implementation of the PMEF catalog-snapping algorithms `pmef-snap/1` (RFC-0002)
and `pmef-snap/2` (RFC-0011).

Deliberately small and dependency-free so it can be read next to the RFCs. The Rust
implementation (`pmef_core::snapping`) must produce identical results on
tests/snapping/vectors.json (pmef-snap/1) and tests/snapping/vectors-v2.json (pmef-snap/2).

`pmef-snap/2` is a superset: for the kinds `pipe` and `i-section` it chooses the same entry
with the same score, residuals, runner-up and ambiguity flag as `pmef-snap/1`.

    python tests/snapping/reference_snap.py            # check all vectors
    python tests/snapping/reference_snap.py --print    # print computed results
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
V1, V2 = "pmef-snap/1", "pmef-snap/2"
ALGORITHM = V1  # default, kept for callers of pmef-snap/1
AMBIGUITY_MARGIN = 0.1  # runner-up closer than this ⇒ ambiguous (needs human review); part of the algorithm identity
ANGLE_SELECTOR_TOL_DEG = 2.0

PIPE_TOL = {"outsideDiameter": (1.0, 0.01), "wallThickness": (0.5, 0.15)}
SECTION_TOL = {"h": (2.0, 0.02), "b": (2.0, 0.02), "tw": (1.0, 0.15), "tf": (1.0, 0.15)}
OD = (1.0, 0.01)
LEN = (2.0, 0.02)


def _num(e: dict, *names: str):
    """First numeric field among `names` (first present wins), else None."""
    for n in names:
        v = e.get(n)
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            return float(v)
    return None


def _dn(e: dict):
    return _num(e, "nominalDiameter_mm", "DN", "nominalDiameter")


def _class_pipe_od(catalog: dict, dn):
    """OD of the first PIPE entry of the same piping class with this DN (catalog order)."""
    if dn is None:
        return None
    for p in catalog.get("entries", []):
        if p.get("componentClass") == "PIPE" and _dn(p) == dn:
            od = _num(p, "outsideDiameter_mm")
            if od is not None:
                return od
    return None


def _sorted_pair(a, b, which):
    if a is None or b is None:
        return None
    return max(a, b) if which == 0 else min(a, b)


# ── kinds ─────────────────────────────────────────────────────────────────────────────
# Each kind: params (ordered, first = primary), required, tolerance table, candidate filter,
# value extractor, selectors, families (for the optional family restriction), algorithms.

def _profile_filter(families):
    def f(catalog, e, family):
        return (catalog.get("catalogType") == "STEEL_PROFILES" and e.get("family") in families
                and (family is None or e.get("family") == family))
    return f


def _class_filter(classes):
    def f(catalog, e, family):
        return (catalog.get("catalogType") == "PIPING_CLASS" and e.get("componentClass") in classes
                and (family is None or e.get("componentClass") == family))
    return f


def _flange_filter(catalog, e, family):
    ct = catalog.get("catalogType")
    if family is not None:  # restriction by componentClass (FLANGE, BLIND_FLANGE); dimension tables have none
        return ct == "PIPING_CLASS" and e.get("componentClass") == family
    return ct == "FLANGE_DIMENSIONS" or (ct == "PIPING_CLASS" and e.get("componentClass") in ("FLANGE", "BLIND_FLANGE"))


def _section_values(catalog, e, p):
    return _num(e, *{"h": ("h_mm", "d_mm"), "b": ("b_mm", "bf_mm"), "tw": ("tw_mm",), "tf": ("tf_mm",)}[p])


def _rect_values(catalog, e, p):
    if p == "t":
        return _num(e, "t_mm")
    return _sorted_pair(_num(e, "h_mm"), _num(e, "b_mm"), 0 if p == "d1" else 1)


def _angle_values(catalog, e, p):
    if p == "t":
        return _num(e, "t_mm")
    return _sorted_pair(_num(e, "b1_mm", "h_mm"), _num(e, "b2_mm", "b_mm"), 0 if p == "leg1" else 1)


def _round_values(catalog, e, p):
    return _num(e, "OD_mm", "outsideDiameter_mm") if p == "outsideDiameter" else _num(e, "t_mm", "wallThickness_mm")


def _fitting_od(catalog, e):
    od = _num(e, "outsideDiameter_mm")
    return od if od is not None else _class_pipe_od(catalog, _dn(e))


def _elbow_values(catalog, e, p):
    return _fitting_od(catalog, e) if p == "outsideDiameter" else _num(e, "centreToFace_mm")


def _tee_values(catalog, e, p):
    if p == "outsideDiameter":
        return _fitting_od(catalog, e)
    if p == "branchOutsideDiameter":
        return _class_pipe_od(catalog, _num(e, "branchDiameter_mm"))
    if p == "centreToEndRun":
        return _num(e, "centreToEnd_run_mm", "centreToEnd_mm")
    return _num(e, "centreToEnd_branch_mm", "centreToEnd_mm")


def _reducer_values(catalog, e, p):
    if p == "outsideDiameter":
        return _fitting_od(catalog, e)
    if p == "reducedOutsideDiameter":
        od = _num(e, "reducedOD_mm")
        return od if od is not None else _class_pipe_od(catalog, _num(e, "reducedDiameter_mm", "reducingDiameter_mm"))
    return _num(e, "length_mm")


def _flange_values(catalog, e, p):
    return {
        "flangeOD": lambda: _num(e, "flangeOD_mm", "outsideDiameter_mm"),
        "flangeThickness": lambda: _num(e, "flangeThickness_mm"),
        "boltCircle": lambda: _num(e, "boltCircle_mm", "boltCircleDiameter_mm"),
        "boltHoleDiameter": lambda: _num(e, "boltHoleDiameter_mm"),
    }[p]()


def _pipe_filter(catalog, e, family):
    return catalog.get("catalogType") == "PIPING_CLASS" and e.get("componentClass") == "PIPE"


KINDS = {
    "pipe": {"params": ["outsideDiameter", "wallThickness"], "required": ["outsideDiameter"], "tol": PIPE_TOL,
             "filter": _pipe_filter, "values": lambda c, e, p: _num(e, p + "_mm"), "algorithms": {V1, V2}},
    "i-section": {"params": ["h", "b", "tw", "tf"], "required": ["h", "b"], "tol": SECTION_TOL,
                  "filter": _profile_filter({"HEA", "HEB", "HEM", "IPE", "W"}), "values": _section_values,
                  "algorithms": {V1, V2}},
    "channel": {"params": ["h", "b", "tw", "tf"], "required": ["h", "b"], "tol": SECTION_TOL,
                "filter": _profile_filter({"UPE", "UPN", "C", "MC"}), "values": _section_values, "algorithms": {V2}},
    "rect-hollow": {"params": ["d1", "d2", "t"], "required": ["d1", "d2"],
                    "tol": {"d1": LEN, "d2": LEN, "t": (0.5, 0.15)}, "ordered": ("d1", "d2"),
                    "filter": _profile_filter({"RHS", "SHS", "HSS_RECT"}), "values": _rect_values, "algorithms": {V2}},
    "round-hollow": {"params": ["outsideDiameter", "wallThickness"], "required": ["outsideDiameter"], "tol": PIPE_TOL,
                     "filter": _profile_filter({"CHS", "HSS_ROUND"}), "values": _round_values, "algorithms": {V2}},
    "angle": {"params": ["leg1", "leg2", "t"], "required": ["leg1", "leg2"],
              "tol": {"leg1": LEN, "leg2": LEN, "t": (1.0, 0.15)}, "ordered": ("leg1", "leg2"),
              "filter": _profile_filter({"EQUAL_ANGLE", "UNEQUAL_ANGLE", "L"}), "values": _angle_values,
              "algorithms": {V2}},
    "elbow": {"params": ["outsideDiameter", "centreToFace"], "required": ["outsideDiameter"],
              "tol": {"outsideDiameter": OD, "centreToFace": LEN}, "selectors": {"angle"},
              "filter": _class_filter({"ELBOW"}), "values": _elbow_values, "algorithms": {V2}},
    "tee": {"params": ["outsideDiameter", "branchOutsideDiameter", "centreToEndRun", "centreToEndBranch"],
            "required": ["outsideDiameter", "branchOutsideDiameter"],
            "tol": {"outsideDiameter": OD, "branchOutsideDiameter": OD, "centreToEndRun": LEN, "centreToEndBranch": LEN},
            "filter": _class_filter({"TEE"}), "values": _tee_values, "algorithms": {V2}},
    "reducer": {"params": ["outsideDiameter", "reducedOutsideDiameter", "length"],
                "required": ["outsideDiameter", "reducedOutsideDiameter"],
                "tol": {"outsideDiameter": OD, "reducedOutsideDiameter": OD, "length": LEN},
                "filter": _class_filter({"REDUCER", "REDUCER_CONCENTRIC", "REDUCER_ECCENTRIC"}),
                "values": _reducer_values, "algorithms": {V2}},
    "flange": {"params": ["flangeOD", "flangeThickness", "boltCircle", "boltHoleDiameter"], "required": ["flangeOD"],
               "tol": {"flangeOD": (2.0, 0.01), "flangeThickness": (1.0, 0.1), "boltCircle": (1.5, 0.01),
                       "boltHoleDiameter": (1.0, 0.05)},
               "selectors": {"boltCount"}, "filter": _flange_filter, "values": _flange_values, "algorithms": {V2}},
}


class SnapError(ValueError):
    pass


def round6(x: float) -> float:
    """Round to 6 decimals, half away from zero (identical to Rust `(x * 1e6).round() / 1e6`)."""
    y = x * 1e6
    return math.copysign(math.floor(abs(y) + 0.5), y) / 1e6


def _selector_ok(entry: dict, name: str, wanted: float) -> bool:
    if name == "angle":
        c = _num(entry, "angle_deg")
        return c is not None and abs(c - wanted) <= ANGLE_SELECTOR_TOL_DEG
    c = _num(entry, "boltHoles", "numberOfBolts")  # boltCount
    return c is not None and c == wanted


def _fmt(x) -> str:
    return "None" if x is None else f"{x:g}"


def designation(catalog: dict, entry: dict, kind: str, algorithm: str = V1) -> str:
    rating = str(entry.get("rating") or catalog.get("rating") or "")
    if kind == "pipe":
        dn = entry.get("nominalDiameter_mm", 0) if algorithm == V1 else _dn(entry)
        return f"DN {_fmt(dn)} {catalog.get('rating', '')}".strip()
    if kind in ("i-section", "channel"):
        if "designation" in entry:
            return entry["designation"]
        cid, fam = entry["catalogId"], entry["family"]
        local = cid.split(":", 1)[-1]
        return f"{fam} {local[len(fam):] if local.startswith(fam) else local}"
    if kind in ("rect-hollow", "round-hollow", "angle"):
        return entry.get("designation") or entry["catalogId"].split(":", 1)[-1]
    dn = _fmt(_dn(entry))
    if kind == "elbow":
        return f"DN {dn} {_fmt(_num(entry, 'angle_deg'))}° {rating}".strip()
    if kind == "tee":
        return f"DN {dn}x{_fmt(_num(entry, 'branchDiameter_mm'))} {rating}".strip()
    if kind == "reducer":
        return f"DN {dn}x{_fmt(_num(entry, 'reducedDiameter_mm', 'reducingDiameter_mm'))} {rating}".strip()
    pc = entry.get("pressureClass")  # flange
    if pc is not None:
        rating = f"Class {_fmt(pc)}" if isinstance(pc, (int, float)) else str(pc)
    return f"DN {dn} {rating}".strip()


def snap(catalog: dict, kind: str, measured: dict, family: str | None = None, tolerances: dict | None = None,
         select: dict | None = None, algorithm: str = V1) -> dict:
    if algorithm not in (V1, V2):
        raise SnapError(f"unsupported algorithm {algorithm!r}")
    if kind not in KINDS or algorithm not in KINDS[kind]["algorithms"]:
        raise SnapError(f"unsupported kind {kind!r} for {algorithm}")
    tolerances = tolerances or {}
    select = select or {}
    spec = KINDS[kind]
    for p in measured:
        if p not in spec["params"]:
            raise SnapError(f"unknown parameter {p!r} for kind {kind!r}")
    for p in spec["required"]:
        if not isinstance(measured.get(p), (int, float)):
            raise SnapError(f"required parameter {p!r} missing")
    for s in select:
        if s not in spec.get("selectors", set()):
            raise SnapError(f"unknown selector {s!r} for kind {kind!r}")
    if "ordered" in spec:
        a, b = spec["ordered"]
        if measured[a] < measured[b]:
            raise SnapError(f"parameter order: {a} must be >= {b}")
    if not catalog.get("catalogId") or not catalog.get("version"):
        raise SnapError("catalog has no catalogId/version")
    used = [p for p in spec["params"] if isinstance(measured.get(p), (int, float))]
    primary = spec["params"][0]

    def tol(p, c):
        if p in tolerances:
            return float(tolerances[p])
        absolute, relative = spec["tol"][p]
        return max(absolute, relative * abs(c))

    ranked, nearest = [], None
    for entry in catalog.get("entries", []):
        if not spec["filter"](catalog, entry, family):
            continue
        if not all(_selector_ok(entry, s, float(v)) for s, v in select.items()):
            continue
        values = {p: spec["values"](catalog, entry, p) for p in spec["params"]}
        if any(values[p] is None for p in spec["required"]) or any(values[p] is None for p in used):
            continue  # entry lacks a required or measured parameter
        norm = [(measured[p] - values[p]) / tol(p, values[p]) for p in used]
        score = round6(math.sqrt(sum(n * n for n in norm) / len(norm)))
        key = (score, abs(measured[primary] - values[primary]), entry["catalogId"])
        if nearest is None or key < nearest[0]:
            nearest = (key, entry)
        if all(abs(n) <= 1.0 for n in norm):
            ranked.append((key, entry, values))
    ranked.sort(key=lambda r: r[0])

    if not ranked:
        return {"snapped": False,
                "nearest": {"entryId": nearest[1]["catalogId"], "score": nearest[0][0]} if nearest else None}
    (score, _, _), best, values = ranked[0]
    result = {
        "catalogRef": {"catalogId": catalog["catalogId"], "catalogVersion": catalog["version"],
                       "entryId": best["catalogId"]},
        "snappedSize": designation(catalog, best, kind, algorithm),
        "residuals": [{"parameter": p, "measured_mm": measured[p], "catalog_mm": values[p],
                       "residual_mm": round6(measured[p] - values[p])} for p in used],
        "algorithm": algorithm,
        "score": score,
    }
    if len(ranked) > 1:
        runner_score = ranked[1][0][0]
        result["runnerUp"] = {"entryId": ranked[1][1]["catalogId"], "score": runner_score}
        result["ambiguous"] = round6(runner_score - score) < AMBIGUITY_MARGIN
    else:
        result["ambiguous"] = False
    if tolerances:
        tolerances_used = {p: float(tolerances[p]) for p in used if p in tolerances}
        if tolerances_used:
            result["tolerances"] = tolerances_used
    return {"snapped": True, "catalogSnap": result}


def run_vector(v: dict, default_algorithm: str) -> dict:
    catalog = json.loads((ROOT / v["catalog"]).read_text(encoding="utf-8"))
    try:
        return snap(catalog, v["kind"], v["measured"], v.get("family"), v.get("tolerances"), v.get("select"),
                    v.get("algorithm", default_algorithm))
    except SnapError as e:
        return {"error": str(e)}


def matches(expected: dict, got: dict) -> bool:
    """Structural comparison; numbers within 1e-6."""
    if isinstance(expected, dict):
        return isinstance(got, dict) and expected.keys() == got.keys() and all(
            matches(expected[k], got[k]) for k in expected)
    if isinstance(expected, list):
        return isinstance(got, list) and len(expected) == len(got) and all(map(matches, expected, got))
    if isinstance(expected, (int, float)) and not isinstance(expected, bool):
        return isinstance(got, (int, float)) and abs(expected - got) <= 1e-6
    return expected == got


VECTOR_FILES = [("vectors.json", V1), ("vectors-v2.json", V2)]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--print", action="store_true")
    args = ap.parse_args()
    bad = total = 0
    for name, default_algorithm in VECTOR_FILES:
        vectors = json.loads((ROOT / "tests" / "snapping" / name).read_text(encoding="utf-8"))["vectors"]
        for v in vectors:
            got = run_vector(v, default_algorithm)
            total += 1
            if args.print:
                print(v["id"], json.dumps(got, ensure_ascii=False))
                continue
            if "expected_error" in v:
                ok = "error" in got and v["expected_error"] in got["error"]
            else:
                ok = matches(v["expected"], got)
            bad += not ok
            print(("✓" if ok else "✗"), v["id"], v["name"], "" if ok else f"\n    got {json.dumps(got, ensure_ascii=False)}")
    if not args.print:
        print(f"{total - bad}/{total} vectors passed")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
