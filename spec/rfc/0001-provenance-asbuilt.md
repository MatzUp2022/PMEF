# RFC-0001: Recognition Provenance, Review Status and As-Built Data

| Field | Value |
|---|---|
| **Status** | `DRAFT` — implemented on a feature branch for review |
| **Target version** | 0.10.0 (minor: new optional fields only) |
| **Working groups** | WG-Piping, WG-Steel, WG-Equipment, WG-Geometry, WG-Catalogs |
| **Author** | PMEF maintainers |
| **Date** | 2026-10-31 |

## Abstract

Every PMEF object gains three **optional** blocks: `provenance` (where the object came from and
how sure the recogniser is), `review` (human review status, mapped to ISO 19650) and `asBuilt`
(fit quality against measured data and the catalog entry the object was snapped to). The blocks
make automatically recognised plant models exchangeable without losing the information needed
to trust, review and compare them.

## Motivation

PMEF is the backbone of a recognition workflow that builds plant models from 2D drawings,
isometrics, DWG/DXF/DGN/PCF/PDF and point clouds, and compares as-built against design.
Today a PMEF object cannot say:

1. whether it was modelled by a person, imported from CAD or *recognised* from a scan or drawing;
2. which source document, page, region or point-cloud segment it was derived from;
3. how confident the recogniser was, and whether a person has checked the result;
4. how well it fits the measured data, and which catalog entry made it exact
   ("exactness comes from snapping, not raw geometry").

Without this, downstream tools must either trust everything or nothing, reviewers cannot
prioritise low-confidence objects, and as-built deviations are not exchangeable.

Use cases:

- **UC-1 Review queue** — a tool lists all objects with `review.status = detected` sorted by
  `provenance.confidence`.
- **UC-2 Traceability** — from any object, open the source drawing region or point-cloud segment.
- **UC-3 As-built vs. design** — report per object the RMS/max deviation and the snapped catalog
  size; compare against the design model by `@id`.
- **UC-4 Release gate** — a CDE refuses to publish objects that are not reviewed.

## Detailed Design

### Placement

The three blocks are optional top-level properties of **every PMEF object** that carries
`revision` (all object types in `pmef-hierarchy` except `FileHeader`, and all types in
`pmef-piping-component`, `pmef-equipment`, `pmef-steel`, `pmef-ei`, `pmef-geometry`,
`pmef-relationships`). `FileHeader` is a package header, not a plant object, and is excluded.

The definitions live in `pmef-base.schema.json` (`$defs/Provenance`, `$defs/ReviewStatus`,
`$defs/AsBuilt`, `$defs/CatalogSnap`) and are referenced from each object definition.

### `provenance`

| Field | Req. | Type | Description |
|---|---|---|---|
| `sourceType` | **MUST** | enum | `manual`, `cad-import`, `iso-drawing`, `2d-drawing`, `scan`, `photo` |
| `sourceRef` | SHOULD (MUST unless `manual`) | URI | Source document / page / region or point-cloud segment |
| `recognizer` | SHOULD for automated sources | object | `name` (MUST), `version` (MUST) |
| `confidence` | MUST if `recognizer` present | number 0..1 | Recogniser confidence for this object |
| `createdAt` | SHOULD | date-time | When the object was created from the source (ISO 8601) |

`sourceRef` is a URI. Recommended fragment conventions (informative):

- documents: `…/ISO-CW-201.pdf#page=2&region=0.12,0.40,0.31,0.55` — `region` is
  `x0,y0,x1,y1` in page-relative coordinates 0..1, origin top-left;
- point clouds: `…/scan-2026-10.e57#segment=CW-201-seg-07`;
- PMEF documents: `urn:pmef:doc:<project>:<doc-id>` (spec 01 §7.3) plus the same fragments.

Schema rules: `dependentRequired: recognizer → confidence`; if `sourceType ≠ manual` then
`sourceRef` is required.

### `review`

| Field | Req. | Type | Description |
|---|---|---|---|
| `status` | **MUST** | enum | `detected`, `reviewed`, `approved`, `rejected` |
| `reviewedBy` | MUST unless `detected` | string | Person or role identifier |
| `reviewedAt` | MUST unless `detected` | date-time | ISO 8601 |

