#!/usr/bin/env python3
"""check-geometry.py — geometric consistency of piping components in PMEF NDJSON files.

Checks (all lengths in mm, tolerance 0.5 mm / 0.5°):
  1. Port directions are unit vectors.
  2. Connected ports (connectedTo, both sides present) are at the same coordinate.
  3. In-line two-port components (pipe, flange, gasket, valve, reducer): port directions point
     away from the component along its axis (spec 02 §2.6); pipeLength equals the port distance.
  4. Elbow: the two port axes intersect (bend centre point), the centre point is equidistant
     from both ports, and the angle between the axes matches `angle`.

Usage:
    python scripts/check-geometry.py                 # all examples/*.ndjson
    python scripts/check-geometry.py FILE [FILE ...]
"""
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOL = 0.5
STRAIGHT = {"pmef:Pipe", "pmef:Flange", "pmef:Gasket", "pmef:Valve", "pmef:Reducer"}  # in-line, two ports


def sub(a, b):
    return [a[i] - b[i] for i in range(3)]


def dot(a, b):
    return sum(a[i] * b[i] for i in range(3))


def norm(a):
    return math.sqrt(dot(a, a))


def axes_intersection(p1, d1, p2, d2):
    """Closest points of p1 + s·d1 and p2 + u·d2; returns (point, gap) or None if parallel."""
    w = sub(p1, p2)
    a, b, c, d, e = dot(d1, d1), dot(d1, d2), dot(d2, d2), dot(d1, w), dot(d2, w)
    den = a * c - b * b
    if abs(den) < 1e-12:
        return None
    s, u = (b * e - c * d) / den, (a * e - b * d) / den
    q1 = [p1[i] + s * d1[i] for i in range(3)]
    q2 = [p2[i] + u * d2[i] for i in range(3)]
    return q1, norm(sub(q1, q2))


def check(objs):
    by_id = {o.get("@id"): o for o in objs}
    issues = []
    for o in objs:
        oid, ports = o.get("@id"), o.get("ports") or []
        for p in ports:
            d = p.get("direction")
            if d is not None and abs(norm(d) - 1) > 1e-3:
                issues.append(f"{oid}.{p.get('portId')}: direction is not a unit vector")
            t = p.get("connectedTo")
            if t in by_id and "coordinate" in p:
                other = [q for q in by_id[t].get("ports") or [] if q.get("connectedTo") == oid and "coordinate" in q]
                if other and min(norm(sub(p["coordinate"], q["coordinate"])) for q in other) > TOL:
                    issues.append(f"{oid}.{p.get('portId')}: not at the same point as connected {t}")
        pts = [p for p in ports if "coordinate" in p]
        if o.get("@type") in STRAIGHT and len(pts) == 2:
            dist = norm(sub(pts[1]["coordinate"], pts[0]["coordinate"]))
            if o.get("@type") == "pmef:Pipe" and "pipeLength" in o and abs(dist - o["pipeLength"]) > TOL:
                issues.append(f"{oid}: pipeLength {o['pipeLength']} but ports are {dist:.1f} mm apart")
            axis = sub(pts[1]["coordinate"], pts[0]["coordinate"])
            d1, d2 = pts[0].get("direction"), pts[1].get("direction")
            if d1 and d2 and dist > TOL and not (dot(axis, d1) < 0 < dot(axis, d2)):
                issues.append(f"{oid}: port directions must point away from the component (spec 02 §2.6)")
        if o.get("@type") == "pmef:Elbow" and len(pts) == 2 and all("direction" in p for p in pts):
            p1, p2 = pts[0]["coordinate"], pts[1]["coordinate"]
            d1, d2 = pts[0]["direction"], pts[1]["direction"]
            hit = axes_intersection(p1, d1, p2, d2)
            if hit is None or hit[1] > TOL:
                issues.append(f"{oid}: port axes do not intersect (no bend centre point)")
                continue
            c = hit[0]
            r1, r2 = norm(sub(c, p1)), norm(sub(c, p2))
            if r1 < TOL or abs(r1 - r2) > TOL:
                issues.append(f"{oid}: centre point not equidistant from ports ({r1:.1f} / {r2:.1f} mm)")
            # bend angle = 180° − angle between the outward port directions
            cosang = max(-1.0, min(1.0, dot(d1, d2) / (norm(d1) * norm(d2))))
            bend = 180.0 - math.degrees(math.acos(cosang))
            if "angle" in o and abs(bend - o["angle"]) > 0.5:
                issues.append(f"{oid}: angle {o['angle']}° but port directions give {bend:.1f}°")
    return issues


def main():
    files = [Path(a) for a in sys.argv[1:]] or sorted((ROOT / "examples").glob("*.ndjson"))
    total = 0
    for f in files:
        objs = [json.loads(line) for line in f.read_text(encoding="utf-8").splitlines() if line.startswith("{")]
        issues = check(objs)
        total += len(issues)
        print(("✗" if issues else "✓"), f.name, f"({len(issues)} issue(s))" if issues else "")
        for i in issues:
            print("    ", i)
    sys.exit(1 if total else 0)


if __name__ == "__main__":
    main()
