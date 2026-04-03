# PMEF Specification · Chapter 03 · Serialisation

**Document number:** PMEF-SPEC-03  
**Version:** 0.9.0-rc  
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

A PMEFX file is a ZIP archive that bundles one or more PMEF
NDJSON files with associated geometry assets. It uses the file
extension `.pmefx`.

### 3.2 Archive Structure

```text
my-plant.pmefx  (ZIP archive)
├── manifest.json           — package manifest (REQUIRED)
├── model/
│   ├── main.ndjson         — primary PMEF NDJSON file
│   ├── ei.ndjson           — E&I objects (optional split)
│   └── steel.ndjson        — structural objects (optional split)
├── geometry/
│   ├── model.glb           — glTF 2.0 geometry (optional)
│   ├── model.usdc          — OpenUSD geometry (optional)
│   └── model.stp           — STEP AP242 (optional)
├── catalogs/
│   ├── piping-class-a1a2.json
│   └── profiles-en.json
└── documents/
    ├── DS-P-201A-Rev3.pdf
    └── ISO-CW-201-Rev2.pdf
```

### 3.3 Manifest

The `manifest.json` file **MUST** be present at the root of the archive.

Required manifest fields:

| Field | Description |
|-------|-------------|
| `pmefVersion` | PMEF version |
| `packageId` | `@id` of the `pmef:FileHeader` object |
| `mainFile` | Relative path to the primary NDJSON file |
| `files` | Array of all files in the archive with type and path |
| `createdAt` | ISO 8601 timestamp |
| `coordinateSystem` | `"Z-up"` or `"Y-up"` |

Example manifest:

```json
{
  "pmefVersion": "0.9.0",
  "packageId": "urn:pmef:pkg:proj:my-plant",
  "mainFile": "model/main.ndjson",
  "coordinateSystem": "Z-up",
  "createdAt": "2026-03-31T10:00:00Z",
  "files": [
    {"path": "model/main.ndjson", "type": "ndjson", "objects": 1250},
    {"path": "model/ei.ndjson", "type": "ndjson", "objects": 340},
    {"path": "geometry/model.glb", "type": "gltf", "sizeBytes": 48200000}
  ]
}
```

### 3.4 Split File Packages

Large plant models **MAY** be split across multiple NDJSON files
within a PMEFX archive. The rules for split packages are:

1. Each NDJSON file **MUST** have a valid `pmef:FileHeader` as its first line.
2. All `pmef:FileHeader` objects in a split package **MUST** reference the same `plantId`.
3. Cross-file `isPartOf` and other references **MUST** use the full `urn:pmef:` URI (not a relative path).
4. The manifest **MUST** list all NDJSON files.

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
- Deleted objects are represented by a PMEF object with `changeState: "ARCHIVED"` and no other content changes.
- Delta packages **MAY** reference objects defined in the base package using their full `urn:pmef:` URIs.

---

*End of Chapter 03.*

**[← Chapter 02](02-information-model.md)** · **[Chapter 04 — Geometry →](04-geometry.md)**
