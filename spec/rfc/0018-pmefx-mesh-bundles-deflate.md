# RFC-0018: PMEFX mesh bundles, compressed mesh entries and required features

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust writer/reader, CLI, example and tests in this repository |
| **Discussion** | issue opened with the implementing pull request, 30-day period |
| **Target version** | 0.10.0 (PMEFX 1.1: one optional manifest field, one optional index field, one relaxed rule) |
| **Depends on** | spec 03 §3.2–3.6, RFC-0005 (PMEFX object index), RFC-0017 (shared meshes) |
| **Working groups** | WG-Serialisation, WG-Geometry |
| **Date** | 2026-11-28 |

## Abstract

PMEFX 1.0 stores each mesh level as one glTF file and keeps it uncompressed. Plant models made
of many small parts then consist mostly of overhead: glTF headers, ZIP headers and one
`manifest.files` entry per file. This RFC adds three optional features:

- **mesh bundles**: `lods[].node` selects one root node of a glTF file, so one file holds the
  meshes of many objects;
- **compressed mesh entries**: glTF entries may be Deflate-compressed;
- **required features**: `manifest.requires` lists the features a package uses, so that
  readers which do not know them reject the package. Packages that list a feature carry
  `pmefxVersion` `"1.1"`.

The third feature answers the open question of RFC-0017 (`offset`, now feature `lodOffset`).

## Motivation

The PlantView converter packs the 69 RVM files of the Equinor Huldra open data set into PMEFX.
After shape sharing (RFC-0017) and dropping the per-object box files, the packages are
364.8 MB in total. A typical piping package (HA-PIPE) has 738 mesh files. Their median size is
2.5 KB, and the glTF JSON header is half of that.

- Deflate on the single files saves 55 %. On the same meshes concatenated, it saves 72 %:
  shared structure (JSON keys, materials, similar vertex data) compresses only across objects.
- Each file costs about 200 bytes of manifest entry (path, size, sha256) and about 200 bytes of
  ZIP headers (local header and central directory, both with the path).
- Viewers load the parts of one line or one region together anyway. One range read per
  bundle replaces hundreds of small reads.

## Detailed Design

Normative text: spec 03 §3.2 rule 4, §3.3 (`requires`), §3.5 (`node`), §3.6 (mesh bundles),
§3.9. Schema: `pmefx-manifest.schema.json` (`Manifest.requires`, `ObjectEntry.lods[].node`).

1. **`lods[].node`** (integer ≥ 0, optional): index of a root node of the default scene of
   `path`. The level is that node's subtree with the node's transform. Absent = the whole
   scene (1.0 rule). The node must be a root node, and its subtree must not be shared with
   another root node. Each root node should be named after the object's `@id` and carry it in
   `extras.pmefId`. `triangles` and `vertexCount` describe the subtree, `sizeBytes` the file.
2. **Deflate for glTF entries**: with feature `deflateMeshes`, `gltf` entries may use ZIP
   method 8. Readers inflate the whole entry; offset and compressed size come from the ZIP
   central directory, so HTTP range reads still fetch exactly one entry. `step` and `usd`
   stay stored.
3. **`manifest.requires`** (array of `lodOffset`, `lodNode`, `deflateMeshes`): writers list
   every feature they use; readers reject packages that list a feature they do not support.
   A package with a non-empty `requires` has `pmefxVersion` `"1.1"`. A package without
   optional features stays `"1.0"`, byte for byte as before.
4. Grouping is the writer's choice. Recommended: objects that are loaded together (one
   pipeline, one octree node, one discipline per area), bundles of at most a few MiB.

Why a node index and not a stable selector as in RFC-0008: package files are written together
with their index and never edited in place, so indices cannot go stale. The `@id` on the node
(`extras.pmefId`) still lets picking map back to the object.

Why Deflate and not a glTF mesh compression extension (Draco, `EXT_meshopt_compression`): ZIP
Deflate needs no new decoder (every ZIP reader has one, including the browser readers
of the ecosystem), it is lossless, and it also compresses the glTF JSON. A mesh compression
extension can be added later as another feature.

### Reference implementation

- `pmef-io::pmefx::glb::GlbBundle` (builds a bundle from self-contained glb files, keeps node
  transforms, stores identical materials once), `GlbFile` (parse once, read the whole scene or
  one root node), `LodRef::read_mesh` / `read_mesh_from`.
- `PmefxWriter::add_bundle`, `add_lod_ref` (with `node`), `set_deflate_meshes`,
  `generate_bbox_lods_bundled`; `finish` writes `requires` and the version.
- `PmefxReader::raw_range` (offset, compressed length, method) for HTTP range readers;
  `verify` checks the features, compressed entries and nodes.
- CLI: `pmef pack --bbox-lods --bundle N --deflate-meshes`.
- Example `examples/pmefx/pump-skid-bundled/`: the pump skid with bundled, compressed box
  levels. The archive is 10.1 KB instead of 39.1 KB.
- Tests: `bundled_and_deflated_meshes_round_trip`, `verify_enforces_required_features`,
  `bundled_bbox_lods_need_fewer_files`, `committed_bundled_example_matches_writer_and_schema`.

## Compatibility

Packages without optional features are unchanged (`"1.0"`, no `requires`). A 1.0 reader that
checks `pmefxVersion` sees `"1.1"`. A 1.0 reader that ignores the version and the unknown
fields would draw a whole bundle for each object, or fail to read a compressed entry. That is
why the version changes and `requires` exists.

RFC-0017 packages written before this RFC used `offset` without `requires`. `pmef verify` now
reports them. Writers in the ecosystem (PlantView converter) are updated together with this
RFC; such packages are regenerated.

## Open Questions

- Should a glTF mesh compression extension (`EXT_meshopt_compression`) become a fourth
  feature? It needs a decoder in every reader.
- Should the object index name the bundle groups (for example `group` per entry), so that
  viewers can schedule loads without reading the octree?
