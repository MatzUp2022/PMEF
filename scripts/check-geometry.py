#!/usr/bin/env python3
"""check-geometry.py — geometric consistency of piping components in PMEF NDJSON files.

Checks (all lengths in mm, tolerance 0.5 mm / 0.5°):

Piping
  1. Port directions are unit vectors.
  2. Connected ports (connectedTo, both sides present) are at the same coordinate.
  3. In-line two-port components (pipe, flange, gasket, valve, reducer): port directions point
     away from the component along its axis (spec 02 §2.6); pipeLength equals the port distance.
  4. Elbow: the two port axes intersect (bend centre point), the centre point is equidistant
     from both ports, and the angle between the axes matches `angle`.

Equipment (spec 02 §5.2, spec 04 primitives)
  5. Nozzle directions are unit vectors.
  6. Nozzle vs. the owner's ParametricGeometry: the nozzle face is not inside the body, the
     direction points away from it (outward, spec 02 §5.2), and the face is at most
     MAX_STANDOFF from the body surface. Without parametric geometry, the face must not lie
     inside the `geometry.boundingBox` (mesh_ref equipment).
  7. Primitive assembly: every DISH sits on an end face of a CYLINDER of the same COMPOSITE
     (DISH `center` = knuckle circle = cylinder face, `axis` pointing away from the cylinder,
     shellRadius ≈ cylinder radius).
  8. A nozzle with `connectedLineId` (line with components in the same file) has a free end
     (port without `connectedTo`, or pointing back at the equipment) of that line at its face
     coordinate, with anti-parallel direction.

Structural steel (spec 02 §7)
  9. Members have non-zero length; `length`, if given, equals the start/end distance.
 10. `profileId` resolves in catalogs/profiles-*.json (exactness comes from catalog snapping).
 11. SteelNode coordinates coincide with an end of each listed member; members of a
     SteelConnection come within (h1 + h2) / 2 of each other (profile depths from the catalog).

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
MAX_STANDOFF = 1000.0  # nozzle face to body surface [mm]
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
    issues += check_equipment(objs, by_id)
    issues += check_steel(objs, by_id)
    return issues


# ── equipment ─────────────────────────────────────────────────────────────────

def unit(a):
    n = norm(a)
    return [x / n for x in a]


def add(a, b, s=1.0):
    return [a[i] + s * b[i] for i in range(3)]


def sdist(prim, p):
    """Approximate signed distance of p to a solid primitive (< 0 inside, sign exact).
    Returns None for primitive types that are not evaluated."""
    t = prim.get("type")
    if t == "COMPOSITE":
        kids = [c["primitive"] for c in prim.get("children", [])]
        ds = [sdist(closed(k, kids), p) for k in kids]
        ds = [d for d in ds if d is not None]
        return min(ds) if ds else None
    if t == "SPHERE":
        return norm(sub(p, prim["center"])) - prim["radius"]
    if t == "BOX":
        if any(prim.get(k) for k in ("rotationX", "rotationY", "rotationZ")):
            return None
        half = [prim["xLen"] / 2, prim["yLen"] / 2, prim["zLen"] / 2]
        return max(abs(p[i] - prim["center"][i]) - half[i] for i in range(3))
    if t in ("CYLINDER", "CONE", "SNOUT", "DISH"):
        a = unit(prim["axis"])
        v = sub(p, prim["center"])
        h = dot(v, a)
        d = norm(sub(v, [a[i] * h for i in range(3)]))
        if t == "DISH":  # half-ellipsoid beyond the knuckle circle
            r = prim["shellRadius"]
            depth = prim.get("depth") or {"HEMISPHERICAL": r, "FLAT": 0.0}.get(prim.get("dishType"), r / 2)
            if depth <= 0:
                return max(d - r, -h, h)
            k = math.sqrt((d / r) ** 2 + (h / depth) ** 2)
            return max((k - 1) * min(r, depth), -h)
        length = prim["length"]
        r = prim["radius"] if t == "CYLINDER" else prim["r1"] + (prim["r2"] - prim["r1"]) * min(max(h / length, 0), 1)
        ri = prim.get("innerRadius") if t == "CYLINDER" else None
        return max(d - r, -h, h - length, (ri - d) if ri else -math.inf)  # hollow: annulus
    return None


def closed(k, kids):
    """A hollow CYLINDER capped at both ends by other primitives (heads, channels) encloses the
    process space and is treated as solid; an open one (e.g. a skirt) stays an annulus."""
    if k.get("type") != "CYLINDER" or not k.get("innerRadius"):
        return k
    a = unit(k["axis"])
    others = [o for o in kids if o is not k]

    def capped(face, out):  # another primitive within 0.5·r beyond the end face (coarse LODs)
        gaps = (1.0, 0.1 * k["radius"], 0.25 * k["radius"], 0.5 * k["radius"])
        return any(any((sdist(o, add(face, out, g)) or 1.0) <= 0 for o in others) for g in gaps)

    if capped(k["center"], [-x for x in a]) and capped(add(k["center"], a, k["length"]), a):
        return {**k, "innerRadius": None}
    return k


def check_equipment(objs, by_id):
    issues = []
    geom = {o["ownerRef"]: o["primitive"] for o in objs if o.get("@type") == "pmef:ParametricGeometry"}
    line_of = {}  # component @id -> line @id
    for o in objs:
        parent = by_id.get(o.get("isPartOf"))
        if parent and parent.get("@type") == "pmef:PipingSegment":
            line_of[o["@id"]] = parent.get("isPartOf")
        elif parent and parent.get("@type") == "pmef:PipingNetworkSystem":
            line_of[o["@id"]] = parent["@id"]
    for o in objs:
        oid = o.get("@id")
        prim = geom.get(oid)
        if prim and prim.get("type") == "COMPOSITE":
            issues += check_assembly(oid, prim)
        for n in o.get("nozzles") or []:
            nid = f"{oid}#{n.get('nozzleId')}"
            c, d = n.get("coordinate"), n.get("direction")
            if d is not None and abs(norm(d) - 1) > 1e-3:
                issues.append(f"{nid}: nozzle direction is not a unit vector")
                continue
            bbox = (o.get("geometry") or {}).get("boundingBox")
            if not prim and bbox and c:  # mesh_ref equipment: the face must not lie inside the box
                depth = min(min(c[i] - bbox[lo], bbox[hi] - c[i]) for i, (lo, hi)
                            in enumerate((("xMin", "xMax"), ("yMin", "yMax"), ("zMin", "zMax"))))
                if depth > TOL:
                    issues.append(f"{nid}: nozzle face is {depth:.0f} mm inside the equipment bounding box")
            if prim and c:
                s0 = sdist(prim, c)
                if s0 is not None:
                    if s0 < -TOL:
                        issues.append(f"{nid}: nozzle face is {-s0:.0f} mm inside the equipment body")
                    elif s0 > MAX_STANDOFF:
                        issues.append(f"{nid}: nozzle face is {s0:.0f} mm away from the equipment body")
                    if d and s0 >= -TOL and sdist(prim, add(c, d, 25.0)) < s0 - TOL:
                        issues.append(f"{nid}: nozzle direction points into the equipment (must point outward)")
            line = n.get("connectedLineId")
            if c and line in by_id:
                ports = [(q, p) for q, ln in line_of.items() if ln == line
                         for p in by_id[q].get("ports") or [] if "coordinate" in p]
                # the port must be a free line end (or point back at this equipment)
                hit = [(q, p) for q, p in ports if norm(sub(p["coordinate"], c)) <= TOL
                       and str(p.get("connectedTo") or oid).split("#")[0] == oid]
                if not ports:
                    pass  # line modelled without components in this file
                elif not hit:
                    issues.append(f"{nid}: no free end of {line} at the nozzle face")
                elif d and all(p.get("direction") and dot(p["direction"], d) > -0.99 for _, p in hit):
                    issues.append(f"{nid}: connected port direction is not opposite to the nozzle direction")
    return issues


def check_assembly(oid, prim):
    issues = []
    kids = [c["primitive"] for c in prim.get("children", [])]
    cyls = [k for k in kids if k.get("type") == "CYLINDER"]
    for k in kids:
        if k.get("type") != "DISH":
            continue
        ok = False
        for cy in cyls:
            a = unit(cy["axis"])
            for face, out in ((cy["center"], [-x for x in a]), (add(cy["center"], a, cy["length"]), a)):
                if norm(sub(k["center"], face)) <= TOL and dot(unit(k["axis"]), out) > 0.999:
                    ok = abs(k["shellRadius"] - cy["radius"]) <= max(0.05 * cy["radius"], TOL)
        if not ok:
            issues.append(f"{oid}: DISH at {k['center']} does not sit on a cylinder end face "
                          "(spec 04: DISH center = knuckle circle, axis outward)")
    return issues


# ── structural steel ──────────────────────────────────────────────────────────

def load_profiles():
    prof = {}
    for f in sorted((ROOT / "catalogs").glob("profiles-*.json")):
        for e in json.loads(f.read_text(encoding="utf-8")).get("entries", []):
            prof[e.get("catalogId")] = e
    return prof


PROFILES = None


def seg_dist(p1, q1, p2, q2):
    """Minimum distance between segments p1-q1 and p2-q2 (sampled, sufficient for checks)."""
    best = float("inf")
    for i in range(51):
        a = add(p1, sub(q1, p1), i / 50)
        d2, w = sub(q2, p2), sub(a, p2)
        t = 0.0 if dot(d2, d2) == 0 else max(0.0, min(1.0, dot(w, d2) / dot(d2, d2)))
        best = min(best, norm(sub(a, add(p2, d2, t))))
    return best


def check_steel(objs, by_id):
    global PROFILES
    issues = []
    for o in objs:
        if o.get("@type") != "pmef:SteelMember":
            continue
        oid, s, e = o["@id"], o.get("startPoint"), o.get("endPoint")
        if s and e:
            L = norm(sub(e, s))
            if L <= TOL:
                issues.append(f"{oid}: member has zero length")
            elif "length" in o and abs(o["length"] - L) > TOL:
                issues.append(f"{oid}: length {o['length']} but start/end are {L:.1f} mm apart")
        pid = o.get("profileId")
        if pid:
            if PROFILES is None:
                PROFILES = load_profiles()
            if pid not in PROFILES:
                issues.append(f"{oid}: profileId {pid} not found in catalogs/profiles-*.json")
    members = {o["@id"]: o for o in objs if o.get("@type") == "pmef:SteelMember"}
    for o in objs:
        t, ids = o.get("@type"), [m for m in o.get("memberIds") or [] if m in members]
        if t == "pmef:SteelNode" and "coordinate" in o:
            for m in ids:
                ends = [members[m].get("startPoint"), members[m].get("endPoint")]
                if not any(x and norm(sub(x, o["coordinate"])) <= TOL for x in ends):
                    issues.append(f"{o['@id']}: node is not at an end of member {m}")
        if t == "pmef:SteelConnection":
            if PROFILES is None:
                PROFILES = load_profiles()
            for i, a in enumerate(ids):
                for b in ids[i + 1:]:
                    ma, mb = members[a], members[b]
                    ha = PROFILES.get(ma.get("profileId"), {}).get("h_mm")
                    hb = PROFILES.get(mb.get("profileId"), {}).get("h_mm")
                    if None in (ha, hb) or not all(m.get(k) for m in (ma, mb) for k in ("startPoint", "endPoint")):
                        continue
                    gap = seg_dist(ma["startPoint"], ma["endPoint"], mb["startPoint"], mb["endPoint"])
                    if gap > (ha + hb) / 2 + TOL:
                        issues.append(f"{o['@id']}: members {a} and {b} are {gap:.0f} mm apart "
                                      f"(> (h1 + h2) / 2 = {(ha + hb) / 2:.0f} mm)")
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
