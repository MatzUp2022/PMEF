# PMEF Specification · Chapter 03 · Serialisation

**Document number:** PMEF-SPEC-03  
**Version:** 0.9.0  
**Status:** Normative  
**Date:** 2026-03-31

---

## Table of Contents

1. [General](#1-general)
2. [NDJSON Primary Serialisation](#2-ndjson-primary-serialisation)
3. [PMEFX Container Format](#3-pmefx-container-format)
4. [CAEX XML Secondary Serialisation](#4-caex-xml-secondary-serialisation)
5. [File Naming Conventions](#5-file-naming-conventions)
6. [Encoding and Character Set](#6-encoding-and-character-set)
7. [Version Declaration](#7-version-declaration)
8. [Large Model Handling](#8-large-model-handling)
9. [Object-Level Comparison](#9-object-level-comparison)
10. [Three-Way Merge](#10-three-way-merge)

---

## 1 General

PMEF defines two serialisation formats:

1. **NDJSON** (Newline-Delimited JSON) — the **primary** format, mandatory for all PMEF implementations.
2. **CAEX XML** — a **secondary** format, aligned with AutomationML, optional.

A PMEF implementation that claims Level 1 or higher conformance **MUST** support reading and writing NDJSON.

Support for CAEX XML is **RECOMMENDED** for implementations
targeting E&I interoperability and is **REQUIRED** for MTP 2.0
integration.

### 1.1 Design Rationale for NDJSON

NDJSON was chosen as the primary format for the following reasons:

- **Git-friendliness.** One object per line means that adding,
  modifying, or deleting one object produces a single changed
  line in a `git diff`, enabling meaningful version control of
  plant models.
- **Streamability.** Large models (millions of objects) can be
  read and written without loading the full document into memory.
- **No special parser.** Any JSON library plus a line-split suffices for reading.
- **Human-readable.** Plant engineers can inspect objects directly, without requiring a specialised viewer.
- **Deterministic serialisation.** PMEF mandates alphabetical key
  ordering and canonical number formatting (see §2.5), ensuring
  that semantically identical objects produce byte-identical
  serialisations.

---

## 2 NDJSON Primary Serialisation

### 2.1 File Structure

A PMEF NDJSON file **MUST** conform to the following structure:

1. **Line 1:** `pmef:FileHeader` object — exactly one, **MUST** be first.
2. **Lines 2–N:** One PMEF object per line, in any order (except where ordering constraints apply; see §2.4).
3. **Line N+1:** A final newline character terminating the file.

A PMEF NDJSON file **MUST NOT** be empty.

**Permitted non-object lines (for annotated example files only):**

- Lines beginning with `//` are comment lines. Comment lines
  **MUST NOT** appear in production PMEF files. They are
  permitted only in files in the `examples/` directory.
- Empty lines are permitted.

Example structure:

```text
{"@type":"pmef:FileHeader","@id":"urn:pmef:pkg:proj:ds01",...}\n
{"@type":"pmef:Plant","@id":"urn:pmef:plant:proj:EAF",...}\n
{"@type":"pmef:Unit","@id":"urn:pmef:unit:proj:U-100",...}\n
{"@type":"pmef:Pump","@id":"urn:pmef:obj:proj:P-201A",...}\n
...
```

### 2.2 One Object Per Line

Each non-comment, non-empty line in a PMEF NDJSON file **MUST**
contain exactly one complete, self-contained JSON object.

- Multi-line pretty-printed objects are **NOT** permitted in PMEF NDJSON files.
- A line **MUST NOT** contain more than one JSON object.
- A line **MUST NOT** contain a JSON array, string, or number as its root element.

### 2.3 Required Object Fields

Every line-level PMEF object **MUST** contain:

- `"@type"` — the entity type string.
- `"@id"` — the unique object identifier.

### 2.4 Recommended Object Ordering

While PMEF does not mandate a specific ordering of objects within
an NDJSON file (except that `pmef:FileHeader` is first), the
following ordering is **RECOMMENDED** to improve human readability
and streaming processing:

1. `pmef:FileHeader`
2. `pmef:Plant`
3. `pmef:Unit` / `pmef:Area` objects
4. `pmef:ParametricGeometry` objects referenced by equipment
5. Equipment objects (`pmef:Pump`, `pmef:Vessel`, etc.)
6. `pmef:PipingNetworkSystem` objects
7. `pmef:PipingSegment` objects
8. Piping component objects, in routing order within each segment
9. `pmef:Spool` objects
10. E&I objects (`pmef:InstrumentObject`, `pmef:PLCObject`, etc.)
11. Structural Steel objects
12. Relationship objects

### 2.5 Canonical Serialisation

For deterministic serialisation (required for checksum computation
and meaningful diffs), PMEF objects **SHOULD** be serialised with
the following rules:

1. **Key ordering:** JSON object keys **MUST** be sorted
   alphabetically (Unicode code point order) at all levels of
   nesting.
2. **Number formatting:** Floating-point numbers **MUST** be
   serialised with sufficient precision to preserve the original
   value (use Python `repr()` or equivalent). Trailing zeros
   after the decimal point **SHOULD** be omitted.
3. **String escaping:** Use UTF-8 with minimal escaping (only escape characters that RFC 8259 requires to be escaped).
4. **No trailing whitespace:** Lines **MUST NOT** contain trailing whitespace before the newline.
5. **Newline character:** Lines **MUST** be terminated by `\n` (LF). `\r\n` (CRLF) is not permitted.

### 2.6 Validation Requirement

Every PMEF object in an NDJSON file **MUST** validate against
the JSON Schema corresponding to its `@type`. Implementations
writing PMEF **MUST** validate objects before writing.
Implementations reading PMEF **SHOULD** validate each object as
it is read and report validation errors without halting
processing.

### 2.7 Reference Resolution

References between PMEF objects are expressed via `@id` values. The following rules apply:

- **Within-package references** (e.g. `isPartOf`,
  `Port.connectedTo`) **MUST** be resolvable within the same
  NDJSON file or PMEFX container.
- **Cross-package references** use the `urn:pmef:external:<package-id>:<object-id>` URI prefix.
- A reader that encounters an unresolvable reference **MUST NOT**
  fail. It **SHOULD** report the unresolvable reference as a
  warning.

### 2.8 Example: Minimal Valid PMEF NDJSON File

```jsonc
{"@type":"pmef:FileHeader","@id":"urn:pmef:pkg:proj:minimal","pmefVersion":"0.9.0","plantId":"urn:pmef:plant:proj:MY-PLANT","projectCode":"MINIMAL","coordinateSystem":"Z-up","units":"mm","changeState":"WIP"}
{"@type":"pmef:Plant","@id":"urn:pmef:plant:proj:MY-PLANT","pmefVersion":"0.9.0","name":"My Plant","revision":{"revisionId":"r2026-01-01-001","changeState":"WIP"}}
{"@type":"pmef:Unit","@id":"urn:pmef:unit:proj:U-100","pmefVersion":"0.9.0","isPartOf":"urn:pmef:plant:proj:MY-PLANT","unitNumber":"U-100","unitName":"Process Unit","revision":{"revisionId":"r2026-01-01-001","changeState":"WIP"}}
```

---

## 3 PMEFX Container Format

### 3.1 Overview

A PMEFX file is a ZIP archive that bundles one or more PMEF NDJSON files with the geometry
and documents that belong to them. It uses the file extension `.pmefx` and the media type
`application/vnd.pmef.pmefx+zip`.

PMEFX is a *package around* PMEF, not a second data model: attributes, topology and catalog
references live only in the NDJSON records. Version 1.0 of the container
([RFC-0005](rfc/0005-pmefx-object-index.md)) adds an **object index** that links every object,
by its stable `@id`, to three representations:

| Representation | Purpose | Format |
|---|---|---|
| NDJSON record | attributes, topology, catalog snap | PMEF NDJSON (this chapter §2) |
| Mesh levels of detail | fast display, streaming | glTF 2.0 binary (`.glb`), coarse → fine |
| Exact B-rep | editing, fabrication, clash with tolerances | STEP AP242 (ISO 10303-242) |

A viewer reads `manifest.json` and the object index, decides from bounding boxes and the
optional spatial index (§3.7) what is visible, and then range-reads only the meshes it needs.
B-reps are read only when an object is opened for editing. The `@id` is the link key: PMEFX
introduces no second identifier (no extra UUID), so diffs, merges and as-built correspondences
(§9, §10, RFC-0003) keep working on packaged data.

The schema `schemas/pmefx-manifest.schema.json` is normative together with this section. Its
root validates `manifest.json`, `$defs/ObjectEntry` validates each object index line and
`$defs/Octree` validates the spatial index.

### 3.2 Archive Structure

```text
my-plant.pmefx  (ZIP archive)
├── manifest.json           — package manifest (REQUIRED)
├── model/
│   ├── main.ndjson         — primary PMEF NDJSON file (REQUIRED)
│   ├── ei.ndjson           — E&I objects (optional split)
│   └── steel.ndjson        — structural objects (optional split)
├── index/
│   ├── objects.ndjson      — object index (REQUIRED in 1.0)
│   └── octree.json         — spatial index (optional)
├── meshes/
│   ├── obj_proj_P-201A.bbox.glb    — BBOX_ONLY level
│   ├── obj_proj_P-201A.lod1.glb    — LOD1_COARSE level
│   └── obj_proj_P-201A.lod3.glb    — LOD3_FINE level
├── brep/
│   └── obj_proj_P-201A.stp — exact geometry, STEP AP242
├── geometry/
│   ├── model.glb           — whole-model glTF 2.0 (optional)
│   ├── model.usdc          — OpenUSD geometry (optional)
│   └── model.stp           — whole-model STEP AP242 (optional)
├── catalogs/
│   ├── piping-class-a1a2.json
│   └── profiles-en.json
└── documents/
    ├── DS-P-201A-Rev3.pdf
    └── ISO-CW-201-Rev2.pdf
```

Only `manifest.json` and the files it names are fixed; the directory names above are the
recommended layout. Rules:

1. The archive **MUST** be a ZIP file (APPNOTE 6.3.x). ZIP64 extensions **MUST** be used for
   entries or archives larger than 4 GiB. Encryption and multi-volume archives **MUST NOT** be
   used.
2. Entry paths **MUST** be relative, use `/` as separator and **MUST NOT** contain empty, `.`
   or `..` segments, backslashes or colons. Readers **MUST** reject paths that escape the
   extraction directory.
3. Every entry other than `manifest.json` **MUST** be listed in `manifest.files`, and every
   listed file **MUST** exist.
4. Entries of type `gltf`, `step` and `usd` **MUST** be stored without compression (ZIP method
   0), so that readers can range-read them over HTTP or memory-map them. Other entries
   **SHOULD** use Deflate.
5. Writers **SHOULD** produce reproducible archives: entries in a deterministic order and a
   fixed entry timestamp, so identical content yields identical bytes.

### 3.3 Manifest

The `manifest.json` file **MUST** be present at the root of the archive.

| Field | Required | Description |
|-------|----------|-------------|
| `pmefxVersion` | **MUST** (1.0) | Container version, `"1.0"`. Absent in 0.9 manifests; readers treat its absence as a 0.9 package without object index. |
| `pmefVersion` | **MUST** | PMEF data version of the NDJSON files |
| `packageId` | **MUST** | `@id` of the main file's `pmef:FileHeader` |
| `mainFile` | **MUST** | Relative path to the primary NDJSON file |
| `files` | **MUST** | All archive entries except `manifest.json`: `path`, `type` (`ndjson`, `gltf`, `step`, `usd`, `catalog`, `document`, `index`, `other`), `sizeBytes`, `sha256`, and `objects` for NDJSON files |
| `createdAt` | **MUST** | ISO 8601 timestamp |
| `coordinateSystem` | **MUST** | `"Z-up"` or `"Y-up"` |
| `objectIndex` | **MUST** (1.0) | Path of the object index (§3.5), recommended `index/objects.ndjson` |
| `objectCount` | SHOULD | Number of object index lines |
| `spatialIndex` | MAY | Path of the spatial index (§3.7), recommended `index/octree.json` |
| `meshOrigin` | MAY | Origin of all mesh assets in project coordinates, `{x, y, z}` [mm] (§3.6). Default `0,0,0`. |
| `generator` | MAY | `{name, version}` of the writing software |

`sha256` is the lower-case hexadecimal SHA-256 of the uncompressed entry; writers **SHOULD**
provide it for every file and readers **MUST** report a mismatch.

Example manifest:

```json
{
  "pmefxVersion": "1.0",
  "pmefVersion": "0.9.0",
  "packageId": "urn:pmef:pkg:proj:my-plant",
  "mainFile": "model/main.ndjson",
  "coordinateSystem": "Z-up",
  "createdAt": "2026-03-31T10:00:00Z",
  "files": [
    {"path": "model/main.ndjson", "type": "ndjson", "objects": 1250, "sizeBytes": 812400, "sha256": "…"},
    {"path": "meshes/obj_proj_P-201A.lod1.glb", "type": "gltf", "sizeBytes": 18320, "sha256": "…"},
    {"path": "brep/obj_proj_P-201A.stp", "type": "step", "sizeBytes": 402112, "sha256": "…"},
    {"path": "index/objects.ndjson", "type": "index", "sizeBytes": 301877, "sha256": "…"},
    {"path": "index/octree.json", "type": "index", "sizeBytes": 40210, "sha256": "…"}
  ],
  "meshOrigin": {"x": 125000.0, "y": 48000.0, "z": 0.0},
  "objectIndex": "index/objects.ndjson",
  "objectCount": 1250,
  "spatialIndex": "index/octree.json",
  "generator": {"name": "pmef-io", "version": "0.9.0"}
}
```

A complete manifest and index generated from `examples/pump-skid-complete.ndjson` are in
`examples/pmefx/pump-skid-complete/`.

### 3.4 Split File Packages

Large plant models **MAY** be split across multiple NDJSON files
within a PMEFX archive. The rules for split packages are:

1. Each NDJSON file **MUST** have a valid `pmef:FileHeader` as its first line.
2. All `pmef:FileHeader` objects in a split package **MUST** reference the same `plantId`.
3. Cross-file `isPartOf` and other references **MUST** use the full `urn:pmef:` URI (not a relative path).
4. The manifest **MUST** list all NDJSON files.
5. An `@id` **MUST** be unique across all NDJSON files of the package.

### 3.5 Object Index

The object index is an NDJSON file with one entry per PMEF object of the package (including
the `pmef:FileHeader` and hierarchy objects), sorted by `id`.

| Field | Required | Description |
|-------|----------|-------------|
| `id` | **MUST** | The object's `@id` |
| `type` | **MUST** | The object's `@type` |
| `record` | **MUST** | `{path, line}`: NDJSON file and 1-based physical line that holds the object |
| `bbox` | SHOULD | Axis-aligned bounding box in project coordinates [mm] (`xMin` … `zMax`) |
| `lods` | MAY | Mesh levels of detail (below) |
| `brep` | MAY | `{path, format}`: exact geometry; `format` is `step` (AP242) or a vendor extension `x-<name>` |

`bbox` **SHOULD** be the object's `geometry.boundingBox`. Where that is absent, writers
**MAY** derive it from the object's anchor points (`coordinate`, `center`, `startPoint`,
`endPoint`), grown by half the largest `nominalDiameter`/`outsideDiameter`; objects without a
box with extent carry no `bbox`.

Each entry of `lods`:

| Field | Required | Description |
|-------|----------|-------------|
| `lod` | **MUST** | `BBOX_ONLY`, `LOD1_COARSE`, `LOD2_MEDIUM`, `LOD3_FINE`, `LOD4_FABRICATION` (spec 02 §2.4) |
| `path` | **MUST** | Archive path of a glTF 2.0 binary |
| `triangles` | SHOULD | Triangle count |
| `vertexCount` | SHOULD | Vertex count — lets viewers check GPU budgets before loading |
| `errorMm` | SHOULD | Largest geometric deviation from the finest available representation [mm] (a Hausdorff-distance estimate is sufficient); drives screen-space-error LOD selection |
| `sizeBytes` | MAY | Size of the `.glb` |

`lods` **MUST** be ordered coarse to fine, and each level **MUST** occur at most once per
object. Readers that find no `errorMm` **SHOULD** fall back to the projected size of the
object's bounding sphere.

Example line:

```json
{"id":"urn:pmef:obj:proj:P-201A","type":"pmef:Pump","record":{"path":"model/main.ndjson","line":17},"bbox":{"xMin":124100.0,"xMax":125900.0,"yMin":47600.0,"yMax":48400.0,"zMin":0.0,"zMax":1250.0},"lods":[{"lod":"BBOX_ONLY","path":"meshes/obj_proj_P-201A.bbox.glb","triangles":12,"vertexCount":24,"sizeBytes":1704},{"lod":"LOD1_COARSE","path":"meshes/obj_proj_P-201A.lod1.glb","triangles":420,"vertexCount":260,"errorMm":35.0,"sizeBytes":18320},{"lod":"LOD3_FINE","path":"meshes/obj_proj_P-201A.lod3.glb","triangles":21800,"vertexCount":11400,"errorMm":0.5,"sizeBytes":702044}],"brep":{"path":"brep/obj_proj_P-201A.stp","format":"step"}}
```

### 3.6 Mesh Assets

Mesh assets are self-contained glTF 2.0 binaries (one `.glb` per object and level). glTF is
Y-up and metre-based; PMEF project coordinates are Z-up millimetres (spec 02 §2.7). A project
point `p` maps to glTF coordinates relative to `o = manifest.meshOrigin` as

```text
gltf = ( (p.x − o.x) / 1000,  (p.z − o.z) / 1000,  −(p.y − o.y) / 1000 )
```

This is a proper rotation plus scaling, so triangle winding is preserved. `meshOrigin` keeps
vertex values small enough for 32-bit floats in plants with large site coordinates; writers
**SHOULD** set it near the centre of the package extent. Mesh vertices are in final project
position (no per-object transform is applied by the reader). A mesh **SHOULD** name its root
node after the object's `@id` and carry it in `extras.pmefId`, so that picking can be mapped
back to the object.

Writers **MAY** store vertex data compactly with the glTF extension `KHR_mesh_quantization`
(normalised integer positions dequantised by the node transform, integer normals) and list it
in `extensionsRequired`; readers **MUST** support it. Other required extensions **MUST NOT** be
used. The position error introduced by quantisation counts towards the level's `errorMm`.

### 3.7 Spatial Index

The optional spatial index is a loose octree over the object bounding boxes:

```json
{"maxDepth": 8, "maxObjectsPerNode": 64,
 "root": {"bbox": {…}, "objects": ["urn:pmef:…"], "children": [{"bbox": {…}, "objects": […], "children": […]}]}}
```

1. Cells subdivide regularly into octants, starting from the union of all object boxes. An
   object is stored in the deepest node whose cell fully contains its box, so it **MUST**
   appear in exactly one node.
2. A node's `bbox` is *loose*: the union of all object boxes in its subtree. It **MUST**
   enclose its objects and its children's boxes, and may extend beyond the node's cell.
3. Every object index entry with a `bbox` **MUST** appear in the octree; entries without
   `bbox` **MUST NOT**.

Viewers cull against the loose boxes and load a node's meshes only when it is visible.

### 3.8 Choice of ZIP (and not HDF5)

ZIP with stored mesh entries already gives random access (central directory, byte offsets,
HTTP range requests), streaming writes and ZIP64 for archives beyond 4 GiB, and it can be
inspected with standard tools. HDF5 is not used in 1.0: it adds a large native dependency,
is hard to read in browsers (WebAssembly) and offers no advantage for per-object binary blobs.
A future version may add an HDF5 or chunked-store variant if measured packages show that the
ZIP central directory or the object index becomes the bottleneck (order of 10⁷ objects);
such a variant would keep the manifest and index model unchanged.

### 3.9 Conformance

A conformant writer produces archives that satisfy §3.2–§3.7. A conformant reader checks at
least: manifest schema, the listing rule (§3.2 rule 3), sizes and `sha256`, stored geometry
entries, the `FileHeader` first line of each NDJSON file, that each `record` points to the
object with that `@id`, LOD ordering, and the octree rules. The reference implementation is
`pmef_io::pmefx` with the CLI commands `pmef pack`, `pmef unpack` and `pmef verify`.

---

## 4 CAEX XML Secondary Serialisation

### 4.1 Overview

PMEF provides a CAEX XML secondary serialisation based on
AutomationML 2.10 (IEC 62714). The CAEX serialisation is
primarily intended for E&I data exchange and MTP integration.

The CAEX serialisation is **OPTIONAL** for Level 1 and Level 2
implementations. It is **RECOMMENDED** for Level 3
implementations targeting E&I tools (COMOS, EPLAN, TIA Portal).

### 4.2 CAEX Document Structure

A PMEF CAEX document follows the AutomationML hierarchy:

```xml
<CAEXFile SchemaVersion="3.0" ...>
  <AdditionalInformation>
    <PMEFHeader pmefVersion="0.9.0" plantId="..." />
  </AdditionalInformation>

  <!-- System Unit class libraries (plant hierarchy) -->
  <SystemUnitClassLib Name="PMEF_SystemUnitClasses">
    <!-- PMEF entity types as SystemUnitClasses -->
  </SystemUnitClassLib>

  <!-- Instance hierarchy (plant objects) -->
  <InstanceHierarchy Name="PlantModel">
    <InternalElement Name="U-100" ID="..." RefBaseSystemUnitPath="PMEF_SystemUnitClasses/Unit">
      <!-- Equipment -->
      <InternalElement Name="P-201A" ID="..." RefBaseSystemUnitPath="PMEF_SystemUnitClasses/Pump">
        <!-- Attributes (property sets) -->
        <Attribute Name="tagNumber" AttributeDataType="xs:string">
          <Value>P-201A</Value>
        </Attribute>
        <Attribute Name="designFlow_m3h" AttributeDataType="xs:double">
          <Value>450.0</Value>
        </Attribute>
        <!-- PMEF ID stored as ExternalInterface -->
        <ExternalInterface Name="pmefId" RefBaseClassPath="PMEF_InterfaceClasses/PmefObject">
          <Attribute Name="id" AttributeDataType="xs:anyURI">
            <Value>urn:pmef:obj:proj:P-201A</Value>
          </Attribute>
        </ExternalInterface>
      </InternalElement>
    </InternalElement>
  </InstanceHierarchy>
</CAEXFile>
```

### 4.3 Mapping Rules

The following mapping rules apply when converting between PMEF NDJSON and CAEX XML:

| PMEF | CAEX |
|------|------|
| `@id` | `InternalElement/@ID` (GUID) + `ExternalInterface[pmefId]/Attribute[id]` |
| `@type` | `InternalElement/@RefBaseSystemUnitPath` |
| Scalar property | `Attribute[@Name=fieldName, @AttributeDataType=...]/<Value>` |
| Object property | Nested `InternalElement` |
| `isPartOf` | Parent-child `InternalElement` nesting |
| Relationship | `InternalLink` between `ExternalInterface` elements |

### 4.4 Unit Serialisation in CAEX

In CAEX, attribute names **MUST** include the unit suffix when the unit is not self-evident:

- `"designPressure_Pa"` for pressure in Pascal
- `"designTemperature_K"` for temperature in Kelvin
- `"nominalDiameter_mm"` for diameters in millimetres
- `"designFlow_m3h"` for flow in m³/h

---

## 5 File Naming Conventions

### 5.1 NDJSON Files

PMEF NDJSON files **SHOULD** use the file extension `.ndjson`.

Recommended naming patterns:

| Pattern | Example | Use |
|---------|---------|-----|
| `<project>-pmef-<date>.ndjson` | `EAF2026-pmef-20260331.ndjson` | Timestamped deliverable |
| `<project>-<domain>.ndjson` | `EAF2026-piping.ndjson` | Split by domain |
| `pmef-ds-NN.ndjson` | `pmef-ds-01.ndjson` | Benchmark dataset |

### 5.2 PMEFX Archives

PMEFX archives **MUST** use the file extension `.pmefx`.

### 5.3 Schema Files

JSON Schema files follow the naming pattern: `pmef-<domain>.schema.json`.

---

## 6 Encoding and Character Set

- PMEF NDJSON files **MUST** be encoded in **UTF-8** without BOM.
- PMEF NDJSON files **MUST** use **LF** (`\n`, 0x0A) as the line terminator. CRLF is not permitted.
- Tag numbers and attribute values **MAY** contain characters
  outside the ASCII range (e.g. German umlauts in equipment
  descriptions). These **MUST** be encoded as UTF-8, not as JSON
  Unicode escape sequences (`\uXXXX`), unless required by JSON
  encoding rules.
- File paths within PMEFX manifests **MUST** use forward slashes and **MUST NOT** contain non-ASCII characters.

---

## 7 Version Declaration

### 7.1 Object-Level Version

Every PMEF physical and functional object **SHOULD** carry a
`pmefVersion` field specifying the PMEF version against which it
was serialised. The value **MUST** follow Semantic Versioning
(`MAJOR.MINOR.PATCH`).

### 7.2 Package-Level Version

The `pmef:FileHeader` object **MUST** carry the `pmefVersion` field.

### 7.3 Version Compatibility

- A reader conformant to PMEF version `M.N.x` **MUST** be able
  to read any PMEF file with version `M.N'.x'` where `N' ≤ N`.
- A reader **MUST NOT** fail on encountering unknown optional fields (introduced in later minor versions).
- A reader **MUST** fail or report an error on encountering a PMEF file with a higher MAJOR version.

---

## 8 Large Model Handling

### 8.1 Streaming Reading

Implementations **SHOULD** read PMEF NDJSON files as a stream,
processing one object per line, without loading the entire file
into memory. This enables processing of plant models with
millions of objects.

### 8.2 Line Length

PMEF does not impose a maximum line length. However, objects with
large embedded geometry arrays (e.g. mesh coordinates) **SHOULD**
use external geometry references (`MeshRef`, `step_ref`,
`usd_ref`) rather than inlining geometry data.

A line exceeding 1 MB (1,048,576 bytes) **SHOULD** be split by externalising its geometry or large arrays.

### 8.3 Performance Guidelines

For models with more than 100,000 objects, the following practices are **RECOMMENDED**:

- Split the model into domain-specific NDJSON files within a PMEFX container.
- Use a single `pmef:PipingNetworkSystem` object per line; do not embed its segments inline.
- Reference geometry objects by URI (`GeometryReference.ref`) rather than inlining them.
- Index objects by `@id` using a hash map for O(1) reference resolution.

### 8.4 Incremental Updates (Delta Packages)

For large plant models that change incrementally, PMEF supports delta packages:

- A delta package is a valid PMEF NDJSON file containing only the changed and new objects.
- Every changed object **MUST** carry a `pmef:IsRevisionOf` relationship referencing the replaced object's `@id`.
- An `IsRevisionOf` with `changeType` `AS_BUILT` is an as-built correspondence (RFC-0003), not a content
  revision; it never marks an object of a delta package as replaced.
- Deleted objects are represented by a PMEF object with `changeState: "ARCHIVED"` and no other content changes.
- Delta packages **MAY** reference objects defined in the base package using their full `urn:pmef:` URIs.

---

## 9 Object-Level Comparison

*Since 0.10.0.* Comparing two PMEF packages — design vs. as-built, or two revisions — is a core
use case. A tool that reports differences between PMEF packages (e.g. `pmef diff`) **MUST**
follow the rules below, so that every tool reports the same differences for the same files.

### 9.1 Matching

1. Objects are matched by `@id` only. An object present in one package only is *added* (head)
   or *removed* (base); an object present in both is *changed* if at least one field differs,
   otherwise *unchanged*.
2. A package containing the same `@id` twice is invalid input and **MUST** be reported as an
   error, not compared.
3. Comment lines (§2.1) and object order are irrelevant.

### 9.2 Field comparison

1. Fields are compared recursively. A field present on one side only is *added* or *removed*.
2. **Keyed arrays:** if every element of both arrays is an object carrying the same key field,
   with unique values per array, elements are matched by that key, not by position. Key fields,
   in this order of precedence: `@id`, `portId`, `nozzleId`, `parameter`, `partName`,
   `connectionId`. All other arrays are compared by position.
3. **Numbers** are equal if they differ by at most the tolerance of chapter 06 §6.2, chosen by
   the field name (pressure 1 Pa, temperature 0.01 K, flow 0.001, angle/direction/axis 0.001,
   otherwise 0.01 — lengths in mm). Tools **MUST** offer an exact mode without tolerances.
4. All other values are equal if they are identical JSON values (object key order is irrelevant).
5. Tools **MAY** let the user ignore field paths; an ignored path also hides everything below it.

### 9.3 Paths and ordering

A field path joins object keys with `.`, positional array elements as `[i]` (0-based) and keyed
array elements as `[key=value]`, e.g. `ports[portId=P2].coordinate[2]`. Results are ordered by
`@id` (Unicode code point order), then by path in traversal order with object keys sorted. Values
in a report are written with object keys sorted (§2.5).

### 9.4 Report

The machine-readable report is a JSON object:

```json
{
  "summary": {"added": 1, "removed": 1, "changed": 2, "unchanged": 7},
  "objects": [
    {"@id": "urn:pmef:obj:demo-asb:B-101", "@type": "pmef:SteelMember", "status": "changed",
     "changes": [{"path": "endPoint[0]", "kind": "modified", "old": 6025.0, "new": 6000.0, "delta": -25.0}]}
  ]
}
```

`kind` is `added`, `removed` or `modified`; `old` is absent for `added`, `new` for `removed`;
`delta` (`new − old`, rounded to 6 decimals) is present for modified numbers. Unchanged objects
are counted, not listed. A command-line tool **SHOULD** exit with 0 (no differences),
1 (differences) or 2 (error).

Reference implementation: `pmef_core::diff` and `pmef diff` (`--exact`, `--ignore <path>`,
`--ignore-recognition` for the RFC-0001 blocks, `--format text|json`); golden tests in
`tests/diff/`.

### 9.5 Matching Objects with Different `@id`s

An as-built model produced by a recogniser usually carries its own `@id`s. A tool **MAY** offer a
matching mode; if it does, it **MUST** match base and head objects one-to-one in these passes,
each pass considering only objects not matched by an earlier pass:

| Pass | Method | Rule |
|---|---|---|
| 1 | `id` | same `@id` |
| 1b | `declared` | a binding as-built correspondence (`pmef:IsRevisionOf`, `changeType = AS_BUILT`, review status absent, `reviewed` or `approved`; [RFC-0003](rfc/0003-as-built-correspondence.md)) between objects of the two sides; correspondences with `review.status = rejected` forbid their pair in all later passes |
| 2 | `tag` | same `@type` and same engineering identifier — the first present of `equipmentBasic.tagNumber`, `lineNumber`, `tagNumber`, `memberMark`, `supportsMark`, `spoolMark`, `weldSpec.weldNumber`, `loopNumber` — where the value is unique per `@type` on both sides |
| 3 | `equivalent` | same `@type` and same native tool object: a `pmef:HasEquivalentIn` with equal `targetSystem` and `targetSystemId` on both sides, unique on both sides |
| 4 | `geometry` | same `@type`; distance ≤ limit (default 100 mm) |
| 5 | `reference` | objects with `ownerRef` whose owners are matched; relationships whose `sourceId` and `targetId` are matched (same `@type`); repeated until no new match |

**Geometry distance.** Anchor points are the `coordinate`s of `ports` and `nozzles`, and
`startPoint`, `endPoint` and `coordinate`. The distance of two objects is the symmetric mean
nearest-neighbour distance of their anchor sets (independent of anchor order and direction),
rounded to 6 decimals; objects without anchors are not matched geometrically. Candidate pairs
are assigned greedily in the order (distance, base `@id`, head `@id`). A geometric match is
**ambiguous** if another candidate pair sharing exactly one of its two objects has a distance
smaller than the chosen distance plus the ambiguity margin (default 10 mm); ambiguous matches
need review.

Correspondence relationships are matching evidence: they are excluded from the objects to
match and are not compared. A tool **MAY** write the matches found by passes 2–5 as proposed
correspondences (`review.status = detected`, RFC-0003), e.g. `pmef diff --match --emit-matches FILE`.

After matching, every string in the head package equal to a matched head `@id` is replaced by
the base `@id`, and the packages are compared per §9.1–§9.4. The report lists matched objects
with `headId` and `matchedBy`, and a `matches` array of all non-`id` correspondences
(`baseId`, `headId`, `method`, `distanceMm` for geometry, `ambiguous` when true), sorted by
`baseId`. Reference implementation: `pmef_core::matching`, `pmef diff --match
[--match-distance <mm>]`.

---

## 10 Three-Way Merge

*Since 0.10.0.* A three-way merge combines two independent edits (`ours`, `theirs`) of a common
ancestor (`base`) — e.g. design changes made while confirmed as-built deviations were being
recorded. A tool that merges PMEF packages (e.g. `pmef merge`) **MUST** follow these rules.

### 10.1 Rules

1. Objects are matched by `@id` (§9.1; duplicates are an error). Packages whose objects carry
   different `@id`s for the same item **SHOULD** first be reconciled via as-built correspondences
   ([RFC-0003](rfc/0003-as-built-correspondence.md)).
2. For every object and, recursively, every field — with absent meaning "not present / deleted":
   - `ours` equals `theirs` → that value;
   - `ours` equals `base` → `theirs`; `theirs` equals `base` → `ours`;
   - otherwise, if all present values are objects → merge per key; if all three are arrays with a
     common key field (§9.2) → merge per key; if all three are arrays of equal length → merge per
     position;
   - otherwise → **conflict**.
3. Equality uses §9.2 (numeric tolerances unless exact mode, keyed arrays order-free).
4. Conflict kinds: `modify-modify` (both changed differently), `modify-delete` (one side deleted
   what the other changed), `add-add` (both added different values where base had none). The
   merged output contains the value of the preferred side (default `ours`); every conflict
   **MUST** be reported with object `@id`, path (§9.3) and the base, ours and theirs values.
5. Keyed arrays keep the element order of `ours`, followed by keys only in `theirs` in key order.

### 10.2 Output

The merged package is written one object per line, `pmef:FileHeader` first, all other objects
ordered by `@id`, object keys sorted (§2.5) — so the same inputs always produce the same bytes,
and swapping `ours` and `theirs` together with the preferred side yields identical output.
A command-line tool **SHOULD** exit with 0 (merged cleanly), 1 (conflicts; output written) or
2 (error). Reference implementation: `pmef_core::merge`, `pmef merge BASE OURS THEIRS -o OUT
[--prefer ours|theirs] [--exact] [--format text|json]`; golden tests in `tests/merge/`.

---

*End of Chapter 03.*

**[← Chapter 02](02-information-model.md)** · **[Chapter 04 — Geometry →](04-geometry.md)**
