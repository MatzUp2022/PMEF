#!/usr/bin/env python3
"""Generate the PMEF conformance test catalogue and its fixtures.

Single source of truth for tests/conformance/manifest.json and
tests/conformance/fixtures/. Fixtures are synthetic (CC0); valid objects are taken
from examples/ where a type exists there, otherwise built minimally here.

    python tests/conformance/tools/generate_fixtures.py          # (re)write
    python tests/conformance/tools/generate_fixtures.py --check  # CI: fail if out of date
"""
from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CONF = ROOT / "tests" / "conformance"
FIXTURES = CONF / "fixtures"

V = "0.9.0"
PRJ = "conf"
REV = {"revisionId": "r2026-10-31-001", "changeState": "SHARED", "authoringTool": "PMEF conformance fixtures"}

# ── base objects ────────────────────────────────────────────────────────────


RFC_EXAMPLES = {"asbuilt-recognition.ndjson"}  # 0.10.0 feature examples, used only where named


def _load_examples(names: set[str] | None = None) -> list[dict]:
    """Objects from examples/. Default: the 0.9.0 benchmark datasets only (stable fixtures)."""
    objs = []
    for p in sorted((ROOT / "examples").glob("*.ndjson")):
        if (p.name in RFC_EXAMPLES) != (names is not None) or (names is not None and p.name not in names):
            continue
        for line in p.read_text(encoding="utf-8").splitlines():
            if line.startswith("{"):
                objs.append(json.loads(line))
    return objs


EXAMPLES = _load_examples()
ASBUILT = _load_examples({"asbuilt-recognition.ndjson"})


def ex(type_: str, pred=lambda o: True, source: list[dict] | None = None) -> dict:
    """First example object of a type (deep copy)."""
    for o in source if source is not None else EXAMPLES:
        if o["@type"] == type_ and pred(o):
            return copy.deepcopy(o)
    raise SystemExit(f"no example object for {type_}")


def oid(domain: str, local: str) -> str:
    return f"urn:pmef:{domain}:{PRJ}:{local}"


def comp(type_: str, local: str, cls: str, **extra) -> dict:
    o = {"@type": type_, "@id": oid("obj", local), "pmefVersion": V, "isPartOf": oid("seg", "L-1-S1"),
         "componentSpec": {"componentClass": cls}, "revision": REV}
    o.update(extra)
    return o


def equip(type_: str, tag: str, cls: str, **extra) -> dict:
    o = {"@type": type_, "@id": oid("obj", tag), "pmefVersion": V, "isPartOf": oid("unit", "U-1"),
         "equipmentBasic": {"tagNumber": tag, "equipmentClass": cls}, "revision": REV}
    o.update(extra)
    return o


def rel(type_: str, code: str, local: str, src: str, tgt: str, **extra) -> dict:
    o = {"@type": type_, "@id": oid("rel", local), "relationType": code, "sourceId": src, "targetId": tgt,
         "revision": REV}
    o.update(extra)
    return o


def geom(local: str, primitive: dict) -> dict:
    return {"@type": "pmef:ParametricGeometry", "@id": oid("geom", local), "ownerRef": oid("obj", "OWNER-1"),
            "lod": "LOD2_MEDIUM", "primitive": primitive, "revision": REV}


def without(o: dict, *path: str) -> dict:
    o = copy.deepcopy(o)
    cur = o
    for k in path[:-1]:
        cur = cur[k]
    del cur[path[-1]]
    return o


def with_(o: dict, path: tuple, value) -> dict:
    o = copy.deepcopy(o)
    cur = o
    for k in path[:-1]:
        cur = cur[k]
    cur[path[-1]] = value
    return o


# ── unit tests: (id, name, level, domain, requirement, expect, objects) ─────

CYL = {"type": "CYLINDER", "center": [0, 0, 0], "axis": [0, 0, 1], "radius": 100.0, "length": 500.0}
VESSEL = ex("pmef:Vessel")
PNS = ex("pmef:PipingNetworkSystem", lambda o: "specification" in o)
PIPE = ex("pmef:Pipe")
INSTR = ex("pmef:InstrumentObject")
MEMBER = ex("pmef:SteelMember")

