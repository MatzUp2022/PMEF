#!/usr/bin/env python3
"""Generate examples/asbuilt-recognition.ndjson (RFC-0001/-0002 example, synthetic, CC0).

Catalog snaps of the pipes and the beam are computed with the pmef-snap/1 reference
implementation, so the example always matches the algorithm and the catalog versions.

    python scripts/generate-asbuilt-example.py examples/asbuilt-recognition.ndjson
"""
import importlib.util
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location("reference_snap", ROOT / "tests" / "snapping" / "reference_snap.py")
_snap = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_snap)


def snap(catalog_file, kind, measured):
    catalog = json.loads((ROOT / "catalogs" / catalog_file).read_text(encoding="utf-8"))
    result = _snap.snap(catalog, kind, measured)
    assert result["snapped"], f"example input does not snap: {measured}"
    return result["catalogSnap"]

P = "demo-asb"
V = "0.10.0"
def rev(state, at):  # RevisionMetadata
    return {"revisionId": "r2026-10-30-001", "changeState": state,
            "authoringTool": "PMEF Reference Generator 0.10", "changedAt": at}
SCAN = "file:scans/demo-hall-2.e57"
ISO = "urn:pmef:doc:demo-asb:ISO-CW-301"
RECOG = {"name": "demo-recognizer", "version": "1.4.0"}
def snap_pipe(od_meas, wt_meas):
    return snap("piping-class-en-p1a.json", "pipe", {"outsideDiameter": od_meas, "wallThickness": wt_meas})
def port(pid, xyz, d, to=None):
    p = {"portId": pid, "portType": "BIDIRECTIONAL", "coordinate": xyz, "direction": d,
         "nominalDiameter": 150, "endType": "BW"}
    if to: p["connectedTo"] = f"urn:pmef:obj:{P}:{to}"
    return p
