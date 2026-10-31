# CHANGELOG

All notable changes to the PMEF Specification are documented here.
Adheres to [Semantic Versioning](https://semver.org/) and [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

**Version identifiers.** The data/spec version is plain SemVer (`pmefVersion: "0.9.0"`, schema
`$id` path `/0.9/`). Pre-release labels such as `-rc` mark *releases* (Git tags, e.g.
`v0.9.0-rc`) and never appear in data. The next schema extension is `0.10.0`.

---

## [Unreleased]

### Removed

- **RVM parser removed from `pmef-adapter-e3d`** (`src/rvm.rs`, `E3DConfig::rvm_export_path`,
  `byteorder` dependency). AVEVA's RVM format is not publicly specified; the reference
  implementation does not reverse-engineer it. See DD-011 in `docs/design-decisions.md`.
  RVM-only models are planned via an external RVM→glTF conversion + `mesh_ref` (ROADMAP).

### Fixed

- **`validate-examples.py` silently skipped 21 of 117 example objects** (all structural-steel
  and E&I types) because its hand-written type list was incomplete, and unknown `@type`s passed.
  The `@type` → schema map is now derived from the schemas themselves; unknown types are errors.
  A Rust test (`pmef-validate/tests/type_map_matches_schemas.rs`) fails if `schema_for_type`
  drifts from the schemas.
- **The whole Rust workspace builds again** (`cargo build --workspace`): all ten
  `pmef-adapter-*` crates failed to compile. `pmef-io` now implements
  `From<PmefIoError> for AdapterError` (used by all adapters); missing `tracing` dependency
  (Plant 3D); Tekla profile override table no longer needs the undeclared `phf` crate;
  lifetime and `&mut self` fixes (Plant 3D, Tekla, CADMATIC); missing test imports
  (Creo, Inventor). All 259 workspace tests pass.
- **Creo STEP parser read digits inside entity names as coordinates**
  (`CARTESIAN_POINT('NOZZLE_N1',(200,…))` yielded `1.0, 200, 400`). Quoted names are now skipped.
- **Plant 3D PCF: blind/slip-on/socket-weld flanges were never detected.** The flange type was
  read from SKEY characters 1–2 (always `FL`) instead of 3–4 (`FLBL`, `FLSO`, …).
- **CADMATIC test expected a wrong value** for 15 barg → Pa(abs): correct is 1 601 325 Pa
  (the implementation was right).
- **E3D PML parser swallowed attributes after reference values.** Any attribute whose value
  starts with `/` (e.g. `SPEC /SPEC-A1A2`, `CREF /PIPE-1`) was treated as a new element,
  so all following attributes were attached to it. Only structural and component keywords
  now start elements.
- **`pmef validate` crashed on every real dataset.** `PmefValidator` compiled
  each `$defs` sub-schema in isolation with no document store, so cross-file
  `$ref`s (`pmef-base.schema.json`, `pmef-property-sets.schema.json`,
  `#/$defs/EquipmentObject`) could not resolve; `jsonschema` then fell back to
  fetching schemas over HTTP, which panicked inside the async runtime. The
  validator now loads every `*.schema.json` in the directory, indexes them by
  `$id`, and validates via an absolute `$ref` root so all references resolve
  locally. `pmef validate examples/*.ndjson` now runs to completion.
- **`FileHeader` schema drifted from the spec.** `pmef-hierarchy.schema.json`
  required `createdAt` (a PMEFX container-manifest field, not a FileHeader
  field per spec §3.1) and defined `projectId` instead of `projectCode`, so
  every benchmark `FileHeader` failed validation — hidden because
  `validate-examples.py` skipped hierarchy objects. Aligned the schema to
  spec §3.1: `required` is now `@type`/`@id`/`pmefVersion`/`plantId`, and the
  field is `projectCode`. Added a top-level `oneOf` so the hierarchy schema
  validates its objects like the piping/equipment schemas do.
- **`validate-examples.py` now validates `FileHeader`/`Plant`/`Unit`/`Area`**
  against `pmef-hierarchy.schema.json` instead of skipping them, so CI catches
  hierarchy schema/data divergence.
- **Relationship objects were never validated** (both tools). `schema_for_type`
  (Rust) and `TYPE_TO_SCHEMA` (Python) listed relationship type names that do
  not exist in `pmef-relationships.schema.json` (`pmef:ConnectedTo`,
  `pmef:HasPort`, `pmef:FlowsTo`, …) and omitted the real ones, so every
  relationship object was silently skipped. Both maps now use the schema's
  actual relationship types (`IsConnectedTo`, `IsDerivedFrom`, `Supports`,
  `ControlledBy`, `HasEquivalentIn`, `IsRevisionOf`, …).
- **`pmef validate` conformance pass rate** now excludes skipped objects.
  A type the validator has no schema for is a tool gap, not a data defect, and
  no longer drags the pass rate below the conformance threshold. With the
  relationship fix, all five benchmark datasets now report 0 errors, 0 skipped,
  and Conformance L1 PASS.

### Changed

- **README and contributor docs match the repository.** Repo URL `MatzUp2022/PMEF` instead of
  the placeholder `pmef/specification` (README, spec 01, Cargo.toml, CONTRIBUTING, getting
  started, issue templates); real folder names and file lists; placeholder badges and links
  removed (Discord, OpenSSF, ghcr.io image, crates.io install); "14+ bidirectional adapters"
  replaced by an implementation-status table; CLI install from source.
- **`PMEF_Specification_v1.2.docx` renamed to `PMEF_Specification_v1.0-draft_DE.docx`** — its
  title page says v1.0 draft; it is a non-normative legacy artefact.
- **Rust CI** (`.github/workflows/rust.yml`): builds and tests the whole `pmef-rs` workspace
  and runs `pmef validate` on every example; clippy runs informationally. The release workflow
  now requires it.
- **`pmef-adapter-e3d`: equipment geometry from PML design primitives.** `BOX`, `CYLI`,
  `CONE`, `SNOU` and `DISH` children of an `EQUI` are mapped to `pmef:ParametricGeometry`
  (single primitive or `COMPOSITE`) and referenced from the equipment via
  `geometry: {type: "parametric", ref}`. Invalid primitives are reported via
  `E3DAdapter::warnings()` instead of being dropped silently. Spec 05 §8.1 updated.
- **`pmef-adapter-e3d` compiles again** (writer errors mapped to `AdapterError::Other`).

- **Catalog schema (`schemas/pmef-catalog.schema.json`)**: tightened from a
  loose envelope into a per-`catalogType` contract. Each `entries` item is now
  validated against a type-specific `$def` (steel profile, piping class, pipe
  dimension, flange dimension, instrument, material grade, RDL mapping), with
  required identifier/core fields enforced per type. Added `RDL_MAPPING` and
  `PIPE_STRESS_MAPPING` to the `catalogType` enum; `pressureClass` now accepts
  numeric ASME classes and EN `PN` strings.
- Reclassified `catalogs/rdl-uri-map.json` from `INSTRUMENT_CATALOG` to
  `RDL_MAPPING` to match its actual content.

### Added

- `scripts/validate-catalogs.py` — validates every file in `catalogs/` against
  the catalog schema and flags duplicate `catalogId` values within a catalog.
- CI job `validate-catalogs` (and `catalogs/**` path trigger) in the
  schema-validation workflow, so malformed catalogs fail the build.
- `scripts/check-topology.py` — dataset connectivity integrity checks for
  NDJSON examples: unique `@id`, `connectedTo` resolution, and port
  reciprocity (with `--allow-external` for federated/cross-dataset boundaries).
  Wired into the `validate-examples` CI job.
- **`pmef-validate` topology module** (`pmef-rs/pmef-validate/src/topology.rs`):
  Rust reference implementation of the same connectivity checks (unique `@id`,
  `connectedTo` resolution, port reciprocity), exposed as `check_topology()`
  with a `TopologyReport`, at parity with `check-topology.py`.
- **`pmef topology` CLI subcommand** — runs the topology checks over an NDJSON
  package, with `--allow-external` for cross-dataset boundaries.

---

## [0.9.0-rc] — 2026-03-31 · Release Candidate

### Summary

First public release candidate. Covers the complete five-layer architecture
for Piping, Equipment, E&I, Structural Steel, and Pipe Stress. Includes
the full Rust reference implementation workspace (`pmef-rs`), 7 open JSON
catalogs, 5 validated benchmark datasets (117 objects, 0 schema errors),
6 normative specification chapters (18,128 words), and a conformance test
suite with 82 tests (80 passing, 97.6%).

---

### Added — Specification (spec/)

Six normative Markdown chapters totalling 3,087 lines / 18,128 words:

- **Ch. 01 Introduction**: 30+ normative references, 25 term definitions,
  45 abbreviations, problem statement, design goals G-01..G-09, five-layer
  architecture, P&ID→3D→EAM linking model, standards alignment
- **Ch. 02 Information Model**: entity type registry (50+ types), all base
  types (PmefId, RevisionMetadata, GeometryReference, CatalogReference, Port),
  plant hierarchy, piping domain (13 component subtypes + topology rules),
  equipment domain (10 subtypes + Nozzle), E&I, steel, 10 relationship types,
  11 property sets, extension mechanism
- **Ch. 03 Serialisation**: NDJSON primary format (canonical key order,
  LF, UTF-8, one-object-per-line), PMEFX container, CAEX XML secondary,
  delta package protocol
- **Ch. 04 Geometry**: 15 parametric primitives fully specified, 5 LOD levels,
  glTF 2.0 + STEP AP242 + OpenUSD layer specs, primitive-to-component mapping
- **Ch. 05 Adapters**: 8-step export + 6-step import pipeline, identity
  resolution, 8 general requirements, 6 tool-specific specs (E3D, Plant3D,
  CADMATIC, Tekla, COMOS, EPLAN), full PCF mapping + unit conversion tables
- **Ch. 06 Conformance**: 3 conformance levels (L1/L2/L3), 32 normative
  clauses, entity type matrix (50+ × 3 levels), 5 benchmark datasets,
  RT fidelity definition, conformance report schema, certification process

### Added — JSON Schemas (schemas/)

9 schemas, all Draft 2020-12, all valid:
`pmef-base`, `pmef-piping-component`, `pmef-equipment`, `pmef-geometry`,
`pmef-property-sets`, `pmef-ei`, `pmef-steel`, `pmef-relationships`,
`pmef-catalog` (new)

### Added — Catalogs (catalogs/) — CC0

| File | Entries | Description |
|------|---------|-------------|
| `profiles-en.json` | 202 | HEA/HEB/IPE/UPE/RHS/SHS/CHS/L/Flat per EN |
| `profiles-aisc.json` | 119 | W / HSS Rect / HSS Round / L per AISC 16th |
| `pipe-dimensions.json` | 162 | ASME B36.10M + B36.19M, DN15–DN1200 |
| `piping-class-a1a2.json` | 203 | CS ANSI-150, DN15–DN600, 9 component types |
| `piping-class-b3c1.json` | 143 | CS ANSI-300, DN15–DN400, 8 component types |
| `caesarII-cii-mapping.json` | 29 records | PMEF↔CAESAR II .cii + ROHR2 .ntr |
| `rdl-uri-map.json` | 35 | ISO 15926-4 PCA-RDL URI cross-references |

### Added — Benchmark Datasets (examples/) — CC0

| Dataset | Objects | Disciplines | Key content |
|---------|---------|------------|-------------|
| DS-01 `pump-skid-complete.ndjson` | 35 | Piping + Equipment + Steel | P-201A (API 610), V-201 (EN 13445), CW-201/202/203, full piping chain, 2 welds, spool, geometry |
| DS-02 `heat-exchanger-station.ndjson` | 26 | Piping + Equipment + E&I | E-301A/B (TEMA BEM), FV-30101 control valve, FIC-30101 (HART), 8 lines |
| DS-03 `compressor-train.ndjson` | 34 | Piping + Equipment + E&I | K-101 (API 617), ST-101 (API 611), V-101 KO drum, E-101 (TEMA AES), STR-101, SIL2 anti-surge |
| DS-04 `ei-loop-fic-10101.ndjson` | 9 | E&I | FIC-10101 SIL1 controller, XV-10101, S7-1500, loop FIC-101 |
| DS-06 `stress-model-cw201.ndjson` | 13 | Piping + Stress | CW-201 CAESAR II model, anchor reactions, spring hanger, stress ratios |

### Added — Rust Reference Implementation (pmef-rs/) — Apache 2.0

6-crate Cargo workspace, 3,723 lines:

- **pmef-core** (1,831 lines): `PmefId` (FromStr), `PmefVersion`, `Coordinate3D`,
  `RevisionMetadata`, `PmefEntityType` (50 variants), `PmefEntity`/`PmefVisitor`/
  `PmefAdapter` traits, `AdapterError`/`AdapterStats`, all 50+ entity structs,
  `piping_component_base!{}` macro, domain methods
- **pmef-io** (375 lines): `NdjsonReader<R: BufRead>` iterator, `NdjsonWriter`,
  `canonical_json()`, `PmefPackageIndex` — 4 unit tests
- **pmef-validate** (21 lines): `schema_for_type()` routing table
- **pmef-geom** (18 lines): `Vec3`, `Aabb`, `cylinder_aabb()`
- **pmef-cli** (466 lines): 7 subcommands (validate, diff, stats, convert,
  index, conformance, check-refs) — clap 4 derive, Tokio async
- **pmef-adapter-plant3d** (672 lines): full PCF parser + export pipeline,
  `MaterialMapper`, coordinate-based topology, 9 unit tests

### Added — Conformance Test Suite (pmef-conformance/)

82 tests: PIPE-U-001..016, EQUIP-U-001..012, EI-U-001..008, STEEL-U-001..006,
REL-U-001..008, GEOM-U-001..008, RT-001..018, INTG-001..006.
**80/82 passing (97.6%).**

---

### Fixed (schema bugs discovered by test suite)

| Fix | Schema | Details |
|-----|--------|---------|
| `PmefId` underscore | `pmef-base` | `_` now allowed in local-id segment |
| `PIPE_SUPPORT` enum | `pmef-property-sets` | Added to componentClass enum |
| `insulationType` enum | `pmef-equipment` | Short forms `HOT`/`COLD` (not `THERMAL_HOT`) |
| `revision` field | `pmef-geometry` | Added to ParametricGeometryObject |
| `operatingTemperature` | `pmef-piping-component` | Removed from PipingSpecification |

---

### Known Issues

| ID | Description | Status |
|----|-------------|--------|
| EQUIP-U-005 | `KNOCK_OUT_DRUM`/`ELECTRIC_ARC_FURNACE` not in vesselSubtype enum | Fix in v0.9.1 |
| PIPE-U-003 | Error message assertion too strict on lineNumber error text | Fix in v0.9.1 |
| pmef-geom | Full AABB for all 15 primitives is a stub | Planned v1.0 |
| pmef-validate | jsonschema Rust crate integration is a stub | Planned v1.0 |
| DS-05 | Mixed EAF + E&I + Steel benchmark not yet produced | Planned v0.9.1 |
| CADMATIC adapter | REST API adapter not yet produced | Planned v0.9.1 |

---
