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

- **Rust model matches the schemas (docs/AUDIT.md P3.9).** `pmef-core` types for the active scope
  (hierarchy, piping, equipment, steel, relationships) now accept every schema-valid object and
  round-trip it without loss: fields required in Rust but optional in the schema became `Option`
  (e.g. `PipeSupport.supportsMark`, `Pipe.pipeLength`, the `PumpSpec`/`VesselDesign`/`hxSpec`
  members), missing schema properties were added, and wrong JSON names were fixed —
  `Iec81346Designation` (`functionalAspect` etc. were silently dropped), `GeometryReference.ref`,
  `SteelMember.teklaGUID`, `CatalogReference.eclassIRDI`, `CompressorSpec.polytropic_efficiency`.
  New test `pmef-cli/tests/schema_drift.rs` guards this against every reference instance.

- **E3D adapter output is schema-valid (docs/AUDIT.md P1.7).** `pmef-adapter-e3d` no longer
  writes `null` for absent PML attributes, puts equipment design data into the required
  type-specific blocks (`vesselDesign`, `pumpSpec`, `compressorSpec`, `hxSpec`, tank fields)
  and always sets `nozzleMark`. In-line piping components (flanges, valves, supports, tees,
  reducers, …) were all exported as `pmef:Pipe` (enum name passed instead of the PML keyword);
  they now get their proper `@type`. New integration test `tests/output_validates.rs` validates
  every exported object against `schemas/`.

- **Benchmark geometry made consistent** (new CI check `scripts/check-geometry.py`: bend centre
  points, port directions per spec 02 §2.6, pipe lengths). PMEF-DS-01 / stress model: elbow
  CW-201-ELB-001 had both ports on the same vertical with no intersecting axes — it is now a real
  90° LR DN200 bend (centre-to-end 304.8 mm around the original corner at x = 11500, z = 850):
  CW-201-PIPE-001 ends at x = 11195.2 (length 2195.2 mm, weight 75.0 kg), CW-201-PIPE-002 starts at
  z = 1154.8 (795.2 mm, 27.8 kg), welds W01/W02 moved accordingly, elbow torus geometry, spool
  length/weight updated. Inward-pointing port directions fixed on CW-201-PIPE-002, -FL-001,
  -GK-001, -FL-002 and on PMEF-DS-02 CW-301-FV-001, -PIPE-002; weld directions `[0,0,0]` replaced
  by the pipe axis. No `@id` changed. Spec 06 §4.1 object count of DS-01 corrected (35, not 16).
- **`catalogs/profiles-en.json` HEA series corrected (catalog version 2024.1 → 2024.2).** All 24
  HEA entries listed the nominal size as height (HEA 300: 300 instead of 290 mm), HEA 900/1000
  had wrong flange thicknesses (28 instead of 30/31 mm), HEA 100 was listed as `EN:HEA96`, and
  `Iy`, `Iz`, `Wel_y`, `Wel_z` were wrong by a factor of up to 15. Fixed: h and tf per the EN 10365
  dimension tables (the corrected h also follows independently from the catalog's own,
  correct cross-section areas); `EN:HEA96` → `EN:HEA100` (entry id change); section properties
  recomputed from the dimensions with `scripts/i-section-properties.py`, which reproduces all
  HEB and IPE entries within 0.31 % and the published HEA properties within 1 %. CI now checks
  every I/H section's properties against its dimensions (±1 %). Snapping test vectors updated
  (only `catalogVersion` changed) plus SNAP-017: HEA 300 and HEB 300 are now distinguishable by
  h and b.
- **Conformance fixtures no longer pick up 0.10.0 feature examples by accident.** The
  generator took the first object of a type from all examples, so after RFC-0001 several
  Level-1 fixtures (PIPE-U-002/004/005/016, STEEL-U-002/005) silently used objects from
  `asbuilt-recognition.ndjson`. They now come from the 0.9.0 benchmark datasets only.
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

- **`pmef-core` API (Rust):** helper methods on fields that are optional in the schema now return
  `Option<f64>`: `PipingDesignConditions::design_temperature_celsius`/`design_pressure_barg`,
  `PipingSpecification::bore_mm`/`effective_wall_mm`, `HeatExchangerSpec::lmtd_counter_current`.
  The corresponding struct fields are `Option<_>`. No workspace caller was affected.