UNIT = [
    # Piping
    ("PIPE-U-001", "PipingNetworkSystem — valid", 1, "piping", "CL1-06", "valid", [PNS]),
    ("PIPE-U-002", "PipingSegment — valid", 1, "piping", "CL1-06", "valid", [ex("pmef:PipingSegment")]),
    ("PIPE-U-003", "PipingNetworkSystem without lineNumber — rejected", 1, "piping", "CL1-02", "invalid",
     [without(PNS, "lineNumber")]),
    ("PIPE-U-004", "Pipe — valid", 1, "piping", "CL1-06", "valid", [PIPE]),
    ("PIPE-U-005", "Elbow — valid", 1, "piping", "CL1-06", "valid", [ex("pmef:Elbow")]),
    ("PIPE-U-006", "Tee — valid", 1, "piping", "CL1-06", "valid", [comp("pmef:Tee", "TEE-1", "TEE")]),
    ("PIPE-U-007", "Reducer (concentric and eccentric) — valid", 1, "piping", "CL1-06", "valid",
     [comp("pmef:Reducer", "RED-1", "REDUCER_CONCENTRIC"), comp("pmef:Reducer", "RED-2", "REDUCER_ECCENTRIC")]),
    ("PIPE-U-008", "Olet — valid", 2, "piping", "CL2-01", "valid", [comp("pmef:Olet", "OLET-1", "OLET_WELDOLET", oletType="WELDOLET", branchDiameter=50.0)]),
    ("PIPE-U-009", "Gasket — valid", 2, "piping", "CL2-01", "valid", [ex("pmef:Gasket")]),
    ("PIPE-U-010", "Weld with WeldSpec — valid", 2, "piping", "CL2-01", "valid", [ex("pmef:Weld")]),
    ("PIPE-U-011", "Flange — valid", 1, "piping", "CL1-06", "valid", [ex("pmef:Flange")]),
    ("PIPE-U-012", "Spool — valid", 3, "piping", "CL3-01", "valid", [ex("pmef:Spool")]),
    ("PIPE-U-013", "Valve with ValveSpec — valid", 1, "piping", "CL1-06", "valid", [ex("pmef:Valve")]),
    ("PIPE-U-014", "PipingSpecification on a line — valid", 1, "piping", "CL1-06", "valid", [PNS]),
    ("PIPE-U-015", "Pipe without componentSpec — rejected", 1, "piping", "CL1-02", "invalid",
     [without(PIPE, "componentSpec")]),
    ("PIPE-U-016", "PipeSupport with SupportSpec — valid", 2, "piping", "CL2-01", "valid",
     [ex("pmef:PipeSupport", lambda o: "supportSpec" in o)]),
    # Equipment
    ("EQUIP-U-001", "Vessel with VesselDesign — valid", 1, "equipment", "CL1-07", "valid", [VESSEL]),
    ("EQUIP-U-002", "Tank — valid", 1, "equipment", "CL1-07", "valid", [equip("pmef:Tank", "T-1", "STORAGE_TANK")]),
    ("EQUIP-U-003", "Pump with PumpSpec — valid", 1, "equipment", "CL1-07", "valid", [ex("pmef:Pump")]),
    ("EQUIP-U-004", "HeatExchanger — valid", 1, "equipment", "CL1-07", "valid", [ex("pmef:HeatExchanger")]),
    ("EQUIP-U-005", "Compressor with CompressorSpec — valid", 2, "equipment", "CL2-01", "valid",
     [ex("pmef:Compressor")]),
    ("EQUIP-U-006", "GenericEquipment — valid", 1, "equipment", "CL1-07", "valid",
     [equip("pmef:GenericEquipment", "X-1", "GENERIC")]),
    ("EQUIP-U-007", "Column — valid", 2, "equipment", "CL2-01", "valid",
     [equip("pmef:Column", "C-1", "COLUMN", vesselDesign=copy.deepcopy(VESSEL.get("vesselDesign", {})))]),
    ("EQUIP-U-008", "Reactor — valid", 2, "equipment", "CL2-01", "valid",
     [equip("pmef:Reactor", "R-1", "REACTOR", vesselDesign=copy.deepcopy(VESSEL.get("vesselDesign", {})))]),
    ("EQUIP-U-009", "Embedded Nozzle — valid", 1, "equipment", "CL1-07", "valid",
     [ex("pmef:Pump", lambda o: bool(o.get("nozzles")))]),
    ("EQUIP-U-010", "Filter and Turbine — valid", 2, "equipment", "CL2-01", "valid",
     [ex("pmef:Filter"), ex("pmef:Turbine")]),
    ("EQUIP-U-011", "Equipment without equipmentBasic — rejected", 1, "equipment", "CL1-02", "invalid",
     [without(VESSEL, "equipmentBasic")]),
    ("EQUIP-U-012", "Nozzle without coordinate — rejected", 1, "equipment", "CL1-02", "invalid",
     [without(ex("pmef:Pump", lambda o: bool(o.get("nozzles"))), "nozzles", 0, "coordinate")]),
    # E&I
    ("EI-U-001", "InstrumentObject — valid", 2, "ei", "CL2-01", "valid", [INSTR]),
    ("EI-U-002", "InstrumentLoop — valid", 2, "ei", "CL2-01", "valid", [ex("pmef:InstrumentLoop")]),
    ("EI-U-003", "InstrumentObject without tagNumber — rejected", 2, "ei", "CL1-02", "invalid",
     [without(INSTR, "tagNumber")]),
    ("EI-U-004", "PLCObject — valid", 2, "ei", "CL2-01", "valid", [ex("pmef:PLCObject")]),
    ("EI-U-005", "CableObject — valid", 2, "ei", "CL2-01", "valid", [ex("pmef:CableObject")]),
    ("EI-U-006", "CableTrayRun — valid", 2, "ei", "CL2-01", "valid", [ex("pmef:CableTrayRun")]),
    ("EI-U-007", "MTPModule — valid", 2, "ei", "CL2-01", "valid",
     [{"@type": "pmef:MTPModule", "@id": oid("obj", "MTP-1"), "isPartOf": oid("unit", "U-1"),
       "moduleName": "DosingModule", "revision": REV}]),
    ("EI-U-008", "InstrumentObject with safetySpec (SIL) — valid", 2, "ei", "CL2-01", "valid",
     [ex("pmef:InstrumentObject", lambda o: "safetySpec" in o)]),
    # Structural steel
    ("STEEL-U-001", "SteelSystem — valid", 2, "steel", "CL2-01", "valid",
     [{"@type": "pmef:SteelSystem", "@id": oid("obj", "SS-1"), "pmefVersion": V, "isPartOf": oid("unit", "U-1"),
       "systemName": "Pipe rack A", "systemType": "PIPE_RACK", "memberIds": [oid("obj", "B-1")], "revision": REV}]),
    ("STEEL-U-002", "SteelMember — valid", 2, "steel", "CL2-01", "valid", [MEMBER]),
    ("STEEL-U-003", "SteelNode — valid", 2, "steel", "CL2-01", "valid",
     [{"@type": "pmef:SteelNode", "@id": oid("obj", "N-1"), "isPartOf": oid("obj", "SS-1"),
       "coordinate": [0.0, 0.0, 4200.0], "revision": REV}]),
    ("STEEL-U-004", "SteelConnection — valid", 2, "steel", "CL2-01", "valid",
     [{"@type": "pmef:SteelConnection", "@id": oid("obj", "CN-1"), "isPartOf": oid("obj", "SS-1"),
       "connectionType": "BOLTED_ENDPLATE", "memberIds": [oid("obj", "B-1"), oid("obj", "C-1")],
       "revision": REV}]),
    ("STEEL-U-005", "SteelMember without profileId — rejected", 2, "steel", "CL1-02", "invalid",
     [without(MEMBER, "profileId")]),
    ("STEEL-U-006", "SteelMember with provenance / review / asBuilt (RFC-0001) — valid", 2, "steel",
     "CL2-01", "valid", [ex("pmef:SteelMember", lambda o: "asBuilt" in o, ASBUILT)]),
    # Relationships
    ("REL-U-001", "IsPartOf — valid", 2, "relationships", "CL2-01", "valid",
     [rel("pmef:IsPartOf", "IS_PART_OF", "R-1", oid("obj", "P-1"), oid("unit", "U-1"))]),
    ("REL-U-002", "IsConnectedTo — valid", 2, "relationships", "CL2-01", "valid",
     [rel("pmef:IsConnectedTo", "IS_CONNECTED_TO", "R-2", oid("obj", "P-1"), oid("obj", "P-2"))]),
    ("REL-U-003", "IsDerivedFrom — valid", 2, "relationships", "CL2-06", "valid", [ex("pmef:IsDerivedFrom")]),
    ("REL-U-004", "Supports — valid", 2, "relationships", "CL2-01", "valid", [ex("pmef:Supports")]),
    ("REL-U-005", "ControlledBy — valid", 2, "relationships", "CL2-01", "valid", [ex("pmef:ControlledBy")]),
    ("REL-U-006", "HasEquivalentIn — valid", 2, "relationships", "CL2-05", "valid", [ex("pmef:HasEquivalentIn")]),
    ("REL-U-007", "IsRevisionOf — valid", 2, "relationships", "CL3-06", "valid",
     [rel("pmef:IsRevisionOf", "IS_REVISION_OF", "R-7", oid("obj", "P-1"), oid("obj", "P-1-OLD"))]),
    ("REL-U-008", "Relationship without targetId — rejected", 2, "relationships", "CL1-02", "invalid",
     [without(ex("pmef:HasEquivalentIn"), "targetId")]),
    # Geometry
    ("GEOM-U-001", "CYLINDER — valid", 2, "geometry", "CL2-03", "valid", [geom("G-1", CYL)]),
    ("GEOM-U-002", "CONE — valid", 2, "geometry", "CL2-03", "valid",
     [geom("G-2", {"type": "CONE", "center": [0, 0, 0], "axis": [0, 0, 1], "r1": 100.0, "r2": 50.0,
                   "length": 200.0})]),
    ("GEOM-U-003", "CIRC_TORUS — valid", 2, "geometry", "CL2-03", "valid",
     [ex("pmef:ParametricGeometry", lambda o: o["primitive"]["type"] == "CIRC_TORUS")]),
    ("GEOM-U-004", "BOX — valid", 2, "geometry", "CL2-03", "valid",
     [geom("G-4", {"type": "BOX", "center": [0, 0, 0], "xLen": 100.0, "yLen": 200.0, "zLen": 50.0})]),
    ("GEOM-U-005", "COMPOSITE — valid", 2, "geometry", "CL2-03", "valid",
     [ex("pmef:ParametricGeometry", lambda o: o["primitive"]["type"] == "COMPOSITE")]),
    ("GEOM-U-006", "MESH_REF (glTF) — valid", 2, "geometry", "CL2-03", "valid",
     [geom("G-6", {"type": "MESH_REF", "meshUri": "file:meshes/demo.glb", "meshFormat": "GLTF"})]),
    ("GEOM-U-007", "SPHERE, DISH and SNOUT — valid", 2, "geometry", "CL2-01", "valid",
     [geom("G-7a", {"type": "SPHERE", "center": [0, 0, 0], "radius": 50.0}),
      ex("pmef:ParametricGeometry", lambda o: "DISH" in json.dumps(o)),
      geom("G-7c", {"type": "SNOUT", "center": [0, 0, 0], "axis": [0, 0, 1], "r1": 100.0, "r2": 80.0,
                    "length": 150.0, "offsetX": 10.0})]),
    ("GEOM-U-008", "CYLINDER with radius 0 — rejected", 2, "geometry", "CL1-02", "invalid",
     [geom("G-8", with_(CYL, ("radius",), 0))]),
]

