#!/usr/bin/env python3
"""check-catalogs.py — internal consistency of the open catalogs in catalogs/.

Catalog snapping (RFC-0002) makes catalogs the source of exactness, so every redundant value
must agree with the primary data it is derived from. Checks (tolerances per check):

Steel profiles (profiles-*.json)
  - section area from the dimensions (I/H, channel, T, angle, CHS, RHS/SHS, flat, HSS);
    I/H sections also Iy, Iz, Wel (scripts/i-section-properties.py); EN 10055 T sections
    also Iy, Iz from the exact tapered polygon (±4 %)
  - mass per length = area × 7850 kg/m³
  - kg/m ↔ lb/ft (AISC), weight in the AISC designation (W14x90 → 90 lb/ft)
  - designation / catalogId ↔ dimensions (RHS100x60x4, L50x5, HSS12x8x0.375, …)
  - section properties increase along a series (A, mass, Iy)
Pipe dimensions (pipe-dimensions*.json)
  - bore = OD − 2t, mass from OD and t, OD and DN strictly increasing, wall increasing with
    the schedule (XXS excluded: thinner than SCH160 from NPS 8 per ASME B36.10M); EN: series-1
    OD per DN, walls from the EN 10220 preferred series, DIN 2448/2458 normal walls present
Flange dimensions (flange-dimensions*.json)
  - bolt circle < flange OD; bolt hole > bolt; raised face inside the bolt holes; OD and bolt
    circle increasing with DN per class
Piping classes (piping-class-*.json)
  - pipes and flanges agree with the dimension catalogs (OD, wall, bore, mass; flange OD,
    bolt circle, number of bolts, thickness)

Usage:
    python scripts/check-catalogs.py            # all catalogs
    python scripts/check-catalogs.py --verbose  # also print the checked counts per catalog
    python scripts/check-catalogs.py --dir DIR  # another catalog directory (tests/catalogs/)
"""
import importlib.util
import json
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CAT = ROOT / "catalogs"  # --dir DIR checks another catalog directory (tests/catalogs/)
RHO = 0.785          # kg/m per cm² (7850 kg/m³)
LB_FT = 1.48816394   # kg/m per lb/ft
IN = 25.4
LB_FT_PER_IN2 = 3.4  # lb/ft per in² (490 lb/ft³)

_spec = importlib.util.spec_from_file_location("isp", ROOT / "scripts" / "i-section-properties.py")
isp = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(isp)

issues = []
checked = {}


def rel(a, b):
    return (a - b) / b


def bad(where, msg):
    issues.append(f"{where}: {msg}")


def close(where, name, value, ref, tol):
    """Relative check; returns True if within tolerance."""
    checked[where.split(":")[0]] = checked.get(where.split(":")[0], 0) + 1
    if ref and abs(rel(value, ref)) > tol:
        bad(where, f"{name} {value} vs {ref:.4g} derived ({100 * rel(value, ref):+.1f} %, tolerance ±{100 * tol:.0f} %)")
        return False
    return True


def load(name):
    f = CAT / name
    return json.loads(f.read_text(encoding="utf-8")) if f.exists() else {"entries": []}


# ── steel profiles ────────────────────────────────────────────────────────────

def channel_area(h, b, tw, tf, r):
    return 2 * b * tf + (h - 2 * tf) * tw + 2 * (1 - math.pi / 4) * r * r


def _round_corners(pts, radii, seg=16):
    """Replace each polygon vertex with radius r > 0 by a tangent arc (convex or concave)."""
    out = []
    for i, p in enumerate(pts):
        r = radii[i]
        if not r:
            out.append(p)
            continue
        a, c = pts[i - 1], pts[(i + 1) % len(pts)]
        u = (a[0] - p[0], a[1] - p[1])
        v = (c[0] - p[0], c[1] - p[1])
        lu, lv = math.hypot(*u), math.hypot(*v)
        u, v = (u[0] / lu, u[1] / lu), (v[0] / lv, v[1] / lv)
        ang = math.acos(max(-1.0, min(1.0, u[0] * v[0] + u[1] * v[1])))
        dist = r / math.tan(ang / 2)
        bis = (u[0] + v[0], u[1] + v[1])
        lb = math.hypot(*bis)
        ctr = (p[0] + bis[0] / lb * r / math.sin(ang / 2), p[1] + bis[1] / lb * r / math.sin(ang / 2))
        a1 = math.atan2(p[1] + u[1] * dist - ctr[1], p[0] + u[0] * dist - ctr[0])
        a2 = math.atan2(p[1] + v[1] * dist - ctr[1], p[0] + v[0] * dist - ctr[0])
        da = (a2 - a1 + math.pi) % (2 * math.pi) - math.pi
        out += [(ctr[0] + r * math.cos(a1 + da * k / seg), ctr[1] + r * math.sin(a1 + da * k / seg))
                for k in range(seg + 1)]
    return out


