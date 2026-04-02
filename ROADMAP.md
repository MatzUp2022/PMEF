# PMEF Roadmap

This roadmap describes the planned milestones for the PMEF specification and
reference implementation. Dates are targets, not commitments.

> **Legend:** ✅ Done · 🔄 In Progress · 🗓️ Planned · 💭 Future

---

## Milestone Overview

```text
2026 Q2          2026 Q3          2026 Q4          2027 Q1
   ▼                ▼                ▼                ▼
[v0.9 RC]       [v1.0 MVP-1]     [v1.1 MVP-2]    [v1.2 MVP-3]
 Schema+Spec     4 Adapters       Mech.CAD+EI      Steel+Stress
 Foundation      Piping+Equip     Full Spec        AAS+ERP
```

---

## v0.9 — Specification Foundation *(current)*

**Target:** 2026 Q2 · **Status:** 🔄 In Progress

### Specification

- ✅ Five-layer architecture defined
- ✅ JSON Schema Draft 2020-12 for Piping domain (PipingNetworkSystem → 12 component types)
- ✅ JSON Schema for Equipment domain (10 subtypes incl. Pump, Vessel, HX, Column, Reactor)
- ✅ Parametric geometry library (15 primitive types)
- ✅ Property sets aligned to CFIHOS V2.0
- ✅ ISO 15926-14/IDO alignment documented
- ✅ NDJSON serialisation specification
- ✅ PMEF-DS-01 benchmark example (pump skid)
- 🔄 Geometry schema: OpenUSD schema extension (`pmef:` custom schema)
- 🔄 Relationships schema (`pmef-relationships.schema.json`)
- 🔄 Catalog schema for piping specs + steel profiles

### Community

- ✅ GitHub repository structure
- ✅ README, CONTRIBUTING, ROADMAP, GOVERNANCE
- ✅ Issue templates (bug, feature, RFC)
- ✅ CI/CD pipeline (schema validation)
- 🔄 Discord server setup
- 🔄 First working group sessions (WG-Piping, WG-Equipment)

---

## v1.0 — MVP-1: Four Core Adapters *(Piping + Equipment)*

**Target:** 2026 Q3 · **Status:** 🗓️ Planned

### New schema domains

- 🗓️ `pmef-relationships.schema.json` — typed relationship objects
- 🗓️ `pmef-catalog.schema.json` — piping spec + steel profile catalog format
- 🗓️ PMEF-DS-02 example: heat exchanger station
- 🗓️ PMEF-DS-05 example: EAF cooling circuit segment

### Reference implementation (`pmef-core` Rust crate)

- 🗓️ `pmef-core` — data model structs (generated from JSON Schema)
- 🗓️ `pmef-io` — NDJSON reader/writer (streaming, async)
- 🗓️ `pmef-validate` — JSON Schema validation + topology checks
- 🗓️ `pmef-geom` — bounding box, primitive intersection, basic clash

### Adapters (v1.0 target: bidirectional, Piping + Equipment)

- 🗓️ **`pmef-adapter-plant3d`** — AutoCAD Plant 3D via Plant SDK + PCF bridge
- 🗓️ **`pmef-adapter-cadmatic`** — CADMATIC via REST Web API (Swagger)
- 🗓️ **`pmef-adapter-e3d`** — AVEVA E3D via RVM export + rvmparser + PML scripting
- 🗓️ **`pmef-adapter-openplant`** — Bentley OpenPlant via ISO 15926/iRING API

### CLI + tooling

- 🗓️ `pmef-cli` — `validate`, `convert`, `diff`, `info` subcommands
- 🗓️ Docker image: `ghcr.io/pmef/pmef-cli:1.0`
- 🗓️ VS Code extension: schema-based autocomplete + inline validation

### Conformance

- 🗓️ Conformance test suite v1.0 (50+ round-trip test cases)
- 🗓️ Conformance level definitions: **Basic** / **Full** / **Round-Trip**

---

## v1.1 — MVP-2: Mechanical CAD + E&I *(Q4 2026)*

**Target:** 2026 Q4

### New schema domains

