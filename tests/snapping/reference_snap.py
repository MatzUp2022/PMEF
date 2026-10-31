#!/usr/bin/env python3
"""Reference implementation of the PMEF catalog-snapping algorithm `pmef-snap/1` (RFC-0002).

Deliberately small and dependency-free so it can be read next to the RFC. The Rust
implementation (`pmef_core::snapping`) must produce identical results on
tests/snapping/vectors.json.

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
ALGORITHM = "pmef-snap/1"
AMBIGUITY_MARGIN = 0.1  # runner-up closer than this ⇒ ambiguous (needs human review)

# kind -> ordered canonical parameters, required parameters, primary parameter
KINDS = {
    "pipe": {"params": ["outsideDiameter", "wallThickness"], "required": ["outsideDiameter"]},
    "i-section": {"params": ["h", "b", "tw", "tf"], "required": ["h", "b"]},
}
I_SECTION_FAMILIES = {"HEA", "HEB", "HEM", "IPE", "W"}
# canonical parameter -> catalog field names (first present wins)
FIELDS = {
    "outsideDiameter": ["outsideDiameter_mm"],
    "wallThickness": ["wallThickness_mm"],
    "h": ["h_mm", "d_mm"],
    "b": ["b_mm", "bf_mm"],
    "tw": ["tw_mm"],
    "tf": ["tf_mm"],
}
# default tolerance band: max(absolute mm, relative × catalog value)
DEFAULT_TOL = {
    "outsideDiameter": (1.0, 0.01),
    "wallThickness": (0.5, 0.15),
    "h": (2.0, 0.02),
    "b": (2.0, 0.02),
    "tw": (1.0, 0.15),
    "tf": (1.0, 0.15),
}


class SnapError(ValueError):
    pass


def round6(x: float) -> float:
    """Round to 6 decimals, half away from zero (identical to Rust `(x * 1e6).round() / 1e6`)."""
    y = x * 1e6
    return math.copysign(math.floor(abs(y) + 0.5), y) / 1e6


def candidates(catalog: dict, kind: str, family: str | None):
    ctype = catalog.get("catalogType")
    for e in catalog.get("entries", []):
        if kind == "pipe":
            if ctype != "PIPING_CLASS" or e.get("componentClass") != "PIPE":
                continue
        else:
            if ctype != "STEEL_PROFILES" or e.get("family") not in I_SECTION_FAMILIES:
                continue
            if family is not None and e.get("family") != family:
                continue
        values = {}
        for p in KINDS[kind]["params"]:
            for f in FIELDS[p]:
                if isinstance(e.get(f), (int, float)):
                    values[p] = float(e[f])
                    break
        if all(p in values for p in KINDS[kind]["required"]):
            yield e, values


def tolerance(param: str, catalog_value: float, overrides: dict) -> float:
    if param in overrides:
        return float(overrides[param])
    absolute, relative = DEFAULT_TOL[param]
    return max(absolute, relative * abs(catalog_value))


def designation(catalog: dict, entry: dict, kind: str) -> str:
    if kind == "pipe":
        return f"DN {entry['nominalDiameter_mm']:g} {catalog.get('rating', '')}".strip()
    if "designation" in entry:
        return entry["designation"]
    cid, fam = entry["catalogId"], entry["family"]
    size = cid.split(":", 1)[-1][len(fam):]
    return f"{fam} {size}"


def snap(catalog: dict, kind: str, measured: dict, family: str | None = None, tolerances: dict | None = None) -> dict:
    if kind not in KINDS:
        raise SnapError(f"unsupported kind {kind!r}")
    tolerances = tolerances or {}
    spec = KINDS[kind]
    for p in measured:
        if p not in spec["params"]:
            raise SnapError(f"unknown parameter {p!r} for kind {kind!r}")
    for p in spec["required"]:
        if not isinstance(measured.get(p), (int, float)):
            raise SnapError(f"required parameter {p!r} missing")
    used = [p for p in spec["params"] if isinstance(measured.get(p), (int, float))]
    primary = spec["params"][0]

    ranked, nearest = [], None
    for entry, values in candidates(catalog, kind, family):
        params = [p for p in used if p in values]
        if params != used:
            continue  # entry lacks a measured parameter
        norm = [(measured[p] - values[p]) / tolerance(p, values[p], tolerances) for p in used]
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
        "snappedSize": designation(catalog, best, kind),
        "residuals": [{"parameter": p, "measured_mm": measured[p], "catalog_mm": values[p],
                       "residual_mm": round6(measured[p] - values[p])} for p in used],
        "algorithm": ALGORITHM,
        "score": score,
    }
    if len(ranked) > 1:
        runner_score = ranked[1][0][0]
        result["runnerUp"] = {"entryId": ranked[1][1]["catalogId"], "score": runner_score}
        result["ambiguous"] = round6(runner_score - score) < AMBIGUITY_MARGIN
    else:
        result["ambiguous"] = False
    if tolerances:
        result["tolerances"] = {p: float(tolerances[p]) for p in used if p in tolerances}
    return {"snapped": True, "catalogSnap": result}


def run_vector(v: dict) -> dict:
    catalog = json.loads((ROOT / v["catalog"]).read_text(encoding="utf-8"))
    try:
        return snap(catalog, v["kind"], v["measured"], v.get("family"), v.get("tolerances"))
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--print", action="store_true")
    args = ap.parse_args()
    vectors = json.loads((ROOT / "tests" / "snapping" / "vectors.json").read_text(encoding="utf-8"))["vectors"]
    bad = 0
    for v in vectors:
        got = run_vector(v)
        if args.print:
            print(v["id"], json.dumps(got))
            continue
        if "expected_error" in v:
            ok = "error" in got and v["expected_error"] in got["error"]
        else:
            ok = matches(v["expected"], got)
        bad += not ok
        print(("✓" if ok else "✗"), v["id"], v["name"], "" if ok else f"\n    got {json.dumps(got)}")
    if not args.print:
        print(f"{len(vectors) - bad}/{len(vectors)} vectors passed")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