def tee_properties(h, b, s, t, r1, taper=0.02):
    """EN 10055 T section as an exact polygon: flange inner face and web faces with 2 % taper,
    flange thickness t at b/4, web thickness s at h/2, root radius r1, toe radii r2 = t/2
    (flange) and r3 = t/4 (web). Returns A [mm²], Iy (horizontal axis), Iz [mm⁴].
    Reproduces the EN 10055 / DIN 1024 tables (T30–T50) within 3 %."""
    def web(y):
        return s / 2 + taper * (y - h / 2)

    def underside(x):
        return h - t - taper * (b / 4 - x)

    y = h - t
    for _ in range(50):
        y = underside(web(y))
    half = [(0.0, 0.0), (web(0), 0.0), (web(y), y), (b / 2, underside(b / 2)), (b / 2, h), (0.0, h)]
    right = _round_corners(half, [0, t / 4, r1, t / 2, 0, 0])
    poly = right[:-1] + [(-x, yy) for x, yy in reversed(right[1:-1])]
    a = sx = ixx = iyy = 0.0
    for (x0, y0), (x1, y1) in zip(poly, poly[1:] + poly[:1]):
        c = x0 * y1 - x1 * y0
        a += c / 2
        sx += (y0 + y1) * c / 6
        ixx += (y0 * y0 + y0 * y1 + y1 * y1) * c / 12
        iyy += (x0 * x0 + x0 * x1 + x1 * x1) * c / 12
    a, yc = abs(a), abs(sx) / abs(a)
    return a, abs(ixx) - a * yc * yc, abs(iyy)


def angle_area(b1, b2, t, r1=0.0, r2=0.0):
    return t * (b1 + b2 - t) + (1 - math.pi / 4) * (r1 * r1 - 2 * r2 * r2)


def hollow_rect_area(h, b, t, ro, ri):
    return 2 * t * (b + h - 2 * t) - (4 - math.pi) * (ro * ro - ri * ri)


def rhs_en_areas(h, b, t):
    """EN 10210 hot-finished (ro = 1.5t, ri = t) and EN 10219 cold-formed corner radii."""
    ro_cold = (2.0 if t <= 6 else 2.5 if t <= 10 else 3.0) * t
    return [hollow_rect_area(h, b, t, 1.5 * t, t), hollow_rect_area(h, b, t, ro_cold, ro_cold - t)]


def num(s):
    return float(s)


def designation_dims(e):
    """Dimensions implied by the designation / catalogId, or {} if not parseable."""
    fam, cid = e.get("family"), e["catalogId"].split(":", 1)[-1]
    d = e.get("designation", cid)
    m = None
    if fam in ("RHS", "SHS"):
        m = re.fullmatch(r"(?:RHS|SHS)([\d.]+)x(?:([\d.]+)x)?([\d.]+)", cid)
        if m:
            a, b2, t = num(m[1]), num(m[2]) if m[2] else num(m[1]), num(m[3])
            return {"dims": sorted([a, b2]), "t": t}
    if fam == "CHS":
        m = re.fullmatch(r"CHS([\d.]+)x([\d.]+)", cid)
        return {"OD_mm": num(m[1]), "t_mm": num(m[2])} if m else {}
    if fam == "FLAT":
        m = re.fullmatch(r"FLAT([\d.]+)x([\d.]+)", cid)
        return {"b_mm": num(m[1]), "t_mm": num(m[2])} if m else {}
    if fam == "EQUAL_ANGLE":
        m = re.fullmatch(r"L([\d.]+)x([\d.]+)", cid)
        return {"b1_mm": num(m[1]), "b2_mm": num(m[1]), "t_mm": num(m[2])} if m else {}
    if fam == "UNEQUAL_ANGLE":
        m = re.fullmatch(r"L([\d.]+)x([\d.]+)x([\d.]+)", cid)
        return {"h_mm": num(m[1]), "b_mm": num(m[2]), "t_mm": num(m[3])} if m else {}
    if fam in ("IPE", "UPE", "T"):
        m = re.fullmatch(r"(?:IPE|UPE|T)([\d.]+)", cid)
        return {"h_mm": num(m[1])} if m else {}
    if fam == "L":
        m = re.fullmatch(r"L([\d.]+)x([\d.]+)x([\d.]+)", d)
        return {"b1_mm": num(m[1]) * IN, "b2_mm": num(m[2]) * IN, "t_mm": num(m[3]) * IN} if m else {}
    if fam == "HSS_RECT":
        m = re.fullmatch(r"HSS([\d.]+)x([\d.]+)x([\d.]+)", d)
        return {"dims": sorted([num(m[1]) * IN, num(m[2]) * IN]), "t": num(m[3]) * IN} if m else {}
    if fam == "HSS_ROUND":
        m = re.fullmatch(r"HSS([\d.]+)x([\d.]+)", d)
        return {"OD_mm": num(m[1]) * IN, "t_mm": num(m[2]) * IN} if m else {}
    return {}