objs = [
 {"@type": "pmef:FileHeader", "@id": f"urn:pmef:pkg:{P}:asbuilt-recognition", "pmefVersion": V,
  "plantId": f"urn:pmef:plant:{P}:DEMO-PLANT", "projectCode": "DEMO-ASB", "coordinateSystem": "Z-up",
  "units": "mm", "revisionId": "r2026-10-30-001", "changeState": "WIP",
  "authoringTool": "PMEF Reference Generator 0.10",
  "description": "RFC-0001 example: scanned DN150 pipe run and HEB 300 beam snapped to open catalogs (synthetic)"},
 {"@type": "pmef:Plant", "@id": f"urn:pmef:plant:{P}:DEMO-PLANT", "pmefVersion": V,
  "name": "Synthetic demo plant", "revision": rev("SHARED", "2026-10-30T08:00:00Z"),
  "provenance": {"sourceType": "manual", "createdAt": "2026-10-30T08:00:00Z"}},
 {"@type": "pmef:Unit", "@id": f"urn:pmef:unit:{P}:U-100", "pmefVersion": V, "name": "Hall 2 cooling water",
  "unitNumber": "U-100", "isPartOf": f"urn:pmef:plant:{P}:DEMO-PLANT",
  "revision": rev("SHARED", "2026-10-30T08:00:00Z")},
 {"@type": "pmef:PipingNetworkSystem", "@id": f"urn:pmef:line:{P}:CW-301", "pmefVersion": V,
  "lineNumber": "6\"-CW-301-ENP1A", "nominalDiameter": 150, "pipeClass": "EN-P1A", "mediumCode": "CW",
  "fluidPhase": "LIQUID", "isPartOf": f"urn:pmef:unit:{P}:U-100", "segments": [f"urn:pmef:seg:{P}:CW-301-S1"],
  "revision": rev("SHARED", "2026-10-30T14:00:00Z"),
  "provenance": {"sourceType": "iso-drawing", "sourceRef": f"{ISO}#page=1&region=0.05,0.08,0.62,0.71",
                 "recognizer": RECOG, "confidence": 0.97, "createdAt": "2026-10-30T09:00:00Z"},
  "review": {"status": "approved", "reviewedBy": "reviewer-01", "reviewedAt": "2026-10-30T14:00:00Z"}},
 {"@type": "pmef:PipingSegment", "@id": f"urn:pmef:seg:{P}:CW-301-S1", "isPartOf": f"urn:pmef:line:{P}:CW-301",
  "segmentNumber": 1,
  "components": [f"urn:pmef:obj:{P}:CW-301-PIPE-001", f"urn:pmef:obj:{P}:CW-301-ELB-001",
                 f"urn:pmef:obj:{P}:CW-301-PIPE-002"],
  "revision": rev("SHARED", "2026-10-30T14:00:00Z")},
 # approved, snapped pipe
 {"@type": "pmef:Pipe", "@id": f"urn:pmef:obj:{P}:CW-301-PIPE-001", "pmefVersion": V,
  "isPartOf": f"urn:pmef:seg:{P}:CW-301-S1", "itemNumber": "1",
  "componentSpec": {"componentClass": "PIPE", "skey": "PIPW"}, "pipeLength": 3000.0,
  "ports": [port("P1", [1000.0, 2000.0, 3500.0], [-1, 0, 0]),
            port("P2", [4000.0, 2000.0, 3500.0], [1, 0, 0], "CW-301-ELB-001")],
  "revision": rev("SHARED", "2026-10-30T14:00:00Z"),
  "provenance": {"sourceType": "scan", "sourceRef": f"{SCAN}#segment=CW-301-seg-01",
                 "recognizer": RECOG, "confidence": 0.94, "createdAt": "2026-10-30T09:12:00Z"},
  "review": {"status": "approved", "reviewedBy": "reviewer-01", "reviewedAt": "2026-10-30T14:00:00Z"},
  "asBuilt": {"pointCloudRef": f"{SCAN}#segment=CW-301-seg-01", "fitMethod": "ransac-cylinder",
              "deviationRms_mm": 1.2, "deviationMax_mm": 4.8, "catalogSnap": snap_pipe(169.1, 5.3)}},
 # detected only: must stay WIP
 {"@type": "pmef:Elbow", "@id": f"urn:pmef:obj:{P}:CW-301-ELB-001", "pmefVersion": V,
  "isPartOf": f"urn:pmef:seg:{P}:CW-301-S1", "itemNumber": "2",
  "componentSpec": {"componentClass": "ELBOW", "skey": "ELBW", "endType1": "BW", "endType2": "BW"},
  "angle": 90.0, "radius": "LONG_RADIUS",
  "ports": [port("P1", [4000.0, 2000.0, 3500.0], [-1, 0, 0], "CW-301-PIPE-001"),
            port("P2", [4228.6, 2000.0, 3271.4], [0, 0, -1], "CW-301-PIPE-002")],
  "revision": rev("WIP", "2026-10-30T09:12:00Z"),
  "provenance": {"sourceType": "scan", "sourceRef": f"{SCAN}#segment=CW-301-seg-02",
                 "recognizer": RECOG, "confidence": 0.71, "createdAt": "2026-10-30T09:12:00Z"},
  "review": {"status": "detected"},
  "asBuilt": {"pointCloudRef": f"{SCAN}#segment=CW-301-seg-02", "fitMethod": "least-squares-torus",
              "deviationRms_mm": 3.9, "deviationMax_mm": 11.5,
              "catalogSnap": {"catalogRef": {"catalogId": "piping-class-en-p1a", "catalogVersion": "2024.1",
                                             "entryId": "ENP1A-ELBOW90-150"},
                              "snappedSize": "DN 150 90° 1.5D",
                              "residuals": [{"parameter": "outsideDiameter", "measured_mm": 170.2,
                                             "catalog_mm": 168.3, "residual_mm": 1.9}]}}},
 # reviewed (corrected), still WIP
 {"@type": "pmef:Pipe", "@id": f"urn:pmef:obj:{P}:CW-301-PIPE-002", "pmefVersion": V,
  "isPartOf": f"urn:pmef:seg:{P}:CW-301-S1", "itemNumber": "3",
  "componentSpec": {"componentClass": "PIPE", "skey": "PIPW"}, "pipeLength": 2271.4,
  "ports": [port("P1", [4228.6, 2000.0, 3271.4], [0, 0, 1], "CW-301-ELB-001"),
            port("P2", [4228.6, 2000.0, 1000.0], [0, 0, -1])],
  "revision": rev("WIP", "2026-10-30T15:30:00Z"),
  "provenance": {"sourceType": "scan", "sourceRef": f"{SCAN}#segment=CW-301-seg-03",
                 "recognizer": RECOG, "confidence": 0.88, "createdAt": "2026-10-30T09:12:00Z"},
  "review": {"status": "reviewed", "reviewedBy": "reviewer-02", "reviewedAt": "2026-10-30T15:30:00Z"},
  "asBuilt": {"pointCloudRef": f"{SCAN}#segment=CW-301-seg-03", "fitMethod": "ransac-cylinder",
              "deviationRms_mm": 1.6, "deviationMax_mm": 5.9, "catalogSnap": snap_pipe(168.8, 4.7)}},
 # rejected false positive from a photo
 {"@type": "pmef:PipeSupport", "@id": f"urn:pmef:obj:{P}:CW-301-PS-901", "pmefVersion": V,
  "isPartOf": f"urn:pmef:seg:{P}:CW-301-S1", "itemNumber": "901",
  "componentSpec": {"componentClass": "PIPE_SUPPORT", "skey": "SUPR"},
  "supportsMark": "PS-901", "supportSpec": {"supportType": "RESTING", "attachmentType": "WELDED"},
  "revision": rev("WIP", "2026-10-30T15:35:00Z"),
  "provenance": {"sourceType": "photo", "sourceRef": "file:photos/IMG_0815.jpg#region=0.41,0.33,0.52,0.47",
                 "recognizer": RECOG, "confidence": 0.42, "createdAt": "2026-10-30T09:20:00Z"},
  "review": {"status": "rejected", "reviewedBy": "reviewer-02", "reviewedAt": "2026-10-30T15:35:00Z"}},
 # steel beam snapped to EN HEB 300
 {"@type": "pmef:SteelMember", "@id": f"urn:pmef:obj:{P}:B-101", "pmefVersion": V,
  "isPartOf": f"urn:pmef:unit:{P}:U-100", "memberMark": "B-101", "memberType": "BEAM",
  "profileId": "EN:HEB300", "startPoint": [0.0, 2600.0, 4200.0], "endPoint": [6000.0, 2600.0, 4200.0],
  "rollAngle": 0, "material": {"grade": "S355JR", "standard": "EN 10025-2", "fy": 355, "fu": 490},
  "revision": rev("PUBLISHED", "2026-10-30T16:00:00Z"),
  "provenance": {"sourceType": "scan", "sourceRef": f"{SCAN}#segment=beam-0042",
                 "recognizer": RECOG, "confidence": 0.91, "createdAt": "2026-10-30T09:15:00Z"},
  "review": {"status": "approved", "reviewedBy": "reviewer-01", "reviewedAt": "2026-10-30T16:00:00Z"},
  "asBuilt": {"pointCloudRef": f"{SCAN}#segment=beam-0042", "fitMethod": "ransac-plane",
              "deviationRms_mm": 1.8, "deviationMax_mm": 6.2,
              "catalogSnap": snap("profiles-en.json", "i-section", {"h": 301.4, "b": 299.1, "tw": 11.6, "tf": 18.7})}},
]
with open(sys.argv[1], "w") as f:
    f.write("// PMEF RFC-0001 example — provenance, review and as-built data (synthetic, CC0)\n")
    for o in objs:
        f.write(json.dumps(o, ensure_ascii=False, separators=(",", ":")) + "\n")
