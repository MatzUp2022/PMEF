# PMEF Specification · Chapter 01 · Introduction

**Document number:** PMEF-SPEC-01  
**Version:** 0.9.0-rc  
**Status:** Release Candidate  
**Date:** 2026-03-31

---

## Table of Contents

1. [Scope](#1-scope)
2. [Normative References](#2-normative-references)
3. [Terms and Definitions](#3-terms-and-definitions)
4. [Abbreviated Terms](#4-abbreviated-terms)
5. [Problem Statement](#5-problem-statement)
6. [Design Goals and Non-Goals](#6-design-goals-and-non-goals)
7. [Overview of PMEF Architecture](#7-overview-of-pmef-architecture)
8. [Relationship to Other Standards](#8-relationship-to-other-standards)
9. [Licensing and Governance](#9-licensing-and-governance)

---

## 1 Scope

This specification defines the **Plant Model Exchange Format (PMEF)**, an open, semantically rich, cross-discipline data exchange format for industrial plant models.

PMEF specifies:

- An **information model** grounded in ISO 15926-14 (Industrial Data Ontology), DEXPI 2.0, CFIHOS V2.0, and IEC 81346, covering eight engineering disciplines: Piping, Equipment, Structural Steel, Electrical and Instrumentation, Pipe Stress Analysis, Process Simulation, ERP/EAM, and Civil/Architecture.
- A **parametric geometry library** of 15 primitive types for lossless 3D shape representation at five levels of detail.
- A **serialisation format** based on Newline-Delimited JSON (NDJSON) as the primary encoding, with CAEX XML as a secondary encoding.
- A **typed relationship model** for explicit, versioned cross-discipline links.
- A **conformance framework** with three levels (PMEF-Basic, PMEF-Full, PMEF-RoundTrip) and a normative test suite.

PMEF does **not** define:

- Proprietary tool-internal data structures.
- Process simulation models or PFDs (process flow diagrams).
- Business logic for engineering workflows.
- Authentication, authorisation, or access control mechanisms.

### 1.1 Intended Audience

This specification is intended for:

- Software developers implementing PMEF readers, writers, and adapters.
- Engineering tool vendors seeking interoperability with PMEF.
- Systems integrators designing multi-tool plant engineering environments.
- Standards bodies and organisations seeking to align with or reference PMEF.

Plant engineers and project managers may find the [Getting Started Guide](../docs/getting-started.md) more accessible.

### 1.2 Document Structure

The PMEF specification consists of the following chapters:

| Chapter | Title | Normative? |
|---------|-------|-----------|
| 01 | Introduction (this document) | Informative |
| 02 | Information Model | **Normative** |
| 03 | Serialisation | **Normative** |
| 04 | Geometry | **Normative** |
| 05 | Adapters | **Normative** |
| 06 | Conformance | **Normative** |

Supporting documents:

| Document | Purpose |
|----------|---------|
| `docs/design-decisions.md` | Records architectural decisions (informative) |
| `docs/iso15926-mapping.md` | ISO 15926 / DEXPI alignment tables (informative) |
| `docs/getting-started.md` | Tutorial introduction (informative) |
| `diagrams/` | Mermaid UML diagrams (informative) |

---

## 2 Normative References

The following documents are referenced normatively in this specification. For dated references, only the edition cited applies.

| Reference | Title |
|-----------|-------|
| **ISO 15926-2:2003** | Industrial automation systems and integration — Integration of life-cycle data for process plants including oil and gas production facilities — Part 2: Data model |
| **ISO 15926-4:2019** | Part 4: Initial reference data |
| **ISO 15926-14:2022** | Part 14: Industrial Data Ontology (IDO) |
| **ISO 19650-1:2018** | Organization and digitization of information about buildings and civil engineering works, including building information modelling (BIM) — Part 1: Concepts and principles |
| **ISO 19650-2:2018** | Part 2: Delivery phase of assets |
| **IEC 81346-1:2022** | Industrial systems, installations and equipment and industrial products — Structuring principles and reference designations — Part 1: Basic rules |
| **IEC 62424:2016** | Representation of process control engineering — Requests in P&I diagrams and data exchange between P&ID tools and PCE-CAE tools (DEXPI) |
| **IEC 61508-1:2010** | Functional safety of E/E/PE safety-related systems — Part 1: General requirements |
| **IEC 61511-1:2016** | Functional safety — Safety instrumented systems for the process industry sector — Part 1: Framework, definitions, system, hardware and application programming requirements |
| **IEC 63278-1:2023** | Asset Administration Shell — Part 1: Metamodel |
| **ASME B31.3:2022** | Process Piping |
| **API 610:2022** | Centrifugal Pumps for Petroleum, Petrochemical and Natural Gas Industries, 13th ed. |
| **DEXPI 2.0:2023** | DEXPI Specification 2.0 (DEXPI e.V.) |
| **CFIHOS V2.0:2020** | Capital Facilities Information Hand-Over Specification (IOGP JIP33 S-616) |
| **RFC 7464:2015** | JavaScript Object Notation (JSON) Text Sequences (NDJSON) |
| **JSON Schema Draft 2020-12** | JSON Schema: A Vocabulary for Annotating and Validating JSON Documents |
| **glTF 2.0:2017** | GL Transmission Format (Khronos Group) |
| **ISO 10303-242:2022** | STEP AP242 — Managed model-based 3D engineering |
| **OpenUSD 24.11** | Universal Scene Description Specification (Alliance for OpenUSD) |
| **MTP 2.0:2023** | Module Type Package (VDI/VDE/NAMUR 2658, IEC 63280) |
| **OPC UA Part 6:2022** | OPC Unified Architecture — Part 6: Mappings |
| **PA-DIM (OPC 30500):2024** | Process Automation — Device Information Model |

### 2.1 Informative References

| Reference | Title |
|-----------|-------|
| AutomationML 2.10 | AutomationML — Engineering Data Exchange Format (IEC 62714) |
| AASX 1.0 | Asset Administration Shell Package (IDTA) |
| CIS/2 2.x | CIMsteel Integration Standards Version 2 (Steel Construction Institute) |
| ISO 10628-2:2012 | Flow diagrams for process plants — Part 2: Graphical symbols |
| RVM | PDMS Review Model Format (AVEVA) |
| PCF | Piping Component File Format (various) |

---

## 3 Terms and Definitions

For the purposes of this document, the following terms and definitions apply. Where a term is defined in a referenced standard, that definition applies unless otherwise stated.

**3.1 adapter**  
A software component that translates between a PMEF package and the native format of a specific engineering tool. Adapters may be bidirectional (export and import) or unidirectional.

**3.2 asset**  
A physical item of plant or equipment that is described by a PMEF object.

**3.3 benchmark dataset**  
A normative NDJSON file containing a representative selection of PMEF objects, used for conformance testing. Identified as PMEF-DS-NNN.

**3.4 catalog reference**  
A link from a PMEF object to an entry in a piping specification catalog, equipment class catalog, or steel profile catalog.

**3.5 change state**  
The ISO 19650 Common Data Environment (CDE) workflow status of a PMEF object: WIP, SHARED, PUBLISHED, or ARCHIVED.

**3.6 discipline**  
A branch of plant engineering, such as Piping, Equipment, Structural Steel, or Electrical and Instrumentation.

**3.7 entity type**  
A named class of PMEF objects, identified by the `@type` property (e.g. `pmef:Pump`, `pmef:PipingNetworkSystem`).

**3.8 functional object**  
An object that represents a function or service in the P&ID or process design, without specifying a physical realisation. Corresponds to ISO 15926-14 `FunctionalObject` and DEXPI functional entities.

**3.9 geometry layer**  
One of three complementary representations of the physical shape of a PMEF object: parametric primitives, glTF 2.0 mesh, or STEP AP242 B-Rep (and optionally OpenUSD).

**3.10 information model**  
The set of entity types, properties, property sets, and relationships that constitute the PMEF data model.

**3.11 level of detail (LOD)**  
A categorisation of geometry representation fidelity: BBOX_ONLY, LOD1_COARSE, LOD2_MEDIUM, LOD3_FINE, LOD4_FABRICATION.

**3.12 nozzle**  
A physical connection point on a piece of equipment, through which the equipment connects to a piping system. In PMEF, the nozzle acts as the cross-domain connector between the Equipment and Piping domains.

**3.13 NDJSON**  
Newline-Delimited JSON. A format for encoding sequences of JSON values, one per line, as defined in RFC 7464. Used as the primary PMEF serialisation format.

**3.14 PMEF object**  
An instance of a PMEF entity type, serialised as a single JSON object on one line of an NDJSON file. Every PMEF object has a globally unique `@id` and a `@type`.

**3.15 PMEF package**  
A collection of PMEF objects that together represent a plant model or a portion thereof. May be serialised as a single NDJSON file or as a PMEFX container.

**3.16 PMEFX**  
The PMEF package container format. A ZIP archive containing one or more NDJSON files plus associated geometry assets (glTF, STEP, USD) and a manifest.

**3.17 physical object**  
An object that represents a physical item of plant, as opposed to a functional object. Corresponds to ISO 15926-14 `InanimatePhysicalObject`.

**3.18 port**  
A named, typed connection point on a piping component. Ports carry geometric coordinates, direction vectors, and topology references (`connectedTo`).

**3.19 property set**  
A named, typed collection of attributes for a specific engineering purpose, such as `PipingDesignConditions` or `PumpSpec`.

**3.20 reference data library (RDL)**  
A repository of standardised class definitions, accessible via URI. In PMEF, the primary RDL is the PCA-RDL (ISO 15926-4 reference data), accessed via SPARQL endpoint.

**3.21 relationship object**  
A first-class PMEF object that explicitly models a typed relationship between two other PMEF objects, such as `pmef:IsDerivedFrom` or `pmef:ControlledBy`.

**3.22 revision**  
A specific version of a PMEF object, identified by `revisionId` and linked to its predecessor via `pmef:IsRevisionOf`.

**3.23 round-trip**  
The process of exporting a PMEF package from one tool, importing it into PMEF, and re-importing it into the same or a different tool, with verification that attribute fidelity meets the specified threshold.

**3.24 tag number**  
The identifier assigned to an instrument, piece of equipment, or pipeline in the engineering design documentation. Tag numbers follow conventions defined in ISA 5.1, ISO 10628, or project-specific tag numbering systems.

---

## 4 Abbreviated Terms

| Abbreviation | Meaning |
|-------------|---------|
| AAS | Asset Administration Shell |
| AML | AutomationML |
| ATEX | ATmosphères EXplosibles (EU Directive 2014/34/EU) |
| BIM | Building Information Modelling |
| CAEX | Computer Aided Engineering eXchange (AML schema) |
| CAPE-OPEN | Computer Aided Process Engineering – Open Platform standard |
| CDE | Common Data Environment (ISO 19650) |
| CFIHOS | Capital Facilities Information Hand-Over Specification |
| CIS/2 | CIMsteel Integration Standards Version 2 |
| DCS | Distributed Control System |
| DEXPI | Data EXchange in the Process Industry |
| EAF | Electric Arc Furnace |
| EAM | Enterprise Asset Management |
| EI&C | Electrical, Instrumentation and Control |
| ERP | Enterprise Resource Planning |
| FEA / FEM | Finite Element Analysis / Method |
| FMI | Functional Mock-up Interface |
| FMU | Functional Mock-up Unit |
| glTF | GL Transmission Format |
| HART | Highway Addressable Remote Transducer |
| HX | Heat Exchanger |
| IEC | International Electrotechnical Commission |
| IFC | Industry Foundation Classes (buildingSMART) |
| ISO | International Organization for Standardization |
| LOD | Level of Detail |
| MDMT | Minimum Design Metal Temperature |
| MTP | Module Type Package |
| NDE / NDT | Non-Destructive Examination / Testing |
| NDJSON | Newline-Delimited JSON |
| NPSH | Net Positive Suction Head |
| OPC UA | OPC Unified Architecture |
| P&ID | Piping and Instrumentation Diagram |
| PA-DIM | Process Automation Device Information Model |
| PCA | POSC Caesar Association |
| PCF | Piping Component File |
| PEA | Process Equipment Assembly (MTP) |
| PED | Pressure Equipment Directive (EU 2014/68/EU) |
| PMEF | Plant Model Exchange Format |
| PMEFX | PMEF Package Container Format |
| POL | Process Orchestration Layer (MTP) |
| PSV | Pressure Safety Valve |
| RDL | Reference Data Library |
| RVM | PDMS Review Model Format |
| SIL | Safety Integrity Level |
| STEP | Standard for the Exchange of Product model data |
| TEMA | Tubular Exchanger Manufacturers Association |
| URI | Uniform Resource Identifier |
| USD / OpenUSD | Universal Scene Description |
| VFD | Variable Frequency Drive |
| WPS | Welding Procedure Specification |

---

## 5 Problem Statement

### 5.1 The Interoperability Gap in Plant Engineering

Industrial plant engineering involves ten or more discipline-specific software tools per project, none of which shares a common data format. A typical large EPC project produces:

- P&ID models in DEXPI XML (from COMOS, AVEVA Diagrams, or similar)
- 3D piping and equipment models in proprietary databases (AVEVA E3D, Hexagon Smart 3D, AutoCAD Plant 3D, CADMATIC)
- Structural models in CIS/2 or IFC (Tekla Structures, Advance Steel, RFEM)
- E&I models in EPLAN or COMOS AML
- Piping stress models in CAESAR II (`.cii`) or ROHR2 (`.ntr`)
- Equipment datasheets in Excel or CFIHOS XML
- As-built data in ERP/EAM systems (SAP PM, IBM Maximo)

Translating data between these tools is manual, error-prone, and extremely expensive. Industry studies consistently estimate that 20–30% of engineering project costs are attributable to rework caused by data translation errors and model inconsistencies.

Existing standards address parts of this problem but none addresses the whole:

- **DEXPI 2.0** covers P&ID data but not 3D geometry or most physical properties.
- **ISO 15926** provides a formal ontology but is too complex for most tool implementations.
- **CFIHOS** defines handover attributes but not the exchange format or geometry.
- **IFC** addresses building and infrastructure but lacks process plant specifics (pipe stress, instrument loops, MTP).
- **PCF** handles piping component geometry but has no semantics and no equipment or E&I.

PMEF fills this gap by providing a practical, open, standard exchange format that is grounded in the above standards but adds the pragmatic detail needed for real-world interoperability.

### 5.2 Existing Format Landscape

The table below summarises the coverage of existing formats relevant to PMEF:

| Format | 3D Geometry | Piping | Equipment | E&I | Steel | Semantics | Open |
|--------|------------|--------|-----------|-----|-------|-----------|------|
| DEXPI 2.0 | ❌ | ✅ (topology) | Partial | Partial | ❌ | ✅ | ✅ |
| PCF | ✅ (coords) | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |
| IFC 4.3 | ✅ | Partial | Partial | ❌ | ✅ | Partial | ✅ |
| ISO 15926 | ❌ | ✅ | ✅ | Partial | ❌ | ✅ | ✅ |
| CFIHOS | ❌ | ✅ | ✅ | Partial | ❌ | Partial | ✅ |
| AutomationML | ❌ | ❌ | ❌ | ✅ | ❌ | Partial | ✅ |
| CIS/2 | ✅ | ❌ | ❌ | ❌ | ✅ | Partial | ✅ |
| RVM | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ | ❌ |
| **PMEF** | **✅** | **✅** | **✅** | **✅** | **✅** | **✅** | **✅** |

---

## 6 Design Goals and Non-Goals

### 6.1 Design Goals

**G-01 Open and non-proprietary.** The specification, schemas, and reference implementation are published under open licences (CC BY 4.0 and Apache 2.0). No patent encumbrances.

**G-02 Semantically grounded.** Every PMEF entity type is grounded in an upstream standard (ISO 15926, DEXPI 2.0, CFIHOS, IEC 81346). PMEF does not invent new semantics where existing standards apply.

**G-03 Cross-discipline.** PMEF covers Piping, Equipment, E&I, Structural Steel, Pipe Stress, Process Simulation, ERP/EAM, and Civil in a unified information model with explicit cross-domain relationship objects.

**G-04 Git-friendly.** The primary serialisation (NDJSON) is designed for version control: one object per line, deterministic key ordering, stable identifiers. Adding, modifying, or removing one object produces a minimal, readable diff.

**G-05 Practical for implementors.** Schemas are expressed in JSON Schema Draft 2020-12, widely supported by tooling. Property sets use familiar engineering terminology. Units are unambiguous (SI: mm, Pa, K).

**G-06 Round-trip fidelity.** A Level 3 conformant implementation must achieve ≥98% attribute fidelity on the normative benchmark dataset when performing a complete export → PMEF → import cycle.

**G-07 Extensible.** The `customAttributes` extension mechanism and the RFC process allow project-specific and industry-specific extensions without breaking the core schema.

**G-08 Scalable.** The streaming NDJSON format and the split-file PMEFX container support plant models with millions of objects. PMEF makes no assumptions about maximum model size.

**G-09 Lifecycle-aware.** Every PMEF object carries ISO 19650 CDE workflow state and revision history, enabling PMEF to serve as the neutral data backbone for design, construction, and operations.

### 6.2 Non-Goals

**NG-01 Not a database.** PMEF is a file exchange format, not a database schema. Storing PMEF data in a database is an implementation choice, not a PMEF requirement.

**NG-02 Not a workflow engine.** PMEF does not define how data flows between tools, who can modify what, or when. These are CDE and project management concerns.

**NG-03 Not a process simulation language.** PMEF references simulation results and links to simulation models (FMU, Plant Simulation files) but does not define simulation algorithms or process models.

**NG-04 Not a replacement for existing standards.** PMEF complements DEXPI, IFC, CIS/2, and AutomationML. It does not intend to replace them in their primary domains.

---

## 7 Overview of PMEF Architecture

### 7.1 Five-Layer Architecture

PMEF is structured as five interdependent layers:

```
┌──────────────────────────────────────────────────────────────┐
│  Layer 5 — Adapters                                          │
│  Tool-specific import/export plugins                         │
│  (Rust crates, Apache 2.0)                                   │
├──────────────────────────────────────────────────────────────┤
│  Layer 4 — Geometry                                          │
│  Parametric primitives · glTF 2.0 · STEP AP242 · OpenUSD    │
├──────────────────────────────────────────────────────────────┤
│  Layer 3 — Serialisation                                     │
│  NDJSON (primary) · CAEX XML (secondary) · AASX container   │
├──────────────────────────────────────────────────────────────┤
│  Layer 2 — Information Model                                 │
│  Entity types · Property sets · Relationships                │
│  (JSON Schema Draft 2020-12)                                 │
├──────────────────────────────────────────────────────────────┤
│  Layer 1 — Core Ontology                                     │
│  ISO 15926-14/IDO · DEXPI 2.0 · IEC 81346 · CFIHOS V2.0    │
│  (OWL 2, normative via rdlType URI references)               │
└──────────────────────────────────────────────────────────────┘
```

Layer 1 (Core Ontology) defines what things *mean*. Layer 2 (Information Model) defines what *data* is exchanged. Layer 3 (Serialisation) defines *how* data is encoded in files. Layer 4 (Geometry) defines how 3D shapes are represented. Layer 5 (Adapters) defines how data is translated to and from specific tools.

### 7.2 Domain Structure

PMEF organises plant objects into eight engineering disciplines, each with its own schema module:

```
pmef-base.schema.json          — shared types (IDs, coords, revisions)
pmef-piping-component.schema.json  — Piping domain
pmef-equipment.schema.json         — Equipment domain
pmef-ei.schema.json                — E&I domain
pmef-steel.schema.json             — Structural Steel domain
pmef-geometry.schema.json          — Geometry layer
pmef-property-sets.schema.json     — all property sets
pmef-relationships.schema.json     — typed relationships
```

Additional modules (Pipe Stress, Simulation, ERP/EAM, Civil) will be added in subsequent versions.

### 7.3 Object Identity

Every PMEF object has a globally unique identifier:

```
@id: "urn:pmef:<domain>:<project>:<local-id>"

Examples:
  urn:pmef:obj:eaf-2026:P-201A          equipment object
  urn:pmef:line:eaf-2026:CW-201         piping line
  urn:pmef:geom:eaf-2026:V-201-prim     geometry object
  urn:pmef:rel:eaf-2026:XV-101-ctrl     relationship object
  urn:pmef:loop:eaf-2026:FIC-101        instrument loop
```

The `<domain>` segment uses the following conventions:

| Segment | Object class |
|---------|-------------|
| `obj` | Physical object (equipment, piping component, instrument) |
| `line` | Piping network system |
| `seg` | Piping segment |
| `spool` | Fabrication spool |
| `loop` | Instrument loop |
| `geom` | Geometry object |
| `rel` | Relationship object |
| `plant` | Plant or facility |
| `unit` | Process unit or area |
| `pkg` | Package file header |
| `doc` | Document reference |
| `functional` | Functional object (from P&ID / DEXPI) |
| `catalog` | Catalog entry |

### 7.4 The P&ID–3D–EAM Linking Model

The central value proposition of PMEF is enabling traceability from P&ID to 3D model to EAM system. This is achieved through the `isDerivedFrom` field and the `pmef:IsDerivedFrom` relationship object:

```
P&ID Layer (DEXPI 2.0)
  └── FunctionalObject (P-201A tag)
          │
          │  isDerivedFrom
          ▼
3D Layer (PMEF)
  └── pmef:Pump  (P-201A physical object)
          │
          │  pmef:HasEquivalentIn
          ▼
EAM Layer (SAP PM / IBM Maximo)
  └── Equipment Master Record (P-201A)
```

This three-way link enables automated completeness checking ("every P&ID tag has a 3D object"), change management ("this 3D object was modified — which P&ID tag does it affect?"), and handover verification.

---

## 8 Relationship to Other Standards

### 8.1 ISO 15926

PMEF is not an ISO 15926 implementation but uses ISO 15926 as its semantic foundation:

- `rdlType` on every PMEF object carries a URI that resolves in the PCA-RDL (ISO 15926-4 reference data). This provides semantic grounding without requiring implementors to understand the full ISO 15926-2 data model.
- The PMEF ontology (`pmef-ontology.owl`, published separately) maps PMEF entity types to ISO 15926-14 IDO classes.
- The `isDerivedFrom` relationship corresponds to ISO 15926-14 `ClassificationOfIndividual`.

See [docs/iso15926-mapping.md](../docs/iso15926-mapping.md) for the full mapping.

### 8.2 DEXPI 2.0

PMEF is designed as the 3D complement to DEXPI 2.0:

- DEXPI models the functional design (P&ID topology, process functions).
- PMEF models the physical realisation (3D geometry, engineering attributes, fabrication data).
- The `isDerivedFrom` relationship links PMEF physical objects to their DEXPI functional counterparts.
- PMEF supports import of DEXPI XML files to create the functional object index.

### 8.3 CFIHOS

PMEF property sets are aligned with CFIHOS V2.0 attribute definitions:

- Equipment basic attributes map to CFIHOS Tag class attributes.
- Piping specification attributes map to CFIHOS piping attributes.
- The `catalogRef.eclassIRDI` field carries eCl@ss classification codes, which CFIHOS requires for IOGP projects.

PMEF can export a CFIHOS-compliant handover package by selecting the CFIHOS-mapped attributes from a PMEF package.

### 8.4 IFC 4.3

PMEF and IFC 4.3 are complementary. IFC 4.3 Infra/MEP covers:

- Building and civil infrastructure (IFC-Infra) — PMEF defers to IFC for this domain.
- MEP systems (IfcDistributionSystem, IfcPipeSegment) — PMEF provides richer piping detail.
- Structural elements (IfcBeam, IfcColumn) — PMEF's SteelMember is semantically aligned.

PMEF provides an IFC export adapter that generates IfcPipeSegment, IfcFlowTerminal, and IfcEquipment objects from PMEF piping and equipment objects.

### 8.5 AutomationML / CAEX

PMEF's E&I domain (Chapter 14 of the Extended Specification) is aligned with AutomationML 2.10:

- `pmef:PLCObject.amlRef` carries the AutomationML `InternalElement` GUID.
- The CAEX XML secondary serialisation is AML-compatible.
- `pmef:MTPModule` links to AML/AASX MTP 2.0 files via `DocumentLink`.

### 8.6 OpenUSD

OpenUSD is supported as the third PMEF geometry layer (alongside parametric primitives and glTF 2.0). The PMEF-USD adapter maps PMEF objects to USD prims with a custom `pmef:` schema namespace, enabling integration with NVIDIA Omniverse, Emulate3D Factory Test, and other Omniverse-connected tools.

---

## 9 Licensing and Governance

### 9.1 Specification Licence

The PMEF Specification (Markdown text, JSON Schemas, UML diagrams) is published under the **Creative Commons Attribution 4.0 International (CC BY 4.0)** licence.

You are free to reproduce, adapt, and build upon this specification provided you give appropriate attribution: "Based on the PMEF Specification, https://github.com/pmef/specification, CC BY 4.0."

### 9.2 Reference Implementation Licence

The PMEF reference implementation (Rust crates `pmef-core`, `pmef-io`, `pmef-validate`, `pmef-cli`) is published under the **Apache License 2.0**.

### 9.3 Example Data Licence

The example datasets in `examples/` are published under **CC0 1.0 Universal (Public Domain Dedication)**. They may be used freely without attribution.

### 9.4 Governance

PMEF is governed by a Technical Steering Committee (TSC) operating under the rules defined in [GOVERNANCE.md](../GOVERNANCE.md). The specification evolves through a public RFC process documented in [CONTRIBUTING.md](../CONTRIBUTING.md).

---

*End of Chapter 01.*

**[Chapter 02 — Information Model →](02-information-model.md)**