def check_designation(e, where):
    dd = designation_dims(e)
    if "dims" in dd:
        have = sorted([e["h_mm"], e["b_mm"]])
        if any(abs(x - y) > 0.5 for x, y in zip(have, dd["dims"])) or abs(e["t_mm"] - dd["t"]) > 0.05:
            bad(where, f"dimensions {have} × {e['t_mm']} do not match the designation")
        return
    for k, v in dd.items():
        if k in e and abs(e[k] - v) > max(0.5, 0.005 * v):
            bad(where, f"{k} {e[k]} does not match the designation ({v:.4g})")


def check_profile(e, src):
    fam, cid = e.get("family"), e["catalogId"]
    w = f"{src}:{cid}"
    check_designation(e, w)
    area = None  # mm² (or in² for the inch catalog)
    if fam in ("HEA", "HEB", "HEM", "IPE"):
        p = isp.properties(e["h_mm"], e["b_mm"], e["tw_mm"], e["tf_mm"], e.get("r_mm", 0))
        area = p["A_cm2"] * 100
        for k in ("Iy_cm4", "Iz_cm4", "Wel_y_cm3", "Wel_z_cm3"):
            if k in e:
                close(w, k, e[k], p[k], 0.02)
    elif fam == "UPE":
        area = channel_area(e["h_mm"], e["b_mm"], e["tw_mm"], e["tf_mm"], e.get("r_mm", 0))
    elif fam == "T":
        area, iy, iz = tee_properties(e["h_mm"], e["b_mm"], e["tw_mm"], e["tf_mm"], e.get("r_mm", 0))
        for k, ref in (("Iy_cm4", iy / 1e4), ("Iz_cm4", iz / 1e4)):
            if k in e:
                close(w, k, e[k], ref, 0.04)
    elif fam == "EQUAL_ANGLE":
        r = e.get("r_mm", 0)
        area = angle_area(e["b1_mm"], e["b2_mm"], e["t_mm"], r, r / 2)
    elif fam == "UNEQUAL_ANGLE":
        area = angle_area(e["h_mm"], e["b_mm"], e["t_mm"], e.get("r1_mm", 0), e.get("r2_mm", 0))
    elif fam == "L":  # AISC: no radii in the catalog → lower bound, fillet adds ≈ 1 %
        area = angle_area(e["b1_mm"], e["b2_mm"], e["t_mm"]) * 1.01
    elif fam == "CHS":
        area = math.pi * (e["OD_mm"] - e["t_mm"]) * e["t_mm"]
    elif fam == "FLAT":
        area = e["b_mm"] * e["t_mm"]
    elif fam in ("RHS", "SHS"):
        areas = rhs_en_areas(e["h_mm"], e["b_mm"], e["t_mm"])
        best = min(areas, key=lambda a: abs(rel(e.get("A_cm2", 0) * 100 or e["mass_kg_m"] / RHO * 100, a)))
        area = best
    elif fam == "HSS_ROUND":  # AISC nominal weight: nominal wall t (area uses 0.93 t)
        area = math.pi * (e["OD_mm"] - e["t_mm"]) * e["t_mm"]
    elif fam == "HSS_RECT":   # AISC nominal weight: nominal t, outside corner radius 2 × 0.93 t
        t = e["t_mm"]
        area = hollow_rect_area(e["h_mm"], e["b_mm"], t, 1.86 * t, 0.86 * t)
    elif fam == "W":          # no fillet radius in the catalog → lower bound + ≈ 3 % fillets
        area = isp.properties(e["d_mm"], e["bf_mm"], e["tw_mm"], e["tf_mm"], 0)["A_cm2"] * 100 * 1.03
    elif fam in ("C", "MC", "WT"):  # inch catalog
        if "A_in2" in e and "mass_lb_ft" in e:
            close(w, "mass_lb_ft", e["mass_lb_ft"], e["A_in2"] * LB_FT_PER_IN2, 0.03)
        m = re.fullmatch(r"(?:C|MC|WT)[\d.]+x([\d.]+)", cid.split(":", 1)[-1])
        if m and "mass_lb_ft" in e:
            close(w, "mass_lb_ft (designation)", e["mass_lb_ft"], num(m[1]), 0.01)
        return
    if area is None:
        return
    tol = 0.05 if fam in ("W", "L") else 0.03
    if "A_cm2" in e:
        close(w, "A_cm2", e["A_cm2"], area / 100, tol)
    if "mass_kg_m" in e:
        close(w, "mass_kg_m", e["mass_kg_m"], area / 100 * RHO, tol)
    if "lbft" in e:
        close(w, "mass_kg_m (lb/ft)", e["mass_kg_m"], e["lbft"] * LB_FT, 0.01)
    if fam == "W":
        m = re.fullmatch(r"W[\d.]+x([\d.]+)", e.get("designation", ""))
        if m and "lbft" in e:
            close(w, "lbft (designation)", e["lbft"], num(m[1]), 0.01)