- **`pmef convert` implemented** (was a stub): `--to pcf|pmef`, `--base`, `-o`; the unused
  `--from` and `--schemas` options were removed.
- **Conformance suite: INTG-006 checks only objects of the claimed `--domains`** (spec 06 CL3-05:
  "every *exported* object"); before, an adapter exporting only piping failed for equipment and steel.
- **`pmef diff` CLI options changed:** `--changed-only` removed (unchanged objects are never
  listed); new `--exact`, `--ignore`, `--ignore-recognition`. The command now exits with 1 when
  the files differ and 2 on errors.
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

- **AutoCAD Plant 3D import direction and round trip via PCF** (spec 05 §8.2).
  `pmef convert MODEL --to pcf -o DIR` writes one PCF per line (identity in
  `UNIQUE-COMPONENT-IDENTIFIER`); `pmef convert DIR --to pmef --base MODEL -o OUT` updates the
  model from the PCF exported by Plant 3D (re-identification, create, delete; fields PCF does not
  carry are kept). Library: `pmef_adapter_plant3d::pcf_bridge`. The PCF reader now also reads
  `CO-ORDS` (supports). Round trip of PMEF-DS-01 is lossless; 15 % of its attributes travel
  through PCF. Conformance: level 3 as converter
  (`tests/conformance/suts/plant3d-pcf.sh`, run in CI), not yet as adapter (INTG-006: no native
  Plant 3D id in PCF). Not yet verified with a Plant 3D installation.
- **Three-way `pmef merge`** (spec 03 §10). Merges two edits of a common base object by object and
  field by field with the diff rules (keyed arrays, numeric tolerances); one-sided changes are
  taken, differing changes on both sides are conflicts (`modify-modify`, `modify-delete`,
  `add-add`) reported with base/ours/theirs values; `--prefer ours|theirs` decides the written
  value. Deterministic output (FileHeader first, `@id` order, sorted keys); exit 0/1/2.
  Library `pmef_core::merge`; golden test: design change merged with confirmed as-built deviations.
- **RFC-0003 — persisted as-built ↔ design correspondence** ([RFC](spec/rfc/0003-as-built-correspondence.md)).
  `pmef:IsRevisionOf` with `changeType = AS_BUILT` gains an optional `correspondence` block
  (`method`, `distance_mm`, `ambiguous`; schema requires `AS_BUILT` when present); confirmation via
  the RFC-0001 `review` block. `pmef diff --match` uses confirmed correspondences as a new pass 1b
  (`declared`), never re-proposes rejected pairs, and treats correspondences as evidence rather
  than compared objects. `--emit-matches FILE` writes proposals (`review.status = detected`) for
  review. Rust: `relationships::AsBuiltCorrespondence`. Example and tests (schema rules, matching
  units, CLI propose → approve/reject → rerun workflow) added.
- **`pmef diff --match`: comparison when `@id`s differ** (spec 03 §9.5) — typical for as-built
  models with recogniser ids. Objects are matched one-to-one in fixed passes: same `@id`, same
  engineering identifier (tag, line number, mark — only if unique), same native tool object
  (`HasEquivalentIn`), geometric proximity of anchor points (ports, nozzles, member ends;
  direction-free mean distance ≤ `--match-distance`, default 100 mm, with an ambiguity flag), and
  references (geometry owners, relationship endpoints). References in the head package are
  rewritten to the matched ids before comparing, so differing ids cause no spurious changes.
  Library: `pmef_core::matching`, `pmef_core::diff::diff_matched`. Golden test: an as-built
  model with scanner ids yields the same plant deviations as the identical-id case.
- **Deterministic object-level `pmef diff`** (spec 03 §9). Objects matched by `@id`, field-level
  changes with stable paths (`ports[portId=P2].coordinate[2]`), keyed array matching (ports,
  nozzles, residuals, composite parts), spec 06 §6.2 numeric tolerances (`--exact` to disable),
  `--ignore <path>` and `--ignore-recognition`, text and JSON report, exit codes 0/1/2. Output
  is independent of object order, JSON key order and keyed array order; duplicate `@id`s are an
  error. Library: `pmef_core::diff`. Golden as-built-vs-design tests in `tests/diff/`.