# ── integration and round-trip tests (evaluated by the runner) ──────────────

INTG = [
    ("INTG-001", "Exactly one FileHeader, written as the first object", 2, "R-GEN-01"),
    ("INTG-002", "@id values unique and well-formed", 2, "R-GEN-02"),
    ("INTG-003", "isPartOf references resolve within the file", 2, "CL2-04"),
    ("INTG-004", "Port.connectedTo references resolve and are reciprocal", 2, "CL2-04"),
    ("INTG-005", "Every object except FileHeader carries RevisionMetadata", 2, "CL1-08"),
    ("INTG-006", "Every exported physical object has pmef:HasEquivalentIn (adapters only)", 2, "CL2-05"),
]

RT = [
    ("RT-001", "@id set preserved", 1, "CL3-04"),
    ("RT-002", "@type preserved", 1, "CL1-01"),
    ("RT-003", "PipingNetworkSystem.lineNumber preserved", 1, "CL3-04"),
    ("RT-004", "EquipmentBasic.tagNumber preserved", 1, "CL3-04"),
    ("RT-005", "RevisionMetadata.revisionId and changeState preserved", 1, "CL1-08"),
    ("RT-006", "Equipment nozzles preserved (count, ids, DN)", 1, "CL1-07"),
    ("RT-007", "isPartOf hierarchy preserved", 1, "CL1-05"),
    ("RT-008", "Piping design conditions preserved (Pa ±1, K ±0.01)", 2, "CL2-07"),
    ("RT-009", "Port coordinates and directions preserved (±0.01 mm)", 2, "CL2-07"),
    ("RT-010", "WeldSpec.weldNumber preserved", 2, "CL3-04"),
    ("RT-011", "PumpSpec preserved", 2, "CL2-07"),
    ("RT-012", "VesselDesign preserved", 2, "CL2-07"),
    ("RT-013", "PipingSpecification preserved", 1, "CL1-06"),
    ("RT-014", "catalogRef preserved", 2, "CL2-07"),
    ("RT-015", "Relationship sourceId / targetId preserved", 2, "CL2-07"),
    ("RT-016", "Unknown @type and unknown properties do not break the round trip", 2, "CL1-03, CL1-04"),
    ("RT-017", "customAttributes preserved exactly", 1, "R-GEN-08"),
    ("RT-018", "Aggregate fidelity ≥ 95 % (L2) / ≥ 98 % (L3), identity fields 100 %", 2, "CL2-07, CL3-02"),
]