def check_series(entries, src):
    """A, mass and Iy must increase along a family sorted by the leading dimension."""
    fams = {}
    for e in entries:
        fams.setdefault(e.get("family"), []).append(e)
    for fam, es in fams.items():
        key = next((k for k in ("h_mm", "d_mm", "d_in") if all(k in e for e in es)), None)
        if fam not in ("HEA", "HEB", "HEM", "IPE", "UPE", "W", "T") or key is None:
            continue
        es = sorted(es, key=lambda e: e[key])
        for prop in ("A_cm2", "mass_kg_m", "Iy_cm4", "Ix_cm4"):
            vals = [(e["catalogId"], e[prop]) for e in es if prop in e]
            for (a, va), (b, vb) in zip(vals, vals[1:]):
                if fam != "W" and vb <= va:
                    bad(f"{src}:{b}", f"{prop} {vb} not larger than {a} ({va})")


def check_profiles():
    for f in sorted(CAT.glob("profiles-*.json")):
        es = load(f.name)["entries"]
        for e in es:
            check_profile(e, f.name)
        check_series(es, f.name)


# ── pipes and flanges ─────────────────────────────────────────────────────────

SCHED = ["SCH5", "SCH10", "SCH20", "SCH30", "SCH40", "SCH60", "SCH80",
         "SCH100", "SCH120", "SCH140", "SCH160"]


def pipe_mass(od, t):
    return math.pi * (od - t) * t * 7.85e-3


def check_pipe_wall(where, od, s):
    t = s["wall_mm"]
    checked[where.split(":")[0]] = checked.get(where.split(":")[0], 0) + 1
    if "bore_mm" in s and abs(s["bore_mm"] - (od - 2 * t)) > 0.06:
        bad(where, f"bore {s['bore_mm']} vs OD − 2t = {od - 2 * t:.2f}")
    if "mass_kg_m" in s:
        close(where, "mass_kg_m", s["mass_kg_m"], pipe_mass(od, t), 0.015)


# EN 10220 / ISO 4200: outside diameters of series 1 (by DN), preferred wall thicknesses, and the
# normal wall thicknesses of the former DIN 2448 (seamless) / DIN 2458 (welded) tables, which a
# catalog for recognition must offer (most drawn and scanned pipes have them).
EN_OD = {15: 21.3, 20: 26.9, 25: 33.7, 32: 42.4, 40: 48.3, 50: 60.3, 65: 76.1, 80: 88.9,
         100: 114.3, 125: 139.7, 150: 168.3, 200: 219.1, 250: 273.0, 300: 323.9, 350: 355.6,
         400: 406.4, 450: 457.0, 500: 508.0, 600: 610.0}