- **RFC-0002 — deterministic catalog snapping `pmef-snap/1`** ([RFC](spec/rfc/0002-catalog-snapping.md),
  spec 02 §2.9.4). Normative algorithm for straight pipes (`PIPING_CLASS`) and HEA/HEB/HEM/IPE/W
  sections (`STEEL_PROFILES`, EN and AISC field conventions): tolerance band per parameter,
  admissibility, normalised RMS score, total ordering (score → primary residual → `catalogId`),
  runner-up and `ambiguous` flag (margin 0.1), recorded non-default tolerances. Schema:
  `CatalogSnap` gains optional `algorithm`, `score`, `runnerUp`, `ambiguous`, `tolerances`.
  Two implementations — Python reference (`tests/snapping/reference_snap.py`) and Rust
  (`pmef_core::snapping`) — pass the same 16 test vectors (`tests/snapping/vectors.json`) in CI.
  `examples/asbuilt-recognition.ndjson` is now generated with the reference implementation
  (`scripts/generate-asbuilt-example.py`).
- **Executable conformance test suite** (`tests/`, spec 06 §5–§7). `python tests/run_tests.py`
  runs the 82 normative test IDs — 50 unit and 8 negative schema tests (fixtures generated from
  one catalogue, `tests/conformance/manifest.json`), INTG-001…006 integration checks and
  RT-001…018 round-trip checks with aggregate and identity-field fidelity per spec 06 §6 — against
  any implementation via command templates (`--validator "cmd {file}"`,
  `--sut "cmd {input} {output}"`, `--kind validator|converter|adapter`). Produces the §7.1 JSON
  report (schema-checked) and the §7.2 text summary. A self-test proves that seven deliberately
  broken implementations and an accept-all validator are detected. CI runs the suite, the
  self-test, and the Rust `pmef` CLI as a second validator (agrees with the Python reference on
  all 58 unit/negative tests). This replaces the `pmef-conformance/` suite claimed for 0.9.0-rc,
  which was never published.
- **RFC-0001 — recognition provenance, review status and as-built data (schema 0.10.0,
  backward compatible).** Every object that carries `revision` may now carry three optional
  blocks ([spec 02 §2.9](spec/02-information-model.md), [RFC](spec/rfc/0001-provenance-asbuilt.md)):
  - `provenance` — `sourceType` (`manual`, `cad-import`, `iso-drawing`, `2d-drawing`, `scan`,
    `photo`), `sourceRef` URI (document page/region or point-cloud segment), `recognizer`
    (name + version), `confidence` 0..1, `createdAt`;
  - `review` — `detected` / `reviewed` / `approved` / `rejected` with `reviewedBy`/`reviewedAt`;
    schema-enforced ISO 19650 rule: `detected` ⇒ `WIP`, `rejected` ⇒ not `PUBLISHED`;
  - `asBuilt` — `pointCloudRef`, `fitMethod`, `deviationRms_mm`, `deviationMax_mm`, and
    `catalogSnap` (catalog id **+ version** + entry, snapped size, per-parameter residuals).
  - Schemas: `pmef-base` `$defs` `Provenance`, `ReviewStatus`, `AsBuilt`, `CatalogSnap`,
    `ReviewChangeStateRule`, referenced from all 20 object definitions. `$id` paths unchanged.
  - Rust: `pmef-core::provenance` types, flattened into all 27 object structs.
  - Example: `examples/asbuilt-recognition.ndjson` — DN 150 pipe run recognised from a scan and
    an isometric, snapped to `piping-class-en-p1a` 2024.1; HEB 300 beam snapped to `profiles-en`
    2024.1; one `detected`, one `reviewed`, one `rejected` object (synthetic, CC0).
  - Tests: Rust positive/negative schema tests (`pmef-validate/tests/provenance.rs`) and a lossless
    serde round trip of every example object (`pmef-core/tests/provenance_roundtrip.rs`).
  - CAEX/AutomationML mapping documented in the RFC.

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

> **Note (2026-10-31):** `pmef-conformance/` is not part of this repository and does not appear
> in its Git history; the figures below cannot be reproduced from the published sources.
> See [docs/legacy-docx-review.md](docs/legacy-docx-review.md). The executable suite now lives in
> [`tests/`](tests/README.md) (see [Unreleased]).

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
