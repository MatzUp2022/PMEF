"""Reference test: re-route pipe runs that are modelled in the corpus IFC files and compare.

Usage:
  python reference_runs.py extract OUT_DIR PLUMBING.ifc BUILDING_DIR [--max N] [--coarse STEP]
  python reference_runs.py compare OUT_DIR PMEF_BINARY

`extract` decomposes the domestic water systems of PLUMBING.ifc into runs (chains of pipes,
elbows, couplings and valves between two anchors: tees, fixtures, equipment or open ends, found
through IfcRelConnectsPorts). Runs whose pipes are all axis-parallel are references. For each it
writes OUT_DIR/<run>/problem.json: the same two end points and directions, the same DN, with the
run's own elements removed from the plumbing obstacles (all other plumbing stays hard; building
hard/soft obstacles come from BUILDING_DIR/hard.obj and soft.obj, written by ifc2obstacles.py)
and the reference centreline in OUT_DIR/<run>/reference.json.

`compare` routes every problem with `pmef route`, checks both centrelines against the obstacles
(mesh distance below the pipe radius = clash, ignoring 150 mm at each end where the run meets its
anchor), and writes OUT_DIR/summary.json plus a plan/elevation sheet per run and an overview
sheet (PNG). Needs IfcOpenShell, numpy, scipy, trimesh, matplotlib.
"""
import json, math, multiprocessing, os, subprocess, sys, time
from collections import defaultdict

import numpy as np

WATER = ("Domestic Hot Water", "Domestic Cold Water")
DNS = [15, 20, 25, 32, 40, 50, 65, 80, 100, 125, 150, 200]
AXIS_TOL = math.radians(2.0)
END_SKIP = 150.0


# ───────────────────────────── extract ─────────────────────────────

def load_plumbing(path):
    import ifcopenshell, ifcopenshell.geom, ifcopenshell.util.element as E
    f = ifcopenshell.open(path)
    s = ifcopenshell.geom.settings()
    s.set("use-world-coords", True)
    it = ifcopenshell.geom.iterator(s, f, multiprocessing.cpu_count())
    geo = {}
    if it.initialize():
        while True:
            sh = it.get()
            v = np.array(sh.geometry.verts, dtype=float).reshape(-1, 3) * 1000.0
            fa = np.array(sh.geometry.faces, dtype=int).reshape(-1, 3)
            if len(fa):
                geo[sh.guid] = (v, fa)
            if not it.next():
                break
    info = {}
    for e in f.by_type("IfcElement"):
        ps = E.get_psets(e)
        system = (ps.get("Mechanical") or {}).get("System Type") or ""
        # "Size" ("15 mmø") first: Clinic's Text.NominalDiameter is "1" for every pipe.
        size = (ps.get("Dimensions") or {}).get("Size") or ""
        dn = "".join(c for c in size.split("m")[0] if c.isdigit() or c == ".") or None
        if dn is None:
            dn = (ps.get("Text") or {}).get("NominalDiameter")
        info[e.GlobalId] = {"type": e.is_a(), "name": e.Name or "", "system": system,
                            "dn": float(dn) if dn not in (None, "") else None}
    port_el = {}
    for r in f.by_type("IfcRelConnectsPortToElement"):
        port_el[r.RelatingPort.id()] = r.RelatedElement.GlobalId
    nbr = defaultdict(set)
    nports = defaultdict(int)
    for r in f.by_type("IfcRelConnectsPortToElement"):
        nports[r.RelatedElement.GlobalId] += 1
    for r in f.by_type("IfcRelConnectsPorts"):
        a, b = port_el.get(r.RelatingPort.id()), port_el.get(r.RelatedPort.id())
        if a and b and a != b:
            nbr[a].add(b)
            nbr[b].add(a)
    return geo, info, nbr, nports


def segment_axis(v):
    c = v.mean(axis=0)
    u, s, vt = np.linalg.svd(v - c, full_matrices=False)
    d = vt[0]
    t = (v - c) @ d
    return c + t.min() * d, c + t.max() * d, d


def axis_parallel(d):
    a = np.abs(d) / np.linalg.norm(d)
    return a.max() > math.cos(AXIS_TOL)


def snap_axis(d):
    i = int(np.argmax(np.abs(d)))
    out = [0, 0, 0]
    out[i] = 1 if d[i] > 0 else -1
    return out