EN_WALLS = {1.6, 1.8, 2.0, 2.3, 2.6, 2.9, 3.2, 3.6, 4.0, 4.5, 5.0, 5.4, 5.6, 5.9, 6.3, 7.1, 8.0,
            8.8, 10.0, 11.0, 12.5, 14.2, 16.0, 17.5, 20.0}
EN_NORMAL_WALL = {21.3: (2.0, 2.0), 26.9: (2.3, 2.0), 33.7: (2.6, 2.0), 42.4: (2.6, 2.3),
                  48.3: (2.6, 2.3), 60.3: (2.9, 2.3), 76.1: (2.9, 2.6), 88.9: (3.2, 2.9),
                  114.3: (3.6, 3.2), 139.7: (4.0, 3.6), 168.3: (4.5, 4.0), 219.1: (5.9, 4.5),
                  273.0: (6.3, 5.0), 323.9: (7.1, 5.6), 355.6: (8.0, 5.6), 406.4: (8.8, 6.3),
                  457.0: (10.0, 6.3), 508.0: (11.0, 6.3), 610.0: (12.5, 6.3)}


def check_en_pipe(w, e):
    od = EN_OD.get(e["DN"])
    if od is None or abs(e["OD_mm"] - od) > 0.01:
        bad(w, f"DN{e['DN']} OD {e['OD_mm']} is not the EN 10220 series-1 diameter ({od})")
    walls = [s["wall_mm"] for s in e["wallThicknesses"]]
    for t in walls:
        if t not in EN_WALLS:
            bad(w, f"wall {t} not in the EN 10220 preferred wall-thickness series")
    if walls != sorted(set(walls)):
        bad(w, "wall thicknesses not strictly increasing")
    for t, kind in zip(EN_NORMAL_WALL.get(od, ()), ("seamless", "welded")):
        if t not in walls:
            bad(w, f"normal {kind} wall {t} (DIN 2448/2458) missing")


def check_pipes():
    for f, key in (("pipe-dimensions.json", "schedules"), ("pipe-dimensions-en.json", "wallThicknesses")):
        prev = None
        for e in load(f)["entries"]:
            w = f"{f}:{e['catalogId']}"
            if prev and (e["DN"] <= prev["DN"] or e["OD_mm"] <= prev["OD_mm"]):
                bad(w, "DN / OD not increasing")
            prev = e
            if key == "wallThicknesses":
                check_en_pipe(w, e)
            walls = []
            for s in e[key]:
                check_pipe_wall(f"{w} {s.get('schedule', s['wall_mm'])}", e["OD_mm"], s)
                if s.get("schedule") in SCHED:
                    walls.append((SCHED.index(s["schedule"]), s["schedule"], s["wall_mm"]))
            walls.sort()
            for (_, a, ta), (_, b, tb) in zip(walls, walls[1:]):
                if tb < ta:
                    bad(w, f"{b} wall {tb} thinner than {a} ({ta})")


def check_flanges():
    for f in ("flange-dimensions.json", "flange-dimensions-en.json"):
        by_class = {}
        for e in load(f)["entries"]:
            w = f"{f}:{e['catalogId']}"
            checked[f] = checked.get(f, 0) + 1
            if e["boltCircle_mm"] >= e["flangeOD_mm"]:
                bad(w, "bolt circle not inside the flange OD")
            if e["boltHoles"] % 4:
                bad(w, f"{e['boltHoles']} bolt holes (multiple of 4 expected)")
            if "boltHoleDiameter_mm" in e and e["boltHoleDiameter_mm"] <= e.get("boltDiameter_mm", 0):
                bad(w, "bolt hole not larger than the bolt")
            hole = e.get("boltHoleDiameter_mm", 0)
            if "raisedFace_mm" in e and e["raisedFace_mm"] >= e["boltCircle_mm"] - hole:
                bad(w, "raised face reaches the bolt holes")
            by_class.setdefault(e["pressureClass"], []).append(e)
        for cls, es in by_class.items():
            es.sort(key=lambda e: e["nominalDiameter"])
            for a, b in zip(es, es[1:]):
                for k in ("flangeOD_mm", "boltCircle_mm"):
                    if b[k] <= a[k]:
                        bad(f"{f}:{b['catalogId']}", f"{k} {b[k]} not larger than DN{a['nominalDiameter']} ({a[k]})")


