# PMEF Specification · Chapter 06 · Conformance

**Document number:** PMEF-SPEC-06  
**Version:** 0.9.0-rc  
**Status:** Normative  
**Date:** 2026-03-31

---

## Table of Contents

1. [General](#1-general)
2. [Conformance Levels](#2-conformance-levels)
3. [Normative Conformance Clauses](#3-normative-conformance-clauses)
4. [Benchmark Datasets](#4-benchmark-datasets)
5. [Conformance Test Suite](#5-conformance-test-suite)
6. [Round-Trip Fidelity Measurement](#6-round-trip-fidelity-measurement)
7. [Conformance Report Format](#7-conformance-report-format)
8. [Certification Process](#8-certification-process)

---

## 1 General

This chapter defines the conformance requirements that an
implementation **MUST** satisfy to claim conformance with this
specification. Conformance is assessed against three levels, each
building on the previous.

### 1.1 Conformance Target

A **PMEF conformant implementation** is any software component that:

- Reads or writes PMEF NDJSON files, or
- Translates between PMEF NDJSON and the native format of an engineering tool.

Web viewers, validation tools, command-line utilities, and programming library APIs are all within scope.

### 1.2 Conformance Statement

An implementation claiming conformance with this specification
**MUST** provide a written conformance statement identifying:

1. The conformance level claimed (Basic, Full, or RoundTrip).
2. The domains covered (Piping, Equipment, E&I, Structural Steel).
3. The PMEF version against which conformance is claimed.
4. The conformance test suite results (test IDs and pass/fail status).
5. Any exceptions or deviations from the normative requirements.

---

## 2 Conformance Levels

### 2.1 Level 1 — PMEF-Basic

**Intended for:** Read-only viewers, lightweight validators, simple converters.

An implementation claiming PMEF-Basic conformance **MUST**:

| ID | Requirement |
|----|-------------|
| CL1-01 | Parse any valid PMEF NDJSON file without error. |
| CL1-02 | Validate each parsed object against the corresponding JSON Schema. |
| CL1-03 | Correctly handle unknown `@type` values (skip + warn, do not fail). |
| CL1-04 | Correctly handle unknown property names in known entity types (preserve, do not fail). |
| CL1-05 | Support all Core entity types: `pmef:FileHeader`, `pmef:Plant`, `pmef:Unit`. |
| CL1-06 | Support the Piping entity types listed as required (✅) in the conformance matrix (§3.1). |
| CL1-07 | Support the Equipment entity types listed as required in the conformance matrix. |
| CL1-08 | Write valid `RevisionMetadata` including `revisionId` and `changeState` on all written objects. |
| CL1-09 | Pass all conformance tests in the `unit` and `roundtrip` categories at Level 1. |

An implementation claiming PMEF-Basic conformance is **exempt** from:

- Writing geometry.
- Resolving cross-file references.
- Writing `pmef:HasEquivalentIn` relationships.
- Supporting E&I, Structural Steel, or Relationship entity types.

### 2.2 Level 2 — PMEF-Full

**Intended for:** Full read/write implementations, design tool plugins.

An implementation claiming PMEF-Full conformance **MUST** satisfy all Level 1 requirements, plus:

| ID | Requirement |
|----|-------------|
| CL2-01 | Support all entity types listed as required (✅) or recommended (🟡) in the conformance matrix. |
| CL2-02 | Write geometry references for all physical objects in supported domains, at minimum `GeometryReference.type = "none"` when no geometry is available. |
| CL2-03 | Support parametric primitives: CYLINDER, CONE, CIRC_TORUS, BOX, COMPOSITE, MESH_REF. |
| CL2-04 | Resolve `isPartOf` and `Port.connectedTo` references within a single NDJSON file. |
| CL2-05 | Write `pmef:HasEquivalentIn` for every exported physical object. |
| CL2-06 | Write `pmef:IsDerivedFrom` relationships when the source tool provides P&ID tag cross-references. |
| CL2-07 | Achieve ≥ 95% round-trip attribute fidelity on the PMEF-DS-01 benchmark dataset (§4.1). |
| CL2-08 | Pass all conformance tests in the `unit`, `integration`, and `roundtrip` categories at Level 2. |

### 2.3 Level 3 — PMEF-RoundTrip

**Intended for:** Adapter vendors seeking interoperability certification.

An implementation claiming PMEF-RoundTrip conformance **MUST** satisfy all Level 2 requirements, plus:

| ID | Requirement |
|----|-------------|
| CL3-01 | Support all entity types in all claimed domains. |
| CL3-02 | Achieve ≥ 98% round-trip attribute fidelity on PMEF-DS-01. |
| CL3-03 | Achieve ≥ 98% round-trip fidelity on at least one domain-specific benchmark (PMEF-DS-02, DS-03, DS-04, or DS-05). |
| CL3-04 | 100% fidelity on identity fields: `@id`, `tagNumber`, `lineNumber`, `weldNumber`. |
| CL3-05 | Write `pmef:HasEquivalentIn` for **every** exported object without exception. |
| CL3-06 | Support delta export: write `pmef:IsRevisionOf` for re-exported objects that have changed. |
| CL3-07 | Generate a machine-readable conformance report (§7). |
| CL3-08 | Pass all conformance tests at Level 3. |

---

## 3 Normative Conformance Clauses

### 3.1 Entity Type Conformance Matrix

The matrix below lists all normatively defined PMEF entity types and their requirements at each conformance level.

**Legend:** ✅ Required · 🟡 Recommended (SHOULD) · ❌ Not required · — Not applicable

#### Core

| Entity Type | L1 | L2 | L3 |
|-------------|----|----|-----|
| `pmef:FileHeader` | ✅ | ✅ | ✅ |
| `pmef:Plant` | ✅ | ✅ | ✅ |
| `pmef:Unit` | ✅ | ✅ | ✅ |
| `pmef:Area` | 🟡 | ✅ | ✅ |
| `RevisionMetadata` | ✅ | ✅ | ✅ |
| `GeometryReference` | 🟡 | ✅ | ✅ |
| `CatalogReference` | ❌ | 🟡 | ✅ |
| `DocumentLink` | ❌ | 🟡 | ✅ |

#### Piping Domain

| Entity Type | L1 | L2 | L3 |
|-------------|----|----|-----|
| `pmef:PipingNetworkSystem` | ✅ | ✅ | ✅ |
| `pmef:PipingSegment` | ✅ | ✅ | ✅ |
| `pmef:Pipe` | ✅ | ✅ | ✅ |
| `pmef:Elbow` | ✅ | ✅ | ✅ |
| `pmef:Tee` | ✅ | ✅ | ✅ |
| `pmef:Reducer` (both subtypes) | ✅ | ✅ | ✅ |
| `pmef:Flange` (all 9 types) | ✅ | ✅ | ✅ |
| `pmef:Valve` | ✅ | ✅ | ✅ |
| `pmef:Olet` | 🟡 | ✅ | ✅ |
| `pmef:Gasket` | 🟡 | ✅ | ✅ |
| `pmef:Weld` + `WeldSpec` | 🟡 | ✅ | ✅ |
| `pmef:PipeSupport` + `SupportSpec` | 🟡 | ✅ | ✅ |
| `pmef:Spool` | 🟡 | ✅ | ✅ |
| `Port.connectedTo` (topology) | 🟡 | ✅ | ✅ |
| `PipingDesignConditions` | 🟡 | ✅ | ✅ |
| `PipingSpecification` | ✅ | ✅ | ✅ |
| `ValveSpec` | 🟡 | ✅ | ✅ |
| `WeldSpec` | ❌ | ✅ | ✅ |

#### Equipment Domain

| Entity Type | L1 | L2 | L3 |
|-------------|----|----|-----|
| `pmef:Vessel` + `VesselDesign` | ✅ | ✅ | ✅ |
| `pmef:Tank` | ✅ | ✅ | ✅ |
| `pmef:Pump` + `PumpSpec` | ✅ | ✅ | ✅ |
| `pmef:Compressor` + `CompressorSpec` | 🟡 | ✅ | ✅ |
| `pmef:HeatExchanger` + `HeatExchangerSpec` | ✅ | ✅ | ✅ |
| `pmef:Column` | 🟡 | ✅ | ✅ |
| `pmef:Reactor` | 🟡 | ✅ | ✅ |
| `pmef:Filter` | 🟡 | ✅ | ✅ |
| `pmef:Turbine` | 🟡 | 🟡 | ✅ |
| `pmef:GenericEquipment` | ✅ | ✅ | ✅ |
| `Nozzle` (embedded) | ✅ | ✅ | ✅ |
| `Nozzle.connectedLineId` | 🟡 | ✅ | ✅ |

#### E&I Domain

| Entity Type | L1 | L2 | L3 |
|-------------|----|----|-----|
| `pmef:InstrumentObject` | 🟡 | ✅ | ✅ |
| `pmef:InstrumentLoop` | 🟡 | ✅ | ✅ |
| `pmef:PLCObject` | 🟡 | ✅ | ✅ |
| `pmef:CableObject` | ❌ | ✅ | ✅ |
| `pmef:CableTrayRun` | ❌ | ✅ | ✅ |
| `pmef:MTPModule` | ❌ | 🟡 | ✅ |
| `safetySpec` (SIL data) | ❌ | 🟡 | ✅ |
| `tiaPLCAddress` | ❌ | 🟡 | ✅ |

#### Structural Steel Domain

| Entity Type | L1 | L2 | L3 |
|-------------|----|----|-----|
| `pmef:SteelSystem` | ❌ | 🟡 | ✅ |
| `pmef:SteelMember` | ❌ | 🟡 | ✅ |
| `pmef:SteelNode` | ❌ | 🟡 | ✅ |
| `pmef:SteelConnection` | ❌ | 🟡 | ✅ |
| `analysisResults` | ❌ | ❌ | 🟡 |

#### Geometry

| Primitive | L1 | L2 | L3 |
|-----------|----|----|-----|
| `CYLINDER` | 🟡 | ✅ | ✅ |
| `CONE` | 🟡 | ✅ | ✅ |
| `SPHERE` | ❌ | ✅ | ✅ |
| `DISH` | ❌ | ✅ | ✅ |
| `CIRC_TORUS` | 🟡 | ✅ | ✅ |
| `SNOUT` | ❌ | ✅ | ✅ |
| `BOX` | 🟡 | ✅ | ✅ |
| `EXTRUSION` | ❌ | ✅ | ✅ |
| `REVOLUTION` | ❌ | ✅ | ✅ |
| `VALVE_BODY` | ❌ | ✅ | ✅ |
| `NOZZLE` | ❌ | ✅ | ✅ |
| `STEEL_PROFILE` | ❌ | 🟡 | ✅ |
| `CABLE_TRAY` | ❌ | 🟡 | ✅ |
| `MESH_REF` (glTF) | 🟡 | ✅ | ✅ |
| `COMPOSITE` | ❌ | ✅ | ✅ |
| STEP AP242 | ❌ | 🟡 | ✅ |
| OpenUSD | ❌ | ❌ | 🟡 |

#### Relationships

| Relationship Type | L1 | L2 | L3 |
|-------------------|----|----|-----|
| `pmef:IsPartOf` | 🟡 | ✅ | ✅ |
| `pmef:IsConnectedTo` | ❌ | ✅ | ✅ |
| `pmef:IsDerivedFrom` | ❌ | ✅ | ✅ |
| `pmef:Supports` | ❌ | 🟡 | ✅ |
| `pmef:ControlledBy` | ❌ | 🟡 | ✅ |
| `pmef:IsDocumentedBy` | ❌ | 🟡 | ✅ |
| `pmef:IsRevisionOf` | ❌ | 🟡 | ✅ |
| `pmef:HasEquivalentIn` | ❌ | ✅ | ✅ |
| `pmef:IsCollocatedWith` | ❌ | ❌ | 🟡 |
| `pmef:ReplacedBy` | ❌ | ❌ | 🟡 |

---

## 4 Benchmark Datasets

### 4.1 PMEF-DS-01 — Cooling Water Pump Skid

**File:** `examples/pump-skid-complete.ndjson`  
**Objects:** 16  
**Disciplines:** Piping + Equipment  
**Required for:** Level 2 and Level 3 conformance.

Includes: `pmef:Pump` P-201A (API 610, 3 nozzles, VFD),
`pmef:Vessel` V-201 (EN 13445, composite geometry),
`pmef:PipingNetworkSystem` CW-201 and CW-202, piping components
(Pipe, Elbow, Flange, Gasket), Weld record, PipeSupport, Spool,
and ParametricGeometry.

**Round-trip test:** 18 individual assertions (RT-001 through RT-018). See `tests/roundtrip/test_roundtrip.py`.

### 4.2 PMEF-DS-02 — Heat Exchanger Station

**File:** `examples/heat-exchanger-station.ndjson`  
**Objects:** ~40  
**Disciplines:** Piping + Equipment  
**Required for:** Level 3 conformance (alternative to DS-03, DS-04, DS-05).

Includes: Shell-and-tube heat exchangers (TEMA BEM), associated
piping (shell-side and tube-side), isolation valves, control
valves with instrument loops, piping supports on steel structure.

### 4.3 PMEF-DS-03 — Compressor Train

**File:** `examples/compressor-train.ndjson`  
**Objects:** ~35  
**Disciplines:** Piping + Equipment  
**Required for:** Level 3 conformance (alternative).

Includes: API 617 compressor, API 611 turbine driver, inter-stage
coolers, suction/discharge piping with spectacle blinds and
pressure safety valves, lube oil system outline.

### 4.4 PMEF-DS-04 — Instrument Loop FIC-10101

**File:** `examples/ei-loop-fic-10101.ndjson`  
**Objects:** 9  
**Disciplines:** E&I  
**Required for:** Level 3 E&I conformance.

Includes: HART flow transmitter-controller (SIL1), pneumatic
control valve (SIL1, FC), Siemens S7-1500 CPU and AI module,
instrumentation cable, instrument loop, and
ControlledBy / IsDerivedFrom relationships.

### 4.5 PMEF-DS-05 — EAF Cooling Circuit Segment

**File:** `examples/eaf-segment.ndjson` (planned)  
**Objects:** ~80  
**Disciplines:** Piping + Equipment + E&I  
**Required for:** Level 3 mixed-discipline conformance.

Includes: Electric Arc Furnace (pmef:Reactor, EAF subtype),
cooling water piping circuit, cooling water pumps,
instrumentation, structural steel supports.

---

## 5 Conformance Test Suite

### 5.1 Test Suite Location

The PMEF conformance test suite is located in `tests/` and is executable via:

```bash
python run_tests.py              # All tests
python run_tests.py --level 1   # Level 1 tests only
python run_tests.py --level 2   # Level 1 + 2 tests
python run_tests.py --level 3   # All tests
python run_tests.py --report json > conformance-report.json
```

### 5.2 Test Categories

| Category | Description | Test IDs |
|----------|-------------|---------|
| `unit` | JSON Schema validation tests, one per entity type | PIPE-U-*, EQUIP-U-*, EI-U-*, STEEL-U-*, REL-U-*, GEOM-U-* |
| `negative` | Invalid input must be rejected | PIPE-U-003, EI-U-003, REL-U-008, … |
| `integration` | Multi-object consistency (topology, cross-refs) | INTG-001 through INTG-006 |
| `roundtrip` | Attribute fidelity against benchmark datasets | RT-001 through RT-018 |
| `performance` | Large model handling (> 10k objects) | PERF-001 through PERF-005 |

### 5.3 Test Pass Requirements

| Level | Required pass rate |
|-------|--------------------|
| L1 | 100% of Level 1 `unit` tests, 100% of Level 1 `roundtrip` tests |
| L2 | 100% of Level 1 + Level 2 `unit`, `integration`, `roundtrip` tests |
| L3 | 100% of all tests; 100% on identity-field assertions |

### 5.4 Normative Test IDs

The following test IDs are normative for conformance claims at each level:

**Level 1 (Piping + Equipment):**
PIPE-U-001, PIPE-U-002, PIPE-U-003, PIPE-U-004, PIPE-U-005,
PIPE-U-006, PIPE-U-007, PIPE-U-011, PIPE-U-013, PIPE-U-014,
PIPE-U-015, EQUIP-U-001, EQUIP-U-002, EQUIP-U-003, EQUIP-U-004,
EQUIP-U-006, EQUIP-U-009, EQUIP-U-011, EQUIP-U-012, RT-001,
RT-002, RT-003, RT-004, RT-005, RT-006, RT-007, RT-013, RT-017

**Level 2 (adds):**
PIPE-U-008, PIPE-U-009, PIPE-U-010, PIPE-U-016, EQUIP-U-005,
EQUIP-U-007, EQUIP-U-008, EQUIP-U-010, EI-U-001 through
EI-U-008, STEEL-U-001 through STEEL-U-006, REL-U-001 through
REL-U-008, GEOM-U-001 through GEOM-U-008, RT-008 through
RT-018, INTG-001 through INTG-006

**Level 3 (adds):**
All tests; plus RT fidelity ≥ 98% on DS-01 and one additional benchmark dataset.

---

## 6 Round-Trip Fidelity Measurement

### 6.1 Definition

Round-trip fidelity for a given attribute `A` on entity type `T` is defined as:

```text
RT_fidelity(A, T) =
  |{objects of type T where exported(A) == imported(A)}|
  ÷
  |{objects of type T where A is defined}|
```

For numeric attributes, equality is tested within a tolerance ε (see §6.2).

### 6.2 Numeric Tolerance

| Attribute type | Tolerance ε |
|---------------|-------------|
| Length [mm] | 0.01 mm |
| Pressure [Pa] | 1 Pa |
| Temperature [K] | 0.01 K |
| Flow [m³/h] | 0.001 m³/h |
| Efficiency [%] | 0.01% |
| Angle [°] | 0.001° |

### 6.3 Identity Field Requirements

The following fields **MUST** achieve exactly 100% round-trip fidelity (no tolerance):

- `@id`
- `PipingNetworkSystem.lineNumber`
- `EquipmentBasic.tagNumber`
- `InstrumentObject.tagNumber`
- `WeldSpec.weldNumber`
- `SteelMember.memberMark`
- `Spool.spoolMark`
- `RevisionMetadata.revisionId`
- `RevisionMetadata.changeState`

### 6.4 Aggregate Fidelity Calculation

Aggregate fidelity is calculated across all tested attributes for all objects in the benchmark:

```text
Aggregate_RT_fidelity =
  Σ(tested attributes that pass) ÷ Σ(total tested attributes)
```

**Thresholds:**

- Level 2: Aggregate fidelity ≥ 0.95 (95%)
- Level 3: Aggregate fidelity ≥ 0.98 (98%)

### 6.5 Exemptions

The following attribute types are exempt from round-trip fidelity measurement:

- Geometry attributes (`geometry.*`) — assessed separately by visual review.
- `customAttributes` — must be preserved exactly (100%) but are not counted in the aggregate.
- Attributes that are explicitly listed as optional in the schema and that the tested tool does not support.

---

## 7 Conformance Report Format

### 7.1 Machine-Readable Report

A conformance report **MUST** be generated in JSON format when
`pmef-cli conformance` is run or when the test suite is executed
with `--report json`.

Mandatory report fields:

```json
{
  "pmefVersion": "0.9.0",
  "implementationName": "pmef-adapter-plant3d",
  "implementationVersion": "0.9.0",
  "conformanceLevel": 3,
  "domains": ["piping", "equipment"],
  "testDate": "2026-03-31",
  "summary": {
    "total": 82,
    "passed": 82,
    "failed": 0,
    "skipped": 0
  },
  "fidelity": {
    "dataset": "PMEF-DS-01",
    "aggregateFidelity": 0.9876,
    "identityFieldFidelity": 1.0
  },
  "tests": [
    {
      "id": "PIPE-U-001",
      "name": "PipingNetworkSystem — minimal valid",
      "status": "PASS",
      "duration_ms": 3.7
    }
  ]
}
```

### 7.2 Human-Readable Summary

The test runner also produces a human-readable summary:

```text
PMEF Conformance Test Suite  (v0.9)
────────────────────────────────────────────────────────────
  Total: 82  │  ✓ 82  ✗ 0  ○ 0
  Level claimed: 3
  Domains: piping, equipment, ei, steel

  Round-Trip Fidelity (PMEF-DS-01):
    Aggregate:            98.76%  ✓ (≥98% required)
    Identity fields:     100.00%  ✓
    Piping attributes:    99.20%  ✓
    Equipment attributes: 98.10%  ✓

  Conformance: PMEF-RoundTrip (Level 3) ✓
────────────────────────────────────────────────────────────
```

---

## 8 Certification Process

### 8.1 Self-Certification

Implementations **MAY** self-certify at any conformance level by:

1. Running the PMEF conformance test suite with `python run_tests.py --level <N> --report json`.
2. Publishing the test report on the implementation's product page or repository.
3. Including the PMEF conformance badge in documentation.

### 8.2 Community Review

To have a self-certification reviewed by the PMEF community:

1. Submit the conformance report as a GitHub issue using the
   [Conformance Report template](../.github/ISSUE_TEMPLATE/conformance.md).
2. The PMEF TSC or a designated WG will review the report within 30 days.
3. Upon acceptance, the implementation is listed on the PMEF
   website's [Certified Implementations](https://pmef.net/certified)
   page.

### 8.3 Conformance Badges

The following badges are available for use in documentation after self-certification:

```text
PMEF-Basic     (Level 1)
PMEF-Full      (Level 2)
PMEF-RoundTrip (Level 3)
```

Badge image URLs: `https://pmef.net/badges/pmef-<level>.svg`

### 8.4 Revoking Conformance Claims

A conformance claim **MUST** be updated or revoked if:

- The implementation introduces a change that causes previously passing tests to fail.
- A schema update in a new PMEF version invalidates the previously tested objects.
- The PMEF TSC identifies a gap in the implementation's coverage that contradicts the claimed level.

---

## Annex A — Requirement Summary

The following table summarises all normative requirements by their unique identifier.

| ID | Chapter | Requirement | Level |
|----|---------|-------------|-------|
| CL1-01 | §2.1 | Parse any valid PMEF NDJSON file without error | L1 |
| CL1-02 | §2.1 | Validate each parsed object against JSON Schema | L1 |
| CL1-03 | §2.1 | Handle unknown @type gracefully | L1 |
| CL1-04 | §2.1 | Handle unknown properties gracefully | L1 |
| CL1-05 | §2.1 | Support Core entity types | L1 |
| CL1-06 | §2.1 | Support required Piping entity types | L1 |
| CL1-07 | §2.1 | Support required Equipment entity types | L1 |
| CL1-08 | §2.1 | Write valid RevisionMetadata | L1 |
| CL1-09 | §2.1 | Pass Level 1 test suite | L1 |
| CL2-01 | §2.2 | Support all required and recommended entity types | L2 |
| CL2-02 | §2.2 | Write geometry references | L2 |
| CL2-03 | §2.2 | Support 6 parametric primitive types | L2 |
| CL2-04 | §2.2 | Resolve isPartOf and Port.connectedTo | L2 |
| CL2-05 | §2.2 | Write pmef:HasEquivalentIn for all objects | L2 |
| CL2-06 | §2.2 | Write pmef:IsDerivedFrom when available | L2 |
| CL2-07 | §2.2 | ≥ 95% RT fidelity on DS-01 | L2 |
| CL2-08 | §2.2 | Pass Level 2 test suite | L2 |
| CL3-01 | §2.3 | Support all entity types in claimed domains | L3 |
| CL3-02 | §2.3 | ≥ 98% RT fidelity on DS-01 | L3 |
| CL3-03 | §2.3 | ≥ 98% RT fidelity on one domain benchmark | L3 |
| CL3-04 | §2.3 | 100% fidelity on identity fields | L3 |
| CL3-05 | §2.3 | pmef:HasEquivalentIn for ALL objects | L3 |
| CL3-06 | §2.3 | Support delta export | L3 |
| CL3-07 | §2.3 | Generate machine-readable conformance report | L3 |
| CL3-08 | §2.3 | Pass all Level 3 tests | L3 |
| R-GEN-01 | §3.1 | Write FileHeader as first line | L1 |
| R-GEN-02 | §3.1 | Assign stable @id values | L1 |
| R-GEN-03 | §3.1 | Write HasEquivalentIn for every object | L2 |
| R-GEN-04 | §3.1 | Write RevisionMetadata with authoringTool | L1 |
| R-GEN-05 | §3.1 | Convert all values to PMEF units | L1 |
| R-GEN-06 | §3.1 | Validate objects before writing | L1 |
| R-GEN-07 | §3.1 | Report unmapped fields | L2 |
| R-GEN-08 | §3.1 | Preserve customAttributes on round-trip | L1 |

---

*End of Chapter 06 and PMEF Specification v0.9.0-rc.*

**[← Chapter 05](05-adapters.md)** · **[↑ Back to README](../README.md)**