**Relation to ISO 19650.** `review.status` describes *object-level* checking of a recognition
result; `revision.changeState` (WIP/SHARED/PUBLISHED/ARCHIVED) describes the *information
container* state in the CDE. They are orthogonal, with these rules:

| `review.status` | Meaning | Allowed `revision.changeState` |
|---|---|---|
| `detected` | Created by a recogniser, not checked | `WIP` |
| `reviewed` | Checked by a person, corrections applied, not yet accepted | `WIP`, `SHARED` |
| `approved` | Accepted for use | any |
| `rejected` | Recognition result judged wrong; kept for traceability | `WIP`, `ARCHIVED` |

Normative rule (schema-enforced): an object with `review.status` `detected` or `rejected`
**MUST NOT** have `revision.changeState = PUBLISHED`; `detected` **MUST** be `WIP`.
The ISO 19650 status-code layer (S0–S7, A1…) is not modelled by PMEF; tools map
`approved` + `PUBLISHED` to their authorisation codes.

### `asBuilt`

| Field | Req. | Type | Description |
|---|---|---|---|
| `pointCloudRef` | SHOULD | URI | Point cloud (segment) the object was fitted to |
| `fitMethod` | SHOULD | string | e.g. `ransac-cylinder`, `least-squares-cylinder`, `ransac-plane`, `icp`, `manual` |
| `deviationRms_mm` | SHOULD | number ≥ 0 | RMS distance measured points → fitted surface, mm |
| `deviationMax_mm` | SHOULD | number ≥ 0 | Maximum distance, mm (SHOULD be ≥ `deviationRms_mm`) |
| `catalogSnap` | MAY | object | See below |

`catalogSnap`:

| Field | Req. | Type | Description |
|---|---|---|---|
| `catalogRef.catalogId` | **MUST** | string | Catalog file id, e.g. `profiles-en`, `piping-class-en-p1a` |
| `catalogRef.catalogVersion` | **MUST** | string | Catalog `version`, e.g. `2024.1` — snapping is only reproducible against a versioned catalog |
| `catalogRef.entryId` | **MUST** | string | Catalog entry `catalogId`, e.g. `EN:HEB300`, `ENP1A-PIPE-150` |
| `snappedSize` | **MUST** | string | Human-readable designation, e.g. `HEB 300`, `DN 150 PN 40` |
| `residuals` | SHOULD | array | Per parameter: `parameter`, `measured_mm`, `catalog_mm`, `residual_mm` (= measured − catalog) |

The snapping *algorithm* (how a size is chosen) is out of scope for this RFC and will be
specified in a follow-up RFC; this RFC only standardises how its result is recorded.

### Serialisation Impact

None for existing data: all blocks are optional, existing files stay valid. Example
(one line in NDJSON, pretty-printed here):

```jsonc
{"@type":"pmef:SteelMember","@id":"urn:pmef:obj:demo-asb:B-101", …,
 "provenance":{"sourceType":"scan","sourceRef":"file:scans/hall-2.e57#segment=beam-0042",
   "recognizer":{"name":"demo-recognizer","version":"1.4.0"},"confidence":0.91,
   "createdAt":"2026-10-30T09:12:00Z"},
 "review":{"status":"approved","reviewedBy":"j.doe","reviewedAt":"2026-10-30T14:00:00Z"},
 "asBuilt":{"pointCloudRef":"file:scans/hall-2.e57#segment=beam-0042","fitMethod":"ransac-plane",
   "deviationRms_mm":1.8,"deviationMax_mm":6.2,
   "catalogSnap":{"catalogRef":{"catalogId":"profiles-en","catalogVersion":"2024.1","entryId":"EN:HEB300"},
     "snappedSize":"HEB 300",
     "residuals":[{"parameter":"h","measured_mm":301.4,"catalog_mm":300,"residual_mm":1.4}]}}}
```

Full example: `examples/asbuilt-recognition.ndjson`.

### Upstream Standard Reference

- ISO 19650-1:2018 §12 (CDE states) — mapping table above.
- ISO 15926 / IDO, DEXPI 2.0, CFIHOS: no equivalent provenance construct; carried as PMEF
  extension, exported as attributes (see CAEX note below).
