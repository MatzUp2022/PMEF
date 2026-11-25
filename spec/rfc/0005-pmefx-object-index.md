# RFC-0005: PMEFX 1.0 — object index, mesh levels of detail, exact B-rep

| Field | Value |
|---|---|
| **Status** | `DRAFT` — reference implementation, schema, example and tests in this repository |
| **Discussion** | [#15](https://github.com/MatzUp2022/PMEF/issues/15) — opened 2026-11-25, 30-day period until 2026-12-25 |
| **Target version** | 0.10.0 (container version `pmefxVersion: "1.0"`; additive, no change to NDJSON objects) |
| **Depends on** | spec 03 §3 (PMEFX 0.9), spec 02 §2.4 (`GeometryReference.lod`) |
| **Working groups** | WG-Geometry, WG-Serialisation |
| **Date** | 2026-11-25 |

## Abstract

PMEFX, the ZIP container of spec 03 §3, has been specified since 0.9 but had no reader or
writer and no way to say which geometry file belongs to which object. This RFC turns it into a
project container for large plants: an **object index** links every object's `@id` to its
NDJSON record, its bounding box, mesh levels of detail (glTF binaries, coarse → fine, with
error and size metrics) and its exact B-rep (STEP AP242). An optional loose **octree** lets
viewers stream only what is visible. The NDJSON data model is unchanged.

## Motivation

- **Three-level architecture.** Plant viewers display meshes, editors need exact B-rep only
  for the object being edited, and attributes live in PMEF. Without a per-object index these
  representations travel as separate, unlinked files.
- **Measured need.** Loading the "Clinic Plumbing" corpus model in cad-engine came close to the
  256 MB GPU limit (project test 2026-11-24). Levels of detail with error metrics plus a
  spatial index allow memory-budgeted streaming.
- **One identifier.** Proposals for CAD containers usually add a UUID per part. PMEF already
  has stable `@id`s that diff, merge and as-built correspondence rely on (spec 03 §9–§10,
  RFC-0003), so the index uses them as the only key.

## Detailed Design

Normative text: spec 03 §3.1–§3.9; schema `schemas/pmefx-manifest.schema.json`.

1. **Manifest** gains `pmefxVersion` (`"1.0"`), `objectIndex`, `objectCount`, `spatialIndex`,
   `meshOrigin`, `generator`, and per-file `sha256`. All existing 0.9 fields keep their meaning.
2. **Object index** `index/objects.ndjson`: one line per object with `id`, `type`,
   `record {path, line}`, `bbox`, `lods[]` (`lod`, `path`, `triangles`, `vertexCount`,
   `errorMm`, `sizeBytes`; ordered coarse → fine) and `brep {path, format}`.
3. **Mesh assets** are glTF 2.0 binaries in glTF space (Y-up, metres) relative to
   `meshOrigin`; the axis mapping is fixed so all producers agree.
4. **Spatial index** `index/octree.json`: loose octree, every object with a box in exactly one
   node, node boxes enclose their subtree.
5. **Archive rules**: listed files only, safe relative paths, ZIP64 when needed, geometry
   entries stored uncompressed for range reads, reproducible output recommended.
6. **ZIP, not HDF5**, with the criteria for revisiting the decision (spec 03 §3.8).

The level names reuse the existing `GeometryReference.lod` enum. `errorMm`, `vertexCount` and
the loose-octree rules were requested by the cad-engine LOD/streaming work, which will consume
the format.

## Compatibility

- NDJSON objects and their schemas are unchanged.
- 0.9 manifests remain valid against the new schema (`pmefxVersion` and the index are optional
  in the schema; spec text makes them mandatory for 1.0 writers). Readers treat a missing
  `pmefxVersion` as a 0.9 package without object index.
- Unknown future minor container versions (`1.x`) are readable by 1.0 readers.

## Reference Implementation

- `pmef-rs/pmef-io/src/pmefx/` (feature `pmefx`, on by default): `PmefxWriter` (streaming,
  deterministic), `PmefxReader` (manifest, index, octree, record lookup, byte ranges of stored
  entries, `verify`), `glb::box_glb` (`BBOX_ONLY` meshes from bounding boxes), loose octree.
- CLI: `pmef pack`, `pmef unpack`, `pmef verify`.
- Tests `pmef-rs/pmef-io/tests/pmefx.rs`: round trip, determinism, schema conformance of
  manifest/index/octree, committed example, tamper detection, octree invariants, mesh winding.
  The generated `.glb` files pass the Khronos glTF validator without errors or warnings.
- Example: `examples/pmefx/pump-skid-complete/`.

## Out of Scope / Follow-ups

- Producing real tessellated levels (`LOD1`…`LOD4`) is the job of cad-kernel (tessellation)
  and cad-engine/PlantView (simplification); this RFC defines where they go and how they are
  described. A PMEFX reader as a geometry source in cad-engine is planned there.
- A native exact-geometry format of cad-kernel can be added as `brep.format = "x-cad-kernel"`
  and standardised later.
- Package signing, delta containers and an HDF5/chunked variant.

## Review Status

- 2026-11-25: draft with implementation; RFC issue #15 opened; awaiting WG review and TSC sign-off.