- 🗓️ `pmef-ei.schema.json` — InstrumentObject, InstrumentLoop, PLCObject, CableObject
- 🗓️ `pmef-steel.schema.json` — SteelObject with CIS/2 mapping
- 🗓️ E&I property sets: SIL, actuator, PA-DIM DeviceType reference
- 🗓️ MTP 2.0 PEA-module entity

### New adapters

- 🗓️ **`pmef-adapter-creo`** — PTC Creo via STEP AP242 (Creo Toolkit)
- 🗓️ **`pmef-adapter-inventor`** — Autodesk Inventor 2026 via IFC4 export
- 🗓️ **`pmef-adapter-catia`** — CATIA V5/V6 via 3DXML + STEP AP242
- 🗓️ **`pmef-adapter-eplan`** — EPLAN Electric P8 via AML + EplanEplApi
- 🗓️ **`pmef-adapter-comos`** — Siemens COMOS via SQL API + AML + DEXPI

### Reference implementation

- 🗓️ `pmef-eplan` — EPLAN AML parser, IEC 81346 BKZ normaliser
- 🗓️ `pmef-tia` — TIA Portal Openness wrapper (AML AR APC parser)
- 🗓️ `pmef-gltf` — glTF 2.0 export with EXT_mesh_features

### Spec completion

- 🗓️ Chapter 05 Adapters — complete normative text
- 🗓️ Chapter 06 Conformance — final conformance matrix

---

## v1.2 — MVP-3: Steel + Stress + AAS *(Q1 2027)*

**Target:** 2027 Q1

### New adapters

- 🗓️ **`pmef-adapter-tekla`** — Tekla Structures via Tekla Open API + CIS/2
- 🗓️ **`pmef-adapter-advance-steel`** — Advance Steel via IFC 2x3 + SDNF
- 🗓️ **`pmef-adapter-smart3d`** — Hexagon Smart 3D via .NET API + SQL Server
- 🗓️ **`pmef-adapter-revit`** — Revit MEP + Structural via IFC4 + IfcOpenShell

### Stress analysis module

- 🗓️ `pmef-stress` — `.cii` (CAESAR II) + `.ntr` (ROHR2) generator + parser
- 🗓️ PMEF-DS-03 benchmark: compressor train with stress analysis

### Digital Twin / ERP

- 🗓️ `pmef-aas-extended` — PMEF AAS submodels for Piping, Equipment, E&I
- 🗓️ `pmef-sap` — SAP EAM OData REST adapter
- 🗓️ AAS AASX container as PMEF package alternative

### Spec additions

- 🗓️ Chapter 07 Digital Twin — OPC UA, AAS, simulation interfaces
- 🗓️ Chapter 08 ERP/EAM integration patterns

---

## v1.5 — Simulation + Full Ecosystem *(Q2 2027)*

**Target:** 2027 Q2

- 💭 `pmef-usd` — OpenUSD export (pmef: custom schema, SimReady assets)
- 💭 `pmef-sim` — Plant Simulation X / AnyLogic / Emulate3D layout adapters
- 💭 `pmef-capopen` — CAPE-OPEN design conditions import
- 💭 `pmef-bcf` — BCF 3.0 issue protocol writer/reader
- 💭 Civil domain schema + ArchiCAD / Allplan IFC adapter
- 💭 PMEF web viewer (xeokit-based, open source)
- 💭 PMEF cloud validation service (REST API)

---

## v2.0 — Long Term *(2028+)*

- 💭 IFC 5 compatibility layer
- 💭 AI-assisted model migration (PMEF ↔ native tool formats)
- 💭 Real-time collaborative editing protocol
- 💭 PMEF as IDTA submodel template submission
- 💭 ISO standardisation pathway (via DEXPI / ISO TC 184)

---

## How to influence the roadmap

1. **Vote** on existing issues with 👍
2. **Open a feature request** using the [feature template](.github/ISSUE_TEMPLATE/feature_request.md)
3. **Join a working group** — WG leads have direct input into milestone planning
4. **Sponsor development** — contact [pmef@example.org](mailto:pmef@example.org)

The TSC reviews and updates this roadmap quarterly.
Last updated: **2026-03-31** · TSC approval: pending
