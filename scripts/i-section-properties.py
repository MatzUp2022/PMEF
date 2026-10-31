#!/usr/bin/env python3
"""Section properties of rolled I/H sections from their dimensions (h, b, tw, tf, r).

Used to check and correct catalogs/profiles-en.json. Formulas: rectangles for flanges and
web plus the four root fillets (area (1 − π/4)·r², centroid 0.2234·r from the corner).

    python scripts/i-section-properties.py --check catalogs/profiles-en.json
"""
import argparse
import json
import math
import sys

FILLET_A = 1 - math.pi / 4
FILLET_E = 0.2234  # centroid offset of a fillet spandrel from its corner, × r


def properties(h, b, tw, tf, r):
    hw = h - 2 * tf
    area = 2 * b * tf + hw * tw + 4 * FILLET_A * r * r
    a = FILLET_A * r * r
    iy = (b * h**3 - (b - tw) * hw**3) / 12 + 4 * a * (hw / 2 - FILLET_E * r) ** 2
    iz = (2 * tf * b**3 + hw * tw**3) / 12 + 4 * a * (tw / 2 + FILLET_E * r) ** 2
    return {"A_cm2": area / 100, "Iy_cm4": iy / 1e4, "Iz_cm4": iz / 1e4,
            "Wel_y_cm3": iy / 1e3 / (h / 2), "Wel_z_cm3": iz / 1e3 / (b / 2)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", required=True)
    ap.add_argument("--families", default="HEA,HEB,HEM,IPE")
    ap.add_argument("--tolerance", type=float, default=0.02)
    args = ap.parse_args()
    fams = set(args.families.split(","))
    entries = json.load(open(args.check, encoding="utf-8"))["entries"]
    worst, bad = 0.0, 0
    for e in entries:
        if e.get("family") not in fams:
            continue
        p = properties(e["h_mm"], e["b_mm"], e["tw_mm"], e["tf_mm"], e.get("r_mm", 0))
        devs = {k: (e[k] - v) / v for k, v in p.items() if k in e}
        k_max = max(devs, key=lambda k: abs(devs[k]))
        worst = max(worst, abs(devs[k_max]))
        if abs(devs[k_max]) > args.tolerance:
            bad += 1
            print(f"{e['catalogId']}: {k_max} catalog {e[k_max]} vs computed {p[k_max]:.1f} ({100 * devs[k_max]:+.1f} %)")
    print(f"max deviation {100 * worst:.2f} %, {bad} entries outside ±{100 * args.tolerance:.0f} %")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
