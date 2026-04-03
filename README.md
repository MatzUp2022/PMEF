<div align="center">

<img src="docs/assets/pmef-logo.svg" width="120" alt="PMEF Logo"/>

# PMEF — Plant Model Exchange Format

<!-- markdownlint-disable MD036 -->
**The open, cross-discipline 3D plant model exchange format**
<!-- markdownlint-enable MD036 -->

[![Spec Version](https://img.shields.io/badge/spec-v0.9--rc-blue?style=flat-square)](CHANGELOG.md)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-green?style=flat-square)](LICENSE-APACHE)
[![License: CC BY 4.0](https://img.shields.io/badge/Spec-CC%20BY%204.0-green?style=flat-square)](LICENSE-CC-BY)
[![Schema CI](https://img.shields.io/github/actions/workflow/status/pmef/specification/validate-schemas.yml?label=schemas&style=flat-square)](../../actions)
[![Discord](https://img.shields.io/badge/community-Discord-7289DA?style=flat-square)](https://discord.gg/pmef)
[![OpenSSF Best Practices](https://img.shields.io/badge/OpenSSF-Best%20Practices-orange?style=flat-square)](https://bestpractices.coreinfrastructure.org)

[**Specification**](spec/) · [**JSON Schemas**](schemas/) · [**Examples**](examples/) ·
[**Adapters**](adapters/) · [**Roadmap**](ROADMAP.md) · [**Discord**](https://discord.gg/pmef)

</div>

---

> **This project is under active development and not yet production-ready.**
> All specifications, schemas, and catalogs are subject to change without notice.
> **Use at your own risk.** We are actively looking for **supporters and contributors** —
> whether you are an engineer, developer, standards expert, or domain specialist,
> your input is welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) or join our
> [Discord](https://discord.gg/pmef) to get involved.

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
| **PMEF** | **NDJSON + PMEFX** | **✅ 100%** | **✅** | **✅** |

PMEF provides:

- **Canonical information model** grounded in ISO 15926-14/IDO, DEXPI 2.0, CFIHOS V2.0
- **Parametric geometry library** (15 primitive types, RVM-inspired)
- **Three geometry layers**: parametric primitives · glTF 2.0 mesh · STEP AP242 B-Rep
- **Git-friendly NDJSON serialisation** with object-level versioning
- **Bidirectional adapters** for 14+ engineering tools across 8 disciplines
- **Apache 2.0 Rust core library** + thin adapter plugins

### Disciplines covered

```text
Piping · Equipment · Structural Steel · E&I · Pipe Stress · ERP/EAM · Simulation · Civil
```

---

## Quick Start

### Validate an existing PMEF file

```bash
# Using the PMEF CLI (once published to crates.io)
cargo install pmef-cli
pmef validate my-plant.ndjson

# Or with Docker
docker run --rm -v $(pwd):/data ghcr.io/pmef/pmef-cli:latest validate /data/my-plant.ndjson
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
  pmef-base.schema.json            — shared primitives (IDs, coordinates, revisions)
  pmef-piping-component.schema.json— PipingNetworkSystem → PipingSegment → 12 component types
  pmef-equipment.schema.json       — 10 equipment types (Vessel, Pump, HX, Column, Reactor…)
  pmef-geometry.schema.json        — 15 parametric primitives
  pmef-property-sets.schema.json   — all property-set definitions (CFIHOS-aligned)
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
           ↑↓ bidirectional adapters
┌──────────┬──────────┬──────────┬──────────┬─────────┐
│AVEVA E3D │Smart 3D  │Plant 3D  │CADMATIC  │OpenPlant│
│Tekla     │Revit     │EPLAN P8  │COMOS     │INOSIM   │
│Creo      │CATIA     │TIA Portal│SAP EAM   │Emulate3D│
└──────────┴──────────┴──────────┴──────────┴─────────┘
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
pmef/specification
├── CHANGELOG.md               — version history
├── CONTRIBUTING.md            — how to contribute
├── ROADMAP.md                 — milestone plan
├── GOVERNANCE.md              — TSC and working group rules
├── CODE_OF_CONDUCT.md         — contributor covenant
│
├── spec/                      — normative specification (Markdown)
│   ├── 01-introduction.md
│   ├── 02-information-model.md
│   ├── 03-serialisation.md
│   ├── 04-geometry.md
│   ├── 05-adapters.md
│   └── 06-conformance.md
│
├── schemas/                   — JSON Schema (Draft 2020-12)
│   ├── pmef-base.schema.json
│   ├── pmef-piping-component.schema.json
│   ├── pmef-equipment.schema.json
│   ├── pmef-geometry.schema.json
│   └── pmef-property-sets.schema.json
│
├── diagrams/                  — Mermaid UML diagrams
│
├── examples/                  — annotated NDJSON reference instances
│   ├── pump-skid-complete.ndjson    (PMEF-DS-01)
│   ├── heat-exchanger-station.ndjson(PMEF-DS-02)
│   └── eaf-segment.ndjson           (PMEF-DS-05)
│
├── adapters/                  — adapter specifications (code in separate repos)
│   ├── plant3d/
│   ├── cadmatic/
│   ├── e3d/
│   └── openplant/
│
├── catalog/                   — open piping spec + profile catalogs
│   ├── profiles-en.json
│   ├── profiles-aisc.json
│   └── piping-class-a1a2.json
│
├── docs/                      — design docs and guides
│   ├── design-decisions.md
│   ├── iso15926-mapping.md
│   ├── dexpi-alignment.md
│   └── getting-started.md
│
└── .github/
    ├── workflows/             — CI/CD pipelines
    ├── ISSUE_TEMPLATE/        — bug, feature, RFC templates
    └── PULL_REQUEST_TEMPLATE/ — PR checklist
```

---

## Standards Alignment

PMEF is designed to complement, not replace, existing standards:

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
- 💬 [Join the Discord](https://discord.gg/pmef)
- 📋 [Good first issues](../../issues?q=is%3Aopen+label%3A%22good+first+issue%22)

---

## Licence

| Artefact | Licence |
|----------|---------|
| Specification (Markdown, schemas, diagrams) | [CC BY 4.0](LICENSE-CC-BY) |
| Reference implementation (Rust crates) | [Apache 2.0](LICENSE-APACHE) |
| Examples and test data | [CC0 1.0](LICENSE-CC0) — no rights reserved |

© 2026 PMEF Contributors