- W3C PROV-O (informative): `provenance` corresponds to `prov:wasDerivedFrom` (`sourceRef`),
  `prov:wasGeneratedBy` (`recognizer`) and `prov:generatedAtTime` (`createdAt`).

### CAEX / AutomationML mapping (IEC 62424)

PMEF's CAEX serialisation maps each PMEF object to an `InternalElement`. The new blocks map to
nested CAEX `Attribute`s on that element, using the JSON property names:

```xml
<InternalElement Name="B-101" ID="urn:pmef:obj:demo-asb:B-101">
  <Attribute Name="provenance">
    <Attribute Name="sourceType" AttributeDataType="xs:string"><Value>scan</Value></Attribute>
    <Attribute Name="sourceRef" AttributeDataType="xs:anyURI"><Value>file:scans/hall-2.e57#segment=beam-0042</Value></Attribute>
    <Attribute Name="recognizer">
      <Attribute Name="name" AttributeDataType="xs:string"><Value>demo-recognizer</Value></Attribute>
      <Attribute Name="version" AttributeDataType="xs:string"><Value>1.4.0</Value></Attribute>
    </Attribute>
    <Attribute Name="confidence" AttributeDataType="xs:double"><Value>0.91</Value></Attribute>
    <Attribute Name="createdAt" AttributeDataType="xs:dateTime"><Value>2026-10-30T09:12:00Z</Value></Attribute>
  </Attribute>
  <Attribute Name="review"> … status / reviewedBy / reviewedAt … </Attribute>
  <Attribute Name="asBuilt">
    <Attribute Name="deviationRms_mm" AttributeDataType="xs:double" Unit="mm"><Value>1.8</Value></Attribute>
    …
  </Attribute>
</InternalElement>
```

Rules: nested JSON objects → nested `Attribute`; enums and strings → `xs:string`; URIs →
`xs:anyURI`; numbers → `xs:double` with `Unit="mm"` for `_mm` fields; timestamps →
`xs:dateTime`; the `residuals` array → one child `Attribute` per entry named
`residual_<parameter>`. A PMEF `AttributeTypeLib` (`PMEF_RecognitionAttributes`) with
`Provenance`, `ReviewStatus` and `AsBuilt` attribute types SHOULD be referenced via
`RefAttributeType` so AML tools can recognise the blocks. No CAEX writer exists yet in
`pmef-rs`; this mapping is the contract for it.

## Backward Compatibility

Fully backward compatible: new optional properties only; no existing field, `@id` pattern or
schema file name changes. `pmefVersion` for data using the blocks is `0.10.0`.

Open point: schema `$id`s keep the `/0.9/` path in this change (changing `$id`s is a schema
rename and needs maintainer approval).

## Alternatives Considered

- **Put the data in `customAttributes`** — rejected: untyped, not validatable, not interoperable.
- **Extend `revision`** — rejected: revision is about the information container lifecycle;
  provenance and fit quality are about the object's origin and geometry and can change
  independently.
- **Reuse `pmef:IsDerivedFrom` relationships for provenance** — kept for object-to-object
  derivation; source documents and point-cloud segments are not PMEF objects, so a URI on the
  object is simpler and streamable.
- **A single `confidence` per review** — rejected: confidence belongs to the recogniser output,
  review is a human decision.

## Open Questions

1. Should `$id` paths move to `/0.10/` (schema rename — maintainer decision)?
2. Should `review` also record `comment` and a link to a markup/BCF issue?
3. Snapping algorithm and residual definitions per catalog type (follow-up RFC).

## Implementation Notes

- Schemas: `pmef-base` `$defs` + references in all object definitions (this branch).
- Rust: `pmef-core::provenance` (`Provenance`, `ReviewStatus`, `AsBuilt`, `CatalogSnap`) and a
  flattened `RecognitionMetadata` on every object struct; serde round-trip tests.
- Validation: example `examples/asbuilt-recognition.ndjson`, negative tests in
  `pmef-validate/tests/provenance.rs`.

## References

- `spec/02-information-model.md` §2.9 (normative text of this RFC)
- `docs/design-decisions.md` DD-006 (geometry layers), DD-007 (RevisionMetadata)
- ISO 19650-1/-2:2018; IEC 62424:2016 (CAEX); W3C PROV-O
