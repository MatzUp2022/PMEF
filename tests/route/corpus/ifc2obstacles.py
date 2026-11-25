"""Turn IFC models of plant-test-corpus into routing obstacles (OBJ, mm) plus an element index.

Usage: python ifc2obstacles.py OUT_DIR MODEL.ifc [MODEL.ifc ...]

Writes OUT_DIR/hard.obj (equipment, structure, MEP, furniture), OUT_DIR/soft.obj (walls,
slabs, roofs, coverings: may be crossed straight with a sleeve) and OUT_DIR/elements.json
(type, GlobalId, name, bbox in mm) for picking terminals. IfcSpace/IfcOpeningElement are
skipped. Needs IfcOpenShell (pip install ifcopenshell). Corpus files: plant-test-corpus,
`python3 fetch.py --set ifc` (PLANT_TEST_CORPUS).
"""
import json, multiprocessing, os, sys

import ifcopenshell
import ifcopenshell.geom

SOFT = {"IfcWall", "IfcWallStandardCase", "IfcSlab", "IfcRoof", "IfcCovering", "IfcCurtainWall"}
# Furniture and cabinets are not obstacles for building services (pipes run through them).
SKIP = {"IfcFurnishingElement", "IfcFurniture", "IfcSpace", "IfcOpeningElement", "IfcAnnotation", "IfcVirtualElement", "IfcGrid", "IfcSite",
        "IfcDoor", "IfcWindow"}


def main(out, models):
    os.makedirs(out, exist_ok=True)
    files = {k: open(os.path.join(out, k + ".obj"), "w") for k in ("hard", "soft")}
    base = {"hard": 0, "soft": 0}
    elements = []
    for path in models:
        f = ifcopenshell.open(path)
        s = ifcopenshell.geom.settings()
        s.set("use-world-coords", True)
        it = ifcopenshell.geom.iterator(s, f, multiprocessing.cpu_count())
        if not it.initialize():
            continue
        while True:
            sh = it.get()
            t = sh.type
            if t not in SKIP:
                v = [x * 1000.0 for x in sh.geometry.verts]  # IfcOpenShell world coords are metres
                fa = sh.geometry.faces
                if fa:
                    kind = "soft" if t in SOFT else "hard"
                    w = files[kind]
                    for i in range(0, len(v), 3):
                        w.write(f"v {v[i]:.1f} {v[i+1]:.1f} {v[i+2]:.1f}\n")
                    b = base[kind]
                    for i in range(0, len(fa), 3):
                        w.write(f"f {fa[i]+1+b} {fa[i+1]+1+b} {fa[i+2]+1+b}\n")
                    base[kind] += len(v) // 3
                    xs, ys, zs = v[0::3], v[1::3], v[2::3]
                    elements.append({"model": os.path.basename(path), "type": t, "guid": sh.guid, "name": sh.name,
                                     "kind": kind, "min": [min(xs), min(ys), min(zs)], "max": [max(xs), max(ys), max(zs)]})
            if not it.next():
                break
    for w in files.values():
        w.close()
    json.dump(elements, open(os.path.join(out, "elements.json"), "w"))
    print(f"{len(elements)} elements, vertices: {base}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:])
