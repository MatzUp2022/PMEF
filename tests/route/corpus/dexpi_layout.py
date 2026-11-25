"""Synthetic 3D layout for a P&ID line list, to run real DEXPI line lists through `pmef route`.

Usage: python dexpi_layout.py PID.ndjson OUT_DIR [--class-map CODE=CLASS ...]

PID.ndjson is DEXPI-Manager `dexpi-pmef` output (e.g. the DEXPI reference P&ID C01: run the
DEXPI-Manager test `cargo test -p dexpi-pmef --test mapping` with DEXPI_PMEF_DUMP=<dir>). A P&ID
has no 3D placement, so this script invents one: every equipment item connected to a line becomes
a 1.5 × 1.5 × 2 m box on a 6 m grid, with its nozzles (marks from `connectionPointSource`) spread
over the four side faces at 1 m height. The 3D equipment carries the P&ID tag number only (no
`@id` link), so the router matches by tag. Writes OUT_DIR/layout.ndjson and OUT_DIR/problem.json.
The layout is synthetic; it tests the line-list pipeline (matching, DN/class, branches, skips),
not routing quality.
"""
import json, os, sys
from collections import defaultdict


def main(pid_path, out, class_map):
    objs = [json.loads(l) for l in open(pid_path) if l.strip()]
    by_id = {o["@id"]: o for o in objs if "@id" in o}
    lines = {o["@id"] for o in objs if o["@type"] == "pmef:PipingNetworkSystem"}
    marks = defaultdict(list)
    for o in objs:
        if o["@type"] == "pmef:IsConnectedTo" and o.get("targetId") in lines:
            m = o.get("connectionPointSource")
            if m and m not in marks[o["sourceId"]]:
                marks[o["sourceId"]].append(m)
    os.makedirs(out, exist_ok=True)
    project = objs[0].get("projectCode", "pid") if objs and objs[0]["@type"] == "pmef:FileHeader" else "pid"
    layout = []
    faces = [([1, 0, 0], 0), ([0, 1, 0], 1), ([-1, 0, 0], 0), ([0, -1, 0], 1)]
    for k, (eq, ms) in enumerate(sorted(marks.items())):
        src = by_id.get(eq, {})
        tag = (src.get("equipmentBasic") or {}).get("tagNumber") or src.get("tagNumber") or eq.split(":")[-1]
        cx, cy = (k % 3) * 6000.0, (k // 3) * 6000.0
        lo, hi = [cx - 750, cy - 750, 0.0], [cx + 750, cy + 750, 2000.0]
        nozzles = []
        for i, m in enumerate(ms):
            d, axis = faces[i % 4]
            slot = i // 4
            p = [cx, cy, 1000.0]
            p[0] += d[0] * 750
            p[1] += d[1] * 750
            p[1 - axis] += (slot - 1) * 400
            nozzles.append({"nozzleId": m, "nozzleMark": m, "coordinate": p, "direction": d})
        local = "".join(c if c.isalnum() or c == "-" else "-" for c in tag)
        layout.append({"@type": "pmef:GenericEquipment", "@id": f"urn:pmef:obj:{project}:L-{local}",
                       "equipmentBasic": {"tagNumber": tag}, "nozzles": nozzles,
                       "geometry": {"boundingBox": {"xMin": lo[0], "xMax": hi[0], "yMin": lo[1], "yMax": hi[1],
                                                    "zMin": lo[2], "zMax": hi[2]}}})
    with open(os.path.join(out, "layout.ndjson"), "w") as w:
        for o in layout:
            w.write(json.dumps(o, separators=(",", ":")) + "\n")
    prob = {"project": project, "revisionId": "r2026-11-25-001", "parent": f"urn:pmef:unit:{project}:U1",
            "grid": {"step": 100, "margin": 2000}, "settings": {"bendPenalty": 1000, "clearance": 50},
            "pmefScene": ["layout.ndjson"],
            "lineLists": [{"file": os.path.relpath(os.path.abspath(pid_path), os.path.abspath(out)),
                           "classMap": class_map}]}
    json.dump(prob, open(os.path.join(out, "problem.json"), "w"), indent=1)
    print(f"{len(layout)} equipment items, {sum(len(v) for v in marks.values())} nozzles, {len(lines)} lines")


if __name__ == "__main__":
    args = sys.argv[1:]
    cm = {}
    if "--class-map" in args:
        i = args.index("--class-map")
        for a in args[i + 1:]:
            k, v = a.split("=", 1)
            cm[k] = v
        args = args[:i]
    main(args[0], args[1], cm)
