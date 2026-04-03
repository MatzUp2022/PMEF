# CHANGELOG

All notable changes to the PMEF Specification are documented here.
Adheres to [Semantic Versioning](https://semver.org/) and [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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

## [Unreleased] — v0.9.1 Planned

- Fix EQUIP-U-005 and PIPE-U-003 schema/test gaps
- DS-05 benchmark (Mixed EAF + E&I + Steel, ~80 objects)
- `pmef-adapter-cadmatic`: CADMATIC REST API adapter
- `pmef-stress`: CAESAR II .cii import/export adapter (Rust)
- Full `pmef-geom` AABB computation (all 15 primitives)
- Full `pmef-validate` jsonschema Rust integration

## [Unreleased] — v1.0 Planned

- `cargo test --workspace` green (all Rust tests pass)
- Published crates on crates.io
- Published schemas on schema.pmef.org
- pmef.org website with full documentation
- First vendor Level 2 certification
