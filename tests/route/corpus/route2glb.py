"""Combine routing obstacles and routed lines into one GLB for screenshots.

Usage: python route2glb.py OBSTACLE_DIR REPORT.json OUT.glb [--soft]

Meshes: `hard` (grey), `soft` (walls/slabs, drawn transparent by view-route.html, only with
--soft), `route-<id>` (pipe centreline swept with the outside diameter, one colour per line).
Coordinates: PMEF/IFC Z-up mm -> glTF Y-up m.
"""
import json, os, sys

import numpy as np
import trimesh

COLORS = [(230, 80, 30), (20, 140, 220), (40, 170, 70), (200, 40, 160), (240, 180, 0), (120, 70, 200)]


def load_obj(path, color, crop=None):
    m = trimesh.load(path, force="mesh", process=False)
    if crop is not None:
        c = m.triangles_center
        keep = np.all((c >= crop[0]) & (c <= crop[1]), axis=1)
        m.update_faces(keep)
        m.remove_unreferenced_vertices()
    m.visual.face_colors = np.tile([*color, 255], (len(m.faces), 1))
    return m


def pipe(points, od, color):
    parts = []
    for a, b in zip(points, points[1:]):
        a, b = np.array(a, float), np.array(b, float)
        if np.linalg.norm(b - a) < 1e-6:
            continue
        parts.append(trimesh.creation.cylinder(radius=od / 2, segment=[a, b], sections=16))
    for p in points[1:-1]:
        s = trimesh.creation.icosphere(subdivisions=1, radius=od / 2 * 1.15)
        s.apply_translation(p)
        parts.append(s)
    m = trimesh.util.concatenate(parts)
    m.visual.face_colors = np.tile([*color, 255], (len(m.faces), 1))
    return m


def main(obst, report, out, soft):
    routes = json.load(open(report))
    pts = np.array([p for r in routes if r["ok"] for p in r["points"]])
    crop = (pts.min(0) - 1500, pts.max(0) + 1500)  # only the surroundings of the routes
    sc = trimesh.Scene()
    sc.add_geometry(load_obj(f"{obst}/hard.obj", (150, 155, 165), crop), geom_name="hard")
    if soft:
        sc.add_geometry(load_obj(f"{obst}/soft.obj", (215, 205, 185), crop), geom_name="soft")
    for i, r in enumerate(routes):
        if r["ok"]:
            # Draw at least 60 mm so small bores stay visible in an overview.
            sc.add_geometry(pipe(r["points"], max(r["outsideDiameter"], float(os.environ.get("MIN_DRAW_OD", 60))), COLORS[i % len(COLORS)]),
                            geom_name="route-" + r["id"])
    t = trimesh.transformations.rotation_matrix(-np.pi / 2, [1, 0, 0]) @ np.diag([1e-3, 1e-3, 1e-3, 1])
    sc.apply_transform(t)
    sc.export(out)


if __name__ == "__main__":
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    main(a[0], a[1], a[2], "--soft" in sys.argv)