# ── piping classes vs. dimension catalogs ─────────────────────────────────────

def check_piping_classes():
    asme = {}
    for e in load("pipe-dimensions.json")["entries"]:
        for s in e["schedules"]:
            asme[(e["DN"], s["schedule"])] = (e["OD_mm"], s)
    en = {(e["DN"], s["wall_mm"]): (e["OD_mm"], s)
          for e in load("pipe-dimensions-en.json")["entries"] for s in e["wallThicknesses"]}
    fl = {(e["nominalDiameter"], f"ANSI-{e['pressureClass']}"): e for e in load("flange-dimensions.json")["entries"]}
    fl_en = {(e["DN"], e["pressureClass"]): e for e in load("flange-dimensions-en.json")["entries"]}
    for f in sorted(CAT.glob("piping-class-*.json")):
        c = load(f.name)
        is_en = "en" in f.stem.split("piping-class-")[1]
        for e in c["entries"]:
            cc, dn = e["componentClass"], e.get("nominalDiameter_mm", e.get("DN"))
            w = f"{f.name}:{e['catalogId']}"
            if cc == "PIPE":
                sched = re.sub(r"S$", "", e.get("schedule") or "")
                ref = en.get((dn, e["wallThickness_mm"])) if is_en else asme.get((dn, sched))
                if ref is None:
                    bad(w, f"DN{dn} {e.get('schedule') or e['wallThickness_mm']} not in the pipe dimension catalog")
                    continue
                od, s = ref
                checked[f.name] = checked.get(f.name, 0) + 1
                for k, v in (("outsideDiameter_mm", od), ("wallThickness_mm", s["wall_mm"]),
                             ("bore_mm", s.get("bore_mm")), ("mass_kg_per_m", s.get("mass_kg_m"))):
                    if k in e and v is not None and abs(e[k] - v) > max(0.015 * v, 0.06):
                        bad(w, f"{k} {e[k]} vs {v} in the pipe dimension catalog")
            elif cc in ("FLANGE", "BLIND_FLANGE"):
                rating = e.get("rating") or c.get("rating")
                ref = fl_en.get((dn, rating.replace("-", ""))) if is_en else fl.get((dn, rating))
                if ref is None:
                    bad(w, f"DN{dn} {rating} not in the flange dimension catalog")
                    continue
                checked[f.name] = checked.get(f.name, 0) + 1
                # two field layouts are in use (see docs/AUDIT.md): A1A2/B3C1 name the flange OD
                # `outsideDiameter_mm`, C5D1/EN-P1A use `flangeOD_mm` (and `outsideDiameter_mm` = pipe OD)
                keys = ((("outsideDiameter_mm", "flangeOD_mm"), ("boltCircleDiameter_mm", "boltCircle_mm"),
                         ("numberOfBolts", "boltHoles")) if "boltCircleDiameter_mm" in e else
                        (("flangeOD_mm", "flangeOD_mm"), ("boltCircle_mm", "boltCircle_mm"), ("boltHoles", "boltHoles")))
                # EN 1092-1 blind flanges (type 05) have their own thicknesses; the catalog is type 11
                if not (is_en and cc == "BLIND_FLANGE"):
                    keys += (("flangeThickness_mm", "flangeThickness_mm"),)
                for k, rk in keys:
                    if k in e and abs(e[k] - ref[rk]) > max(0.01 * ref[rk], 0.5):
                        bad(w, f"{k} {e[k]} vs {ref[rk]} in the flange dimension catalog")


def main():
    global CAT
    if "--dir" in sys.argv:
        CAT = Path(sys.argv[sys.argv.index("--dir") + 1])
    check_profiles()
    check_pipes()
    check_flanges()
    check_piping_classes()
    if "--verbose" in sys.argv:
        for k, v in sorted(checked.items()):
            print(f"  {k}: {v} checks")
    for i in issues:
        print("✗", i)
    print(f"{'✗' if issues else '✓'} catalogs: {len(issues)} inconsistenc{'y' if len(issues) == 1 else 'ies'}")
    return 1 if issues else 0


if __name__ == "__main__":
    sys.exit(main())
