# RFC-0017: PMEFX shared mesh files with a per-level offset

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust writer/reader and tests in this repository |
| **Discussion** | issue opened with the implementing pull request, 30-day period |
| **Target version** | 0.10.0 (one optional field in the object index) |
| **Depends on** | spec 03 §3.5–3.6, RFC-0005 (PMEFX object index) |
| **Working groups** | WG-Serialisation, WG-Geometry |
| **Date** | 2026-11-27 |

## Abstract

Plant models repeat the same shape many times: identical fittings, flanges, valves, supports
and duct parts. PMEFX 1.0 stores one mesh file per object and level, with vertices in final
project position, so every copy is stored again. This RFC adds an optional `offset` to each
entry of `lods`: the mesh's vertices are relative to `meshOrigin + offset`. Several objects may
then reference the same mesh file with different offsets.

## Motivation

- **Size.** In HVAC and piping models many parts are translated copies of another part. The
  converter in PlantView already writes these once in glTF (shape library). PMEFX could not,
  because a mesh file can only sit in one place.
- **Streaming.** A viewer loads a shared file once and draws it at each offset (instancing).

## Detailed Design

Normative text: spec 03 §3.5 and §3.6; schema `pmefx-manifest.schema.json` (`ObjectEntry.lods[].offset`).

1. `lods[].offset` (`{x, y, z}`, mm, optional): the mesh's vertices are relative to
   `manifest.meshOrigin + offset`. Absent means `0,0,0` (the 1.0 rule).
2. Several `lods` entries, of the same or of different objects, **MAY** reference the same
   archive path. The file is listed once in `manifest.files`.
3. Only translation. Rotated or mirrored copies keep their own file. This keeps the rule
   "vertices in final position up to a shift" and readers free of matrix handling.
4. `errorMm`, `triangles` and `vertexCount` describe the mesh as referenced. `sizeBytes`
   is the size of the shared file.

Reference implementation:

- `pmef-io::pmefx`: `LodRef::offset`, `LodRef::mesh_origin(package_origin)` (the origin to
  pass to `read_mesh_glb`), and `PmefxWriter::add_lod_at` / `reuse_lod`.
- Test: `shared_mesh_files_are_placed_by_offset`.

## Compatibility

Packages without `offset` are unchanged. A reader written for PMEFX 1.0 ignores the unknown
field and draws a shared mesh at its shape-local position, which is wrong. Writers therefore
**SHOULD** use `offset` only for consumers known to support this RFC. The ecosystem readers
(PlantView import, cad-engine `geom-pmefx`) are updated together with this RFC.

## Open Questions

- Should the manifest list required features (for example `"requires": ["lodOffset"]`) so
  that 1.0 readers reject such packages instead of misplacing meshes?
- Full transforms (rotation, mirroring) for mapped items, e.g. IFC `IfcMappedItem`.