EXTENSIONS = [  # supplementary round-trip input for RT-016 / RT-017
    {"@type": "pmef:FileHeader", "@id": oid("pkg", "extensions"), "pmefVersion": V, "plantId": oid("plant", "P-1"),
     "projectCode": "CONF", "coordinateSystem": "Z-up", "units": "mm", "revisionId": "r2026-10-31-001",
     "changeState": "SHARED", "authoringTool": "PMEF conformance fixtures"},
    {"@type": "pmef:Plant", "@id": oid("plant", "P-1"), "pmefVersion": V, "name": "Conformance plant",
     "revision": REV},
    {"@type": "pmef:Unit", "@id": oid("unit", "U-1"), "pmefVersion": V, "name": "Unit 1", "isPartOf": oid("plant", "P-1"),
     "revision": REV},
    equip("pmef:GenericEquipment", "X-1", "GENERIC",
          customAttributes={"vendorCode": "ABC-123", "inspectionInterval_months": 24, "critical": True,
                            "legacyRef": None},
          **{"x-futureField": "unknown property, must be preserved (CL1-04)"}),
    {"@type": "pmef:FutureThing", "@id": oid("obj", "FUTURE-1"), "pmefVersion": V, "note": "unknown @type (CL1-03)"},
]


def build() -> dict[Path, str]:
    files: dict[Path, str] = {}
    tests = []
    for tid, name, level, domain, req, expect, objs in UNIT:
        rel_path = f"fixtures/unit/{tid}.ndjson"
        files[CONF / rel_path] = "".join(json.dumps(o, ensure_ascii=False, separators=(",", ":")) + "\n" for o in objs)
        tests.append({"id": tid, "name": name, "category": "negative" if expect == "invalid" else "unit",
                      "level": level, "domain": domain, "requirement": req, "fixture": rel_path, "expect": expect})
    for tid, name, level, req in INTG:
        tests.append({"id": tid, "name": name, "category": "integration", "level": level, "domain": "core",
                      "requirement": req, "dataset": "PMEF-DS-01"})
    for tid, name, level, req in RT:
        entry = {"id": tid, "name": name, "category": "roundtrip", "level": level, "domain": "core",
                 "requirement": req, "dataset": "PMEF-DS-01"}
        if tid in ("RT-016", "RT-017"):
            entry["dataset"] = "fixtures/roundtrip/extensions.ndjson"
        tests.append(entry)
    files[FIXTURES / "roundtrip" / "extensions.ndjson"] = "".join(
        json.dumps(o, ensure_ascii=False, separators=(",", ":")) + "\n" for o in EXTENSIONS)
    manifest = {
        "suiteVersion": "0.10.0",
        "description": "PMEF conformance test catalogue (spec 06 §5). Generated by tools/generate_fixtures.py.",
        "datasets": {"PMEF-DS-01": "examples/pump-skid-complete.ndjson"},
        "thresholds": {"2": 0.95, "3": 0.98},
        "tests": tests,
    }
    files[CONF / "manifest.json"] = json.dumps(manifest, indent=2, ensure_ascii=False) + "\n"
    return files


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="fail if generated files differ from the repository")
    args = ap.parse_args()
    files = build()
    stale = [p for p, c in files.items() if not p.exists() or p.read_text(encoding="utf-8") != c]
    if args.check:
        for p in stale:
            print(f"out of date: {p.relative_to(ROOT)}")
        unit_dir = FIXTURES / "unit"
        extra = [p for p in unit_dir.glob("*.ndjson") if p not in files] if unit_dir.exists() else []
        for p in extra:
            print(f"orphaned fixture: {p.relative_to(ROOT)}")
        return 1 if stale or extra else 0
    for p, c in files.items():
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(c, encoding="utf-8")
    print(f"wrote {len(files)} files ({sum(1 for t in json.loads(files[CONF / 'manifest.json'])['tests'])} tests)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
