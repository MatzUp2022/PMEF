# RFC-0008: `mesh_ref` glTF node fragments and mesh frame

| Field | Value |
|---|---|
| **Status** | `DRAFT` — reference implementation, schema, example and tests in this repository |
| **Discussion** | issue opened with the implementing pull request, 30-day period |
| **Target version** | 0.10.0 (additive: fragment syntax, one new optional field, `ref` format relaxed to `uri-reference`) |
| **Depends on** | spec 04 §2.1 (`GeometryReference`), §5 (glTF layer), DD-011 |
| **Working groups** | WG-Geometry, WG-Serialisation |
| **Date** | 2026-11-27 |

## Abstract

Models that exist only as plant design review files reach PMEF as glTF meshes converted by an
external tool (DD-011). One converted file holds hundreds of objects, so each PMEF object needs
to point at one node of the file. A test with an offshore E3D model wrote
`"ref": "HA-PIPE.glb#/nodes/34"`, but PMEF never defined the fragment, nor the units and axes
of the converted file. This RFC defines a fragment grammar with stable selectors, an optional
`meshFrame`, and a resolvability check in `pmef check`.

## Motivation

- **Consumers need one rule.** PlantView and cad-engine read `mesh_ref` geometry; without a
  defined fragment every consumer guesses.
- **Node indices are not stable.** A new export of the same model renumbers the nodes. The
  source's own identifier (E3D `RefNo`, kept by converters in `extras`) or the node name is.
- **Converted files differ from glTF defaults.** Converters for review formats commonly write
  metres, Z-up and a local origin; spec 04 §5.3 assumes Y-up with origin 0.

## Detailed Design

Normative text: spec 04 §2.3; schema `pmef-base.schema.json` (`GeometryReference`,
`MeshFrame`).

1. `ref = <file>[#<fragment>]`, `fragment = selector *(";" selector)`, selectors
   `/nodes/<i>`, `node=<name>`, `extras.<key>=<value>`, percent-encoded values.
2. Stable selectors (`node=`, `extras.`) must all match the same single node; the index is a
   hint, used alone only without stable selectors; a stale index gives a warning.
3. `meshFrame {unit: m|mm, upAxis: Y|Z, origin}`; absent = glTF convention (§5.3).
4. The schema checks the fragment syntax when `type` is `mesh_ref`; `ref` uses format
   `uri-reference` (package-relative paths are not absolute URIs).
5. `pmef check` gains `meshRefsResolve`: fragments whose file lies in the package must
   resolve; files outside the package and stale indices are reported as notes.

## Compatibility

Additive. Existing `mesh_ref` values without fragment (including the `urn:pmef:geom:…` style
of the examples) stay valid. `uri` → `uri-reference` only relaxes an annotation.

## Reference Implementation

- `pmef-io::mesh_ref` — `parse`, `resolve`, `gltf_json` (`.gltf` and `.glb`); unit tests.
- `pmef-core::geometry::MeshFrame` with `to_project`; serde round trip.
- `pmef check` (`meshRefsResolve`, `meshRefNotes`); CLI test with a temporary glTF file.
- Example `compressor-train.ndjson` (K-101 vendor mesh with node fragment and `meshFrame`);
  conformance tests `EQUIP-U-013/014`.

## Open Questions

- Should a FileHeader-level default `meshFrame` per file avoid repeating it on every object?
- Selecting a mesh primitive below a node (`/meshes/<m>/primitives/<p>`) is left out.