def line_corner(p1, d1, p2, d2):
    """Closest point between line p1+s d1 and p2+t d2 (midpoint)."""
    w = p1 - p2
    a, b, c, d, e = d1 @ d1, d1 @ d2, d2 @ d2, d1 @ w, d2 @ w
    den = a * c - b * b
    if abs(den) < 1e-9:
        return None
    s = (b * e - c * d) / den
    t = (a * e - b * d) / den
    return (p1 + s * d1 + p2 + t * d2) / 2


def runs_of(info, nbr, nports):
    water = {g for g, i in info.items() if any(w in i["system"] for w in WATER)}
    # Anchors: anything that is not a 2-connection inline element of the system.
    def inline(g):
        i = info[g]
        return (g in water and i["type"] in ("IfcFlowSegment", "IfcFlowFitting", "IfcFlowController")
                and len(nbr[g]) == 2 and nports[g] <= 2)
    seen = set()
    runs = []
    for g in sorted(water):
        if not inline(g) or g in seen:
            continue
        seen.add(g)
        chain = [g]
        ends = []
        for side, first in enumerate(sorted(nbr[g])):
            prev, cur = g, first
            while True:
                if not inline(cur):
                    ends.append(cur)
                    break
                if cur in seen:  # closed loop
                    ends.append(None)
                    break
                seen.add(cur)
                if side == 0:
                    chain.insert(0, cur)
                else:
                    chain.append(cur)
                nxt = [n for n in nbr[cur] if n != prev]
                if not nxt:
                    ends.append(None)
                    break
                prev, cur = cur, nxt[0]
        runs.append((ends[0], chain, ends[1]))
    return runs


def reference_line(chain, geo, info):
    segs = []
    for g in chain:
        if info[g]["type"] != "IfcFlowSegment" or g not in geo:
            continue
        a, b, d = segment_axis(geo[g][0])
        segs.append([a, b])
    if len(segs) < 2:
        return None
    # Orient the segments along the chain.
    for k in range(len(segs) - 1):
        a, b = segs[k]
        n0, n1 = segs[k + 1]
        if k == 0:
            # choose orientation of the first segment: its end b near the next segment
            if min(np.linalg.norm(a - n0), np.linalg.norm(a - n1)) < min(np.linalg.norm(b - n0), np.linalg.norm(b - n1)):
                segs[0] = [b, a]
                a, b = segs[0]
        if np.linalg.norm(n1 - b) < np.linalg.norm(n0 - b):
            segs[k + 1] = [n1, n0]
    dirs = [(s[1] - s[0]) / max(np.linalg.norm(s[1] - s[0]), 1e-9) for s in segs]
    if not all(axis_parallel(d) for d in dirs):
        return None
    dirs = [np.array(snap_axis(d), dtype=float) for d in dirs]
    pts = [segs[0][0]]
    bends = 0
    for k in range(len(segs) - 1):
        if abs(dirs[k] @ dirs[k + 1]) > 0.99:
            if dirs[k] @ dirs[k + 1] < 0:
                return None  # U-turn without a bend in between: not an orthogonal run
            # collinear (coupling / transition) or parallel offset: keep both ends
            if np.linalg.norm(np.cross(segs[k + 1][0] - segs[k][1], dirs[k])) > 5.0:
                pts += [segs[k][1], segs[k + 1][0]]
                bends += 2
            continue
        c = line_corner(segs[k][0], dirs[k], segs[k + 1][0], dirs[k + 1])
        if c is None:
            return None
        pts.append(c)
        bends += 1
    pts.append(segs[-1][1])
    length = sum(np.linalg.norm(pts[i + 1] - pts[i]) for i in range(len(pts) - 1))
    return {"points": [p.round(1).tolist() for p in pts], "bends": bends, "length": round(length, 1),
            "fromDir": snap_axis(dirs[0]), "toDir": snap_axis(-dirs[-1])}


def write_obj(path, parts, geo):
    with open(path, "w") as w:
        base = 0
        for g in parts:
            v, fa = geo[g]
            for p in v:
                w.write(f"v {p[0]:.1f} {p[1]:.1f} {p[2]:.1f}\n")
            for t in fa:
                w.write(f"f {t[0]+1+base} {t[1]+1+base} {t[2]+1+base}\n")
            base += len(v)


