<div align="center">

# PMEF — Plant Model Exchange Format

<!-- markdownlint-disable MD036 -->
**The open, cross-discipline 3D plant model exchange format**
<!-- markdownlint-enable MD036 -->

[![Spec Version](https://img.shields.io/badge/spec-0.9.0%20(RC)-blue?style=flat-square)](CHANGELOG.md)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-green?style=flat-square)](LICENSE-APACHE)
[![License: CC BY 4.0](https://img.shields.io/badge/Spec-CC%20BY%204.0-green?style=flat-square)](LICENSE-CC-BY)
[![Schema CI](https://img.shields.io/github/actions/workflow/status/MatzUp2022/PMEF/validate-schemas.yml?label=schemas&style=flat-square)](https://github.com/MatzUp2022/PMEF/actions/workflows/validate-schemas.yml)
[![Rust CI](https://img.shields.io/github/actions/workflow/status/MatzUp2022/PMEF/rust.yml?label=rust&style=flat-square)](https://github.com/MatzUp2022/PMEF/actions/workflows/rust.yml)

[**Specification**](spec/) · [**JSON Schemas**](schemas/) · [**Examples**](examples/) ·
[**Reference implementation**](pmef-rs/) · [**Roadmap**](ROADMAP.md)

</div>

---

> **This project is under active development and not yet production-ready.**
> All specifications, schemas, and catalogs are subject to change without notice.
> **Use at your own risk.** We are actively looking for **supporters and contributors** —
> whether you are an engineer, developer, standards expert, or domain specialist,
> your input is welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) or open an
> [issue](https://github.com/MatzUp2022/PMEF/issues).

---

## What is PMEF?

PMEF is an **open, semantically rich, cross-discipline 3D plant model exchange format** for industrial
facilities. It solves a problem that has cost the engineering industry billions in rework: every major
plant design tool uses a completely isolated, proprietary data format.

| Tool | Format | Open? | 3D + Semantics? | Cross-discipline? |
|------|--------|-------|-----------------|-------------------|
| AVEVA E3D | RVM / Dabacon DB | Partial | ✅ | ❌ |
| Hexagon Smart 3D | VUE / SQL Server | ❌ | ✅ | ❌ |
| AutoCAD Plant 3D | PCF / DWG | Partial | Partial | ❌ |
| CADMATIC | 3DDX / DGN | ❌ | ✅ | ❌ |
| Bentley OpenPlant | DGN / iModel | Partial | ✅ | Partial |
| **PMEF** | **NDJSON** (+ PMEFX container, specified) | **✅ 100%** | **✅** | **✅** |

PMEF provides:

- **Information model** designed for alignment with ISO 15926-14/IDO, DEXPI 2.0 and CFIHOS V2.0
  (alignment is documented in prose; machine-checked mappings are planned — see [ROADMAP](ROADMAP.md))
- **Parametric geometry library** (15 primitive types)
- **Three geometry layers**: parametric primitives · glTF 2.0 mesh · STEP AP242 B-Rep
- **Git-friendly NDJSON serialisation** with object-level versioning
- **Apache 2.0 Rust reference implementation**: core model, NDJSON I/O, schema validation,
  CLI, and export adapters for 10 tools (see [status](#implementation-status))

### Disciplines

Schemas exist for Piping, Equipment, Structural Steel and E&I; a pipe-stress benchmark example
is included. **Current focus: Piping, Structural Steel, Equipment.** Further disciplines
(ERP/EAM, Simulation, Civil) are on the [roadmap](ROADMAP.md), not implemented.

---

## Quick Start

### Validate an existing PMEF file

```bash
# Build the CLI from source (not yet published on crates.io; no container image yet)
cargo install --path pmef-rs/pmef-cli
pmef validate my-plant.ndjson --schemas schemas

# Or, without Rust, the Python checks used in CI
pip install -r requirements.txt
python scripts/validate-examples.py
```

### View the example

The [examples/pump-skid-complete.ndjson](examples/pump-skid-complete.ndjson) contains
a fully annotated pump skid (P-201A + V-201 + lines CW-201/202) with:

- `pmef:Pump` with API 610 spec, 3 nozzles, VFD drive
- `pmef:Vessel` with EN 13445 design, composite geometry
- Full piping run: Pipe → Elbow → Flange → Gasket → Valve
- Weld records, pipe support, fabrication spool

```jsonc
// Single object per line (NDJSON)
{"@type":"pmef:Pump","@id":"urn:pmef:obj:proj:P-201A","pmefVersion":"0.9.0",
 "equipmentBasic":{"tagNumber":"P-201A","equipmentClass":"CENTRIFUGAL_PUMP"},
 "pumpSpec":{"designFlow":450.0,"designHead":65.0,"motorPower":132.0,...},
 "nozzles":[{"nozzleId":"SUCTION","nominalDiameter":200,...}],
 "geometry":{"type":"parametric","ref":"urn:pmef:geom:proj:P-201A-prim"}}
```

### Explore the schemas

All schemas live in [`schemas/`](schemas/). Open them in any JSON Schema-aware editor
([VS Code](https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml),
[Insomnia](https://insomnia.rest/), [Stoplight Studio](https://stoplight.io/)) for autocomplete.

```text
schemas/
  pmef-base.schema.json             — shared primitives (IDs, coordinates, revisions)
  pmef-hierarchy.schema.json        — FileHeader, Plant, Unit, Area
  pmef-piping-component.schema.json — PipingNetworkSystem → PipingSegment → 11 component types
  pmef-equipment.schema.json        — 10 equipment types (Vessel, Pump, HX, Column, Reactor…)
  pmef-steel.schema.json            — SteelSystem, SteelMember, SteelNode, SteelConnection
  pmef-ei.schema.json               — instruments, loops, PLC, cables, cable trays, MTP modules
  pmef-relationships.schema.json    — typed relationships (IsConnectedTo, HasEquivalentIn, …)
  pmef-geometry.schema.json         — 15 parametric primitives
  pmef-property-sets.schema.json    — property-set definitions (CFIHOS-aligned)
  pmef-catalog.schema.json          — piping classes, profiles, dimensions, materials, RDL maps
```

---

## Architecture

```text
┌─────────────────────────────────────────────────────┐
│                  PMEF Package (.pmefx)               │
│                                                     │
│  model/           geometry/         catalogs/        │
│  ├─ piping.ndjson ├─ parametric.ndjson ├─ piping-spec│
│  ├─ equipment.ndjson├─ model.glb    └─ profiles      │
│  ├─ structural.ndjson├─ model.usdc                   │
│  └─ ei.ndjson     └─ model.stp      documents/       │
└─────────────────────────────────────────────────────┘
           ↑ export adapters (Rust reference implementation)
┌──────────────┬──────────────┬──────────────┬──────────────┬──────────────┐
│AVEVA E3D ★   │Plant 3D ★    │CADMATIC      │COMOS         │Tekla         │
│Revit         │Advanced Steel│Navisworks    │Inventor      │Creo          │
└──────────────┴──────────────┴──────────────┴──────────────┴──────────────┘
  ★ = active focus · all others frozen · import direction (PMEF → tool) not implemented
```

### Five-layer architecture

| Layer | Content | Format |
|-------|---------|--------|
| **1 — Core Ontology** | ISO 15926-14/IDO subset, DEXPI 2.0, IEC 81346, CFIHOS | OWL 2 (normative), JSON Schema |
| **2 — Information Model** | Entity types, relationships, property sets | JSON Schema Draft 2020-12 + UML |
| **3 — Serialisation** | NDJSON (primary), CAEX XML (secondary), AASX container | RFC 7464, IEC 62424 |
| **4 — Geometry** | Parametric primitives, glTF 2.0 mesh, STEP AP242 B-Rep, OpenUSD | PMEF spec, Khronos, ISO 10303 |
| **5 — Adapters** | Tool-specific import/export plugins | Rust crates (Apache 2.0) |

---

## Repository Structure

```text
MatzUp2022/PMEF
├── CHANGELOG.md · CONTRIBUTING.md · ROADMAP.md · GOVERNANCE.md · CODE_OF_CONDUCT.md
├── CLAUDE.md                   — contributor/agent working rules and verified commands
├── THIRD-PARTY-NOTICES.md
│
├── spec/                       — normative specification (Markdown, chapters 01–06; RFCs in spec/rfc/)
├── schemas/                    — JSON Schema Draft 2020-12 (10 files, normative with spec/)
├── examples/                   — NDJSON benchmark datasets (CC0)
│   ├── pump-skid-complete.ndjson       (PMEF-DS-01)
│   ├── heat-exchanger-station.ndjson   (PMEF-DS-02)
│   ├── compressor-train.ndjson         (PMEF-DS-03)
│   ├── ei-loop-fic-10101.ndjson        (PMEF-DS-04)
│   ├── stress-model-cw201.ndjson       (pipe-stress example)
│   └── asbuilt-recognition.ndjson      (RFC-0001 provenance / review / as-built, 0.10.0)
├── catalogs/                   — open piping-class, profile, dimension and material catalogs
├── diagrams/                   — Mermaid UML diagrams
├── docs/                       — design decisions, getting started, ISO 15926 mapping, audit
├── scripts/                    — Python validation and release helpers (run in CI)
│
├── pmef-rs/                    — Rust workspace (Apache 2.0)
│   ├── pmef-core · pmef-io · pmef-validate · pmef-geom · pmef-cli
│   └── pmef-adapter-{e3d, plant3d, cadmatic, comos, creo, inventor,
│                     navisworks, revit, tekla, advanced-steel}
├── pmef-plant3d-dotnet/        — AutoCAD Plant 3D host plug-in (C#)
├── pmef-comos-dotnet/ · pmef-tekla-dotnet/ · pmef-inventor-addin/ · pmef-dotnet-adapters/
├── pmef-creo-toolkit/          — Creo Toolkit plug-in (C)
│
├── PMEF_CHANGELOG_v0.9.0-rc.docx           — legacy, non-normative
├── PMEF_Specification_v1.0-draft_DE.docx   — legacy German draft, non-normative
│
└── .github/                    — CI workflows, issue templates, PR template
```

The `.docx` files are **non-normative legacy artefacts**. Where they differ from `spec/` and
`schemas/`, the Markdown specification and the schemas win.

---

## Implementation Status

| Component | Status |
|-----------|--------|
| Specification (`spec/`) + schemas | 0.9.0 release candidate; schemas and examples validated in CI |
| Recognition provenance / review / as-built ([RFC-0001](spec/rfc/0001-provenance-asbuilt.md)) | in schemas, spec 02 §2.9 and `pmef-core` (0.10.0, draft for review) |
| `pmef-core`, `pmef-io`, `pmef-validate`, `pmef-geom` | builds and tests green in CI |
| `pmef` CLI | `validate`, `stats`, `index`, `check-refs`, `topology` work; `diff` and `conformance` are basic; `convert` not implemented |
| Adapters (Rust) | all 10 compile and pass their unit tests; export direction only; real-tool round-trips not yet verified |
| Host plug-ins (C#, C) | source only, not built in CI |
| PMEFX container | specified (spec 03 §3), not implemented |

Details and known gaps: [docs/AUDIT.md](docs/AUDIT.md).

---

## Standards Alignment

PMEF is designed to complement, not replace, existing standards. The table describes the
**intended** relationship; conversion tooling (DEXPI, IFC, CAEX, AAS, CFIHOS export) is not yet
implemented unless stated in [Implementation Status](#implementation-status).

| Standard | Relationship to PMEF |
|----------|---------------------|
| **DEXPI 2.0** | PMEF extends DEXPI — adds 3D geometry + cross-discipline; imports DEXPI P&ID entities |
| **ISO 15926-14/IDO** | PMEF's upper ontology; `rdlType` URIs resolve via PCA-RDL SPARQL endpoint |
| **CFIHOS V2.0** | PMEF property sets aligned to CFIHOS; PMEF exports CFIHOS-compatible handover packages |
| **IFC 4.3** | PMEF imports/exports IFC via IfcOpenShell; complements IFC for process plant specifics |
| **AutomationML/CAEX** | PMEF's XML serialisation is CAEX-compatible; AML AR APC for E&I/PLC data |
| **IEC 81346** | PMEF's `iec81346` designation block uses IEC 81346 aspects (=, -, +) |
| **ISO 19650** | PMEF adopts ISO 19650 CDE workflow (WIP→SHARED→PUBLISHED→ARCHIVED) |
| **AAS / IEC 63278** | PMEF exports AAS submodels; PMEF-DEXPI submodel (IDTA 02016) aligned |

---

## Contributing

We welcome contributions of all kinds — see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

**Quick links:**

- 🐛 [Report a bug](.github/ISSUE_TEMPLATE/bug_report.md)
- 💡 [Propose a feature](.github/ISSUE_TEMPLATE/feature_request.md)
- 📝 [Submit an RFC](.github/ISSUE_TEMPLATE/rfc.md)
- 📋 [Open issues](https://github.com/MatzUp2022/PMEF/issues)

---

## Licence

| Artefact | Licence |
|----------|---------|
| Specification (Markdown, schemas, diagrams) | [CC BY 4.0](LICENSE-CC-BY) |
| Reference implementation (Rust crates) | [Apache 2.0](LICENSE-APACHE) |
| Examples and test data | [CC0 1.0](LICENSE-CC0) — no rights reserved |

© 2026 PMEF Contributors