def extract(out, plumbing, building, max_runs=12, coarse=None):
    geo, info, nbr, nports = load_plumbing(plumbing)
    runs = []
    for a, chain, b in runs_of(info, nbr, nports):
        ref = reference_line(chain, geo, info)
        if not ref or ref["bends"] < 1 or ref["length"] < 1500:
            continue
        dns = [info[g]["dn"] for g in chain if info[g]["type"] == "IfcFlowSegment" and info[g]["dn"]]
        if not dns:
            continue
        dn = min(DNS, key=lambda x: abs(x - max(dns)))
        system = next(info[g]["system"] for g in chain if info[g]["system"])
        runs.append((ref, chain, a, b, dn, system))
    runs.sort(key=lambda r: -r[0]["length"])
    runs = runs[:max_runs]
    os.makedirs(out, exist_ok=True)
    index = []
    for k, (ref, chain, a, b, dn, system) in enumerate(runs, 1):
        rid = f"R{k:02d}"
        d = os.path.join(out, rid)
        os.makedirs(d, exist_ok=True)
        pts = np.array(ref["points"])
        lo, hi = pts.min(axis=0) - 2500, pts.max(axis=0) + 2500
        own = set(chain)
        near = [g for g, (v, _) in geo.items()
                if g not in own and np.all(v.max(axis=0) >= lo) and np.all(v.min(axis=0) <= hi)]
        write_obj(os.path.join(d, "plumbing.obj"), near, geo)
        grid = {"step": 50, "margin": 1500}
        if coarse:
            grid["coarseStep"] = coarse
        prob = {
            "project": "ref-test", "revisionId": "r2026-11-25-001", "parent": "urn:pmef:unit:ref-test:U1",
            "grid": grid,
            "settings": {"bendPenalty": 600, "clearance": 25, "penetrationPenalty": 3000, "verticalFactor": 1.0},
            "obstacles": [
                {"id": "plumbing", "kind": "hard", "obj": "plumbing.obj"},
                {"id": "building-hard", "kind": "hard", "obj": os.path.relpath(os.path.join(building, "hard.obj"), d)},
                {"id": "building-soft", "kind": "soft", "obj": os.path.relpath(os.path.join(building, "soft.obj"), d)},
            ],
            "routes": [{
                "id": rid, "pipeClass": "A1A2", "dn": dn,
                "from": {"point": ref["points"][0], "direction": ref["fromDir"], "owner": "plumbing"},
                "to": {"point": ref["points"][-1], "direction": ref["toDir"], "owner": "plumbing"},
            }],
        }
        json.dump(prob, open(os.path.join(d, "problem.json"), "w"), indent=1)
        meta = {"id": rid, "system": system, "dn": dn, "elements": len(chain),
                "fromAnchor": info[a]["name"] if a else None, "toAnchor": info[b]["name"] if b else None,
                **ref}
        json.dump(meta, open(os.path.join(d, "reference.json"), "w"), indent=1)
        index.append(meta)
        print(rid, system, f"DN{dn}", f"{ref['length']:.0f} mm", f"{ref['bends']} bends", len(chain), "elements")
    json.dump(index, open(os.path.join(out, "references.json"), "w"), indent=1)


# ───────────────────────────── compare ─────────────────────────────

def load_obj(path):
    v, f = [], []
    for line in open(path):
        if line.startswith("v "):
            v.append([float(x) for x in line.split()[1:4]])
        elif line.startswith("f "):
            f.append([int(x.split("/")[0]) - 1 for x in line.split()[1:4]])
    return np.array(v).reshape(-1, 3), np.array(f, dtype=int).reshape(-1, 3)


def sample(points, step=20.0):
    pts = np.array(points, dtype=float)
    out = []
    total = 0.0
    for a, b in zip(pts[:-1], pts[1:]):
        n = max(1, int(np.linalg.norm(b - a) // step))
        for i in range(n):
            out.append((a + (b - a) * i / n, total + np.linalg.norm(b - a) * i / n))
        total += np.linalg.norm(b - a)
    out.append((pts[-1], total))
    return np.array([p for p, _ in out]), np.array([s for _, s in out]), total


def clashes(points, radius, meshes):
    """Length [mm] of the centreline closer than `radius` to a hard mesh (ends excluded)."""
    import trimesh
    p, s, total = sample(points)
    keep = (s > END_SKIP) & (s < total - END_SKIP)
    p = p[keep]
    if not len(p):
        return 0.0
    hit = np.zeros(len(p), dtype=bool)
    for m in meshes:
        lo, hi = p.min(axis=0) - radius, p.max(axis=0) + radius
        tri = m.triangles
        sel = np.all(tri.max(axis=1) >= lo, axis=1) & np.all(tri.min(axis=1) <= hi, axis=1)
        if not sel.any():
            continue
        sub = trimesh.Trimesh(vertices=tri[sel].reshape(-1, 3), faces=np.arange(sel.sum() * 3).reshape(-1, 3), process=False)
        _, dist, _ = trimesh.proximity.closest_point(sub, p)
        hit |= dist < radius
    return float(hit.sum() * 20.0)


def deviation(a, b):
    from scipy.spatial import cKDTree
    pa, _, _ = sample(a, 10.0)
    pb, _, _ = sample(b, 10.0)
    da, _ = cKDTree(pb).query(pa)
    return float(da.mean()), float(da.max())


OD = {15: 21.3, 20: 26.7, 25: 33.4, 32: 42.2, 40: 48.3, 50: 60.3, 65: 73.0, 80: 88.9, 100: 114.3}


def in_soft(points, boxes):
    """Length [mm] of the centreline inside walls and slabs (bounding boxes of the soft elements
    from elements.json): a straight crossing adds about the wall thickness, a pipe in a chase its
    whole length there. Boxes of skewed walls are too large; both corpus buildings are orthogonal."""
    p, s, total = sample(points)
    keep = (s > END_SKIP) & (s < total - END_SKIP)
    p = p[keep]
    if not len(p) or boxes is None or not len(boxes):
        return 0.0
    lo, hi = boxes[:, 0], boxes[:, 1]
    inside = np.zeros(len(p), dtype=bool)
    for k in range(0, len(p), 2000):
        q = p[k:k + 2000, None, :]
        inside[k:k + 2000] = np.any(np.all((q > lo) & (q < hi), axis=2), axis=1)
    return float(inside.sum() * 20.0)


MODES = {"default": "walls and slabs soft (crossed straight, penalty)", "chases": "pipes may run inside walls and slabs"}


def run_mode(pmef, d, mode):
    prob = json.load(open(os.path.join(d, "problem.json")))
    if mode == "chases":
        prob["obstacles"] = [o for o in prob["obstacles"] if o["kind"] != "soft"]
    path = os.path.join(d, f"problem-{mode}.json")
    json.dump(prob, open(path, "w"), indent=1)
    routed, report = os.path.join(d, f"routed-{mode}.ndjson"), os.path.join(d, f"report-{mode}.json")
    # Stale outputs of an earlier run must never stand in for this one.
    for f in (routed, report):
        if os.path.exists(f):
            os.remove(f)
    t0 = time.time()
    # A non-zero exit with a report means unroutable lines (reported per route); no report
    # means the router itself failed.
    res = subprocess.run([pmef, "route", path, "-o", routed, "--report", report],
                         capture_output=True, text=True, timeout=1800)
    if not os.path.exists(report):
        raise RuntimeError(f"pmef route failed for {path} (exit {res.returncode}): {res.stderr.strip()}")
    rep = json.load(open(report))[0]
    rep["seconds"] = round(time.time() - t0, 1)
    return rep


def compare(out, pmef):
    import trimesh
    refs = json.load(open(os.path.join(out, "references.json")))
    rows = []
    for ref in refs:
        d = os.path.join(out, ref["id"])
        prob = json.load(open(os.path.join(d, "problem.json")))
        meshes, soft = [], None
        for o in prob["obstacles"]:
            path = os.path.normpath(os.path.join(d, o["obj"]))
            if o["kind"] == "hard":
                v, f = load_obj(path)
                meshes.append(trimesh.Trimesh(vertices=v, faces=f, process=False))
            else:
                els = json.load(open(os.path.join(os.path.dirname(path), "elements.json")))
                soft = np.array([[e["min"], e["max"]] for e in els if e["kind"] == "soft"])
        radius = OD.get(ref["dn"], ref["dn"] * 1.2) / 2
        row = {"id": ref["id"], "system": ref["system"].replace("Piping System: ", ""), "dn": ref["dn"],
               "refLength": ref["length"], "refBends": ref["bends"],
               "refClash": clashes(ref["points"], radius, meshes), "refInSoft": in_soft(ref["points"], soft)}
        reps = {}
        for mode in MODES:
            rep = run_mode(pmef, d, mode)
            reps[mode] = rep
            r = {"ok": rep["ok"], "seconds": rep["seconds"]}
            if rep["ok"]:
                r.update({"length": round(rep["lengthMm"], 1), "bends": rep["bends"],
                          "penetrations": rep["penetrations"], "clash": clashes(rep["points"], radius, meshes),
                          "inSoft": in_soft(rep["points"], soft),
                          "lengthRatio": round(rep["lengthMm"] / ref["length"], 3)})
                r["meanDev"], r["maxDev"] = [round(x) for x in deviation(rep["points"], ref["points"])]
            else:
                r["error"] = rep.get("error")
            row[mode] = r
        rows.append(row)
        print(json.dumps(row))
        sheet(d, ref, reps, meshes, row)
    json.dump(rows, open(os.path.join(out, "summary.json"), "w"), indent=1)
    overview(out, refs, rows)


STYLE = {"default": ("#dd6b20", "--"), "chases": ("#805ad5", ":")}


def _draw(ax, ref, reps, meshes, i, j, lo, hi):
    from matplotlib.collections import PolyCollection
    polys = []
    for m in meshes[:1]:  # plumbing context
        tri = m.triangles
        sel = np.all(tri.max(axis=1) >= lo, axis=1) & np.all(tri.min(axis=1) <= hi, axis=1)
        polys += [t[:, [i, j]] for t in tri[sel][:: max(1, sel.sum() // 20000)]]
    if polys:
        ax.add_collection(PolyCollection(polys, facecolors="#c9ced6", edgecolors="none", alpha=0.35))
    r = np.array(ref["points"])
    ax.plot(r[:, i], r[:, j], color="#2b6cb0", lw=3.2, label=f"reference ({ref['length']/1000:.2f} m, {ref['bends']} bends)")
    for mode, rep in reps.items():
        if rep.get("ok"):
            q = np.array(rep["points"])
            c, ls = STYLE[mode]
            ax.plot(q[:, i], q[:, j], color=c, lw=1.8, ls=ls,
                    label=f"pmef route, {mode} ({rep['lengthMm']/1000:.2f} m, {rep['bends']} bends)")
    ax.plot(*r[0, [i, j]], "o", color="#2f855a", ms=7)
    ax.plot(*r[-1, [i, j]], "s", color="#2f855a", ms=7)
    ax.set_xlim(lo[i], hi[i]); ax.set_ylim(lo[j], hi[j])
    ax.set_aspect("equal"); ax.tick_params(labelsize=7)


def sheet(d, ref, reps, meshes, row):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    pts = np.array(ref["points"] + sum((r["points"] for r in reps.values() if r.get("ok")), []))
    lo, hi = pts.min(axis=0) - 600, pts.max(axis=0) + 600
    ext = hi - lo
    side = (0, 2) if ext[0] >= ext[1] else (1, 2)
    fig, axes = plt.subplots(1, 2, figsize=(12, 5.5))
    _draw(axes[0], ref, reps, meshes, 0, 1, lo, hi)
    axes[0].set_title("plan (x–y) [mm]", fontsize=9)
    _draw(axes[1], ref, reps, meshes, side[0], side[1], lo, hi)
    axes[1].set_title(f"elevation ({'xy'[side[0]]}–z) [mm]", fontsize=9)
    axes[0].legend(fontsize=7.5, loc="best")
    parts = []
    for mode in reps:
        m = row[mode]
        parts.append(f"{mode}: " + (f"×{m['lengthRatio']:.2f} length, {m['bends'] - row['refBends']:+d} bends, Ø dev {m['meanDev']} mm"
                                    if m["ok"] else "not routed"))
    fig.suptitle(f"{ref['id']} · {row['system']} · DN{ref['dn']} · reference in walls/slabs {row['refInSoft']/1000:.2f} m\n" + " | ".join(parts), fontsize=9.5)
    fig.tight_layout()
    fig.savefig(os.path.join(d, "compare.png"), dpi=110)
    plt.close(fig)


def overview(out, refs, rows):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from PIL import Image
    imgs = [Image.open(os.path.join(out, r["id"], "compare.png")) for r in refs]
    if not imgs:
        return
    w, h = imgs[0].size
    cols = 2
    rows_n = math.ceil(len(imgs) / cols)
    sheet_img = Image.new("RGB", (w * cols, h * rows_n), "white")
    for k, im in enumerate(imgs):
        sheet_img.paste(im, ((k % cols) * w, (k // cols) * h))
    sheet_img.save(os.path.join(out, "overview.png"))


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "extract":
        args = sys.argv[2:]
        mx = int(args[args.index("--max") + 1]) if "--max" in args else 12
        coarse = int(args[args.index("--coarse") + 1]) if "--coarse" in args else None
        extract(args[0], args[1], args[2], mx, coarse)
    elif cmd == "compare":
        compare(sys.argv[2], sys.argv[3])
    else:
        sys.exit(__doc__)
