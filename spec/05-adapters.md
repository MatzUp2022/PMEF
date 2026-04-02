# PMEF Specification · Chapter 05 · Adapters

**Document number:** PMEF-SPEC-05  
**Version:** 0.9.0-rc  
**Status:** Normative  
**Date:** 2026-03-31

---

## Table of Contents

1. [General](#1-general)
2. [Adapter Architecture](#2-adapter-architecture)
3. [General Adapter Requirements](#3-general-adapter-requirements)
4. [Piping Adapter Requirements](#4-piping-adapter-requirements)
5. [Equipment Adapter Requirements](#5-equipment-adapter-requirements)
6. [E&I Adapter Requirements](#6-ei-adapter-requirements)
7. [Structural Steel Adapter Requirements](#7-structural-steel-adapter-requirements)
8. [Tool-Specific Adapter Specifications](#8-tool-specific-adapter-specifications)
9. [PCF Round-Trip Mapping](#9-pcf-round-trip-mapping)
10. [Unit Conversion Rules](#10-unit-conversion-rules)

---

## 1 General

An **adapter** is a software component that translates between the native data format of an engineering tool and PMEF. This chapter defines:

- The mandatory and optional capabilities of PMEF adapters.
- The normative mapping rules for each supported engineering domain.
- Tool-specific adapter specifications for the adapters included in the PMEF reference implementation.

Adapter conformance is evaluated at Level 3 (PMEF-RoundTrip). See Chapter 06 for conformance requirements.

### 1.1 Adapter Types

| Type | Direction | Notes |
|------|-----------|-------|
| **Export adapter** | Tool → PMEF | Reads native tool data, writes PMEF NDJSON |
| **Import adapter** | PMEF → Tool | Reads PMEF NDJSON, writes native tool data |
| **Bidirectional adapter** | Tool ↔ PMEF | Both directions; required for Level 3 |
| **Delta adapter** | Tool → PMEF (incremental) | Only changed objects; uses `pmef:IsRevisionOf` |

### 1.2 Adapter Identification

Each adapter **MUST** declare its identity in the `pmef:FileHeader.authoringTool` field:

```jsonc
"authoringTool": "pmef-adapter-plant3d v0.9.0 (pmef-core 0.9.0)"
```

---

## 2 Adapter Architecture

### 2.1 Reference Implementation

The PMEF reference implementation provides adapters as Rust crates under the Apache 2.0 licence:

```
pmef-core          — data model structs and validation
pmef-io            — NDJSON streaming reader/writer
pmef-validate      — JSON Schema validation
pmef-geom          — geometry operations (bounding box, clash)
pmef-cli           — command-line tool (validate, convert, diff)
pmef-adapter-*     — tool-specific adapters (one crate per tool)
```

### 2.2 Adapter Pipeline

The normative adapter pipeline for export (tool → PMEF):

```
1. Connect      Connect to the native tool API or file.
2. Read         Read native objects into an intermediate representation.
3. Map          Apply field mapping rules (§4–§7).
4. Convert      Convert units (§10).
5. Resolve      Resolve identities: look up or create pmef:HasEquivalentIn.
6. Validate     Validate each PMEF object against the schema.
7. Write        Write valid objects to NDJSON.
8. Report       Report unmapped fields and mapping statistics.
```

For import (PMEF → tool):

```
1. Read         Read PMEF NDJSON (streaming).
2. Resolve      Use pmef:HasEquivalentIn to find existing native objects.
3. Map          Apply reverse field mapping.
4. Convert      Convert units.
5. Write        Write to native tool.
6. Report       Report objects created, updated, skipped.
```

### 2.3 Identity Resolution

Adapters **MUST** implement identity resolution to enable incremental updates:

**Export:** Before creating a new PMEF object, check whether a `pmef:HasEquivalentIn` relationship already exists for the native object's ID. If found, reuse the existing `@id`. If not, generate a new `urn:pmef:obj:...` URI.

**Import:** Before writing to the native tool, scan `pmef:HasEquivalentIn` objects for the target system. If a matching `targetSystemId` is found, update the existing native object. If not found, create a new native object.

---

## 3 General Adapter Requirements

### 3.1 Mandatory Capabilities

All PMEF adapters **MUST**:

- **R-GEN-01:** Write a `pmef:FileHeader` as the first line of every exported NDJSON file.
- **R-GEN-02:** Assign stable `@id` values to all exported objects, reusing existing IDs on re-export.
- **R-GEN-03:** Write a `pmef:HasEquivalentIn` relationship for every exported object, identifying the native tool and the native object ID.
- **R-GEN-04:** Write `RevisionMetadata` including `authoringTool`, `authoringToolObjectId`, and `changedAt` on every exported object.
- **R-GEN-05:** Convert all values to PMEF units before writing (§10).
- **R-GEN-06:** Validate every PMEF object against the schema before writing. Do not write invalid objects.
- **R-GEN-07:** Report, in the adapter log, the number of objects exported, the number of fields unmapped, and any errors.
- **R-GEN-08:** Preserve `customAttributes` on round-trip import without modification.

### 3.2 Recommended Capabilities

Adapters **SHOULD**:

- **R-GEN-09:** Write `rdlType` for all entity types where an RDL mapping is defined in `docs/iso15926-mapping.md`.
- **R-GEN-10:** Write `catalogRef` for piping components with a known piping class.
- **R-GEN-11:** Write `iec81346` designation for instruments, equipment, and E&I objects.
- **R-GEN-12:** Support delta export (incremental update packages).

### 3.3 Error Handling

- An adapter **MUST NOT** write an object that fails schema validation. Instead, it **MUST** write an error log entry and continue.
- An adapter **MUST NOT** halt processing when a single object fails. It **MUST** process all remaining objects.
- At the end of processing, the adapter **MUST** report the total number of successful and failed objects.

---

## 4 Piping Adapter Requirements

### 4.1 Minimum Field Set

For an adapter to claim piping support, it **MUST** export the following fields:

| PMEF Field | Required Level |
|------------|---------------|
| `PipingNetworkSystem.lineNumber` | **MUST** |
| `PipingNetworkSystem.nominalDiameter` | **MUST** |
| `PipingNetworkSystem.pipeClass` | SHOULD |
| `PipingNetworkSystem.mediumCode` | SHOULD |
| `PipingSegment.segmentNumber` | SHOULD |
| `PipingComponent.componentSpec.componentClass` | **MUST** |
| `PipingComponent.ports[].coordinate` | **MUST** |
| `PipingComponent.ports[].connectedTo` | SHOULD |
| `PipingDesignConditions.designPressure` | SHOULD |
| `PipingDesignConditions.designTemperature` | SHOULD |
| `PipingSpecification.material` | SHOULD |

### 4.2 Component Class Mapping

Adapters **MUST** map native component types to PMEF `componentClass` values. When a precise match is not available, the adapter **MUST** use `"SPECIAL"` and record the unmapped native type in `customAttributes.nativeComponentType`.

### 4.3 PCF Compatibility

Adapters that consume or produce PCF files **MUST** implement the PCF mapping defined in §9.

### 4.4 Port Topology

Export adapters **MUST** attempt to populate `Port.connectedTo` by resolving the native tool's pipe routing topology. When the native tool does not provide explicit connectivity, adapters **SHOULD** use coordinate-based matching (ports closer than 1 mm are assumed connected) and write the inferred connections with `confidence < 1.0` in a `pmef:IsConnectedTo` relationship.

---

## 5 Equipment Adapter Requirements

### 5.1 Minimum Field Set

| PMEF Field | Required Level |
|------------|---------------|
| `EquipmentBasic.tagNumber` | **MUST** |
| `EquipmentBasic.equipmentClass` | SHOULD |
| `Nozzle.nozzleId` | **MUST** (if nozzles present) |
| `Nozzle.nominalDiameter` | SHOULD |
| `Nozzle.coordinate` | **MUST** (if nozzles present) |
| `Nozzle.direction` | SHOULD |
| `Nozzle.connectedLineId` | SHOULD |

### 5.2 Equipment Class Mapping

Native equipment types **MUST** be mapped to PMEF `@type` values. The following minimum mapping **MUST** be supported:

| Native concept | PMEF `@type` |
|---------------|-------------|
| Vessel / drum / separator | `pmef:Vessel` |
| Tank (atmospheric or low-pressure) | `pmef:Tank` |
| Centrifugal pump | `pmef:Pump` |
| Heat exchanger | `pmef:HeatExchanger` |
| Column / tower | `pmef:Column` |
| Any other equipment | `pmef:GenericEquipment` |

### 5.3 Nozzle Export

Nozzle `coordinate` and `direction` **MUST** be expressed in the project coordinate system (Z-up, mm). The adapter **MUST** transform from the tool's local equipment coordinate system to the project coordinate system using the equipment's placement transformation.

---

## 6 E&I Adapter Requirements

### 6.1 Minimum Field Set

| PMEF Field | Required Level |
|------------|---------------|
| `InstrumentObject.tagNumber` | **MUST** |
| `InstrumentObject.instrumentClass` | SHOULD |
| `InstrumentObject.connectionSpec.signalType` | SHOULD |
| `InstrumentObject.comosCUID` (COMOS adapters) | SHOULD |
| `InstrumentObject.eplanBKZ` (EPLAN adapters) | SHOULD |
| `InstrumentObject.tiaPLCAddress` (TIA adapters) | SHOULD |
| `PLCObject.plcClass` | **MUST** |
| `PLCObject.vendor`, `family` | SHOULD |
| `PLCObject.amlRef` | SHOULD |

### 6.2 COMOS Adapter Requirements

Adapters for Siemens COMOS **MUST**:

- Populate `InstrumentObject.comosCUID` with the COMOS object CUID.
- Populate `PLCObject.amlRef` with the COMOS AML `InternalElement` ID.
- Export DEXPI functional tags and create `pmef:IsDerivedFrom` relationships.
- Export SIL data from COMOS safety analyses to `InstrumentObject.safetySpec`.

### 6.3 EPLAN Adapter Requirements

Adapters for EPLAN Electric P8 **MUST**:

- Populate `InstrumentObject.eplanBKZ` with the IEC 81346 designation from EPLAN.
- Export cable data to `pmef:CableObject`.
- Export PLC hardware configuration to `pmef:PLCObject` (from AML AR APC export).
- Map EPLAN function pages to `pmef:InstrumentLoop` objects.

### 6.4 TIA Portal Adapter Requirements

Adapters for Siemens TIA Portal **MUST**:

- Populate `InstrumentObject.tiaPLCAddress` with rack, slot, channel, and symbol.
- Populate `PLCObject` from the TIA hardware configuration (`.ap17` / `.ap18` via Openness).
- Export OPC UA NodeSet references to `InstrumentObject.opcuaSpec.nodeRef`.

---

## 7 Structural Steel Adapter Requirements

### 7.1 Minimum Field Set

| PMEF Field | Required Level |
|------------|---------------|
| `SteelMember.memberType` | **MUST** |
| `SteelMember.profileId` | **MUST** |
| `SteelMember.startPoint`, `endPoint` | **MUST** |
| `SteelMember.material.grade` | SHOULD |
| `SteelMember.cis2Ref` (Tekla adapters) | SHOULD |
| `SteelMember.teklaGUID` (Tekla adapters) | SHOULD |

### 7.2 Profile ID Mapping

Native profile designations **MUST** be mapped to PMEF profile IDs (`<standard>:<designation>`). The adapter **MUST** maintain a profile mapping table. When a profile cannot be mapped, the adapter **MUST** use `"CUSTOM:<native-designation>"` and record the native name in `customAttributes.nativeProfileName`.

### 7.3 CIS/2 Adapters

Adapters for tools using CIS/2 (Tekla Structures, Advance Steel) **MUST**:

- Populate `SteelMember.cis2Ref` with the CIS/2 member ID.
- Map CIS/2 `EndRelease` to `SteelConnection.connectionType` (PINNED or MOMENT_RIGID).
- Map CIS/2 `BoundaryCondition` to `SteelNode.supportType`.

---

## 8 Tool-Specific Adapter Specifications

### 8.1 AVEVA E3D Adapter (`pmef-adapter-e3d`)

**Approach:** RVM file export for geometry; PML scripting for semantic data.

| PMEF Entity | AVEVA E3D Source | Method |
|-------------|-----------------|--------|
| `PipingNetworkSystem` | PIPE/BRAN hierarchy | PML: `CE OBJ` traversal |
| `pmef:Pipe` | PIPE elements | RVM cylinder + PML attributes |
| `pmef:Elbow` | ELBOW elements | RVM CIRC_TORUS + PML |
| `pmef:Valve` | VALV elements | RVM + PML tag attributes |
| `EquipmentObject` | EQUI elements | RVM mesh + PML tag |
| `Nozzle` | NOZZ elements | PML coordinates + direction |
| `PipingDesignConditions` | Line attributes (TEMP, PRES) | PML |
| `PipingSpecification` | SPEC and BORESIZE | PML |

**Identity:** E3D database address (e.g. `/SITE/ZONE/PIPE/...`) stored in `authoringToolObjectId` and `pmef:HasEquivalentIn.targetSystemId`.

**Known limitations:**
- E3D RVM does not carry piping class data; class must be read via PML.
- Instrument tags in E3D are stored on nozzle elements, not as separate objects; the adapter creates `InstrumentObject` stubs.

### 8.2 AutoCAD Plant 3D Adapter (`pmef-adapter-plant3d`)

**Approach:** Plant SDK for semantic data; PCF export for piping component geometry.

| PMEF Entity | Plant 3D Source | Method |
|-------------|----------------|--------|
| `PipingNetworkSystem` | Line number groups | Plant SDK `PnPLineNumber` |
| `pmef:Pipe`, `pmef:Elbow`, etc. | PCF file | PCF parser (§9) |
| `EquipmentObject` | Equipment drawings | Plant SDK equipment objects |
| `Nozzle` | Nozzle connectors | Plant SDK nozzle objects |

**Identity:** Plant 3D object handle (64-bit) stored in `authoringToolObjectId`.

**Known limitations:**
- PCF carries limited semantic data; design conditions must be read from line spec.
- Curved pipes in Plant 3D export as multiple PCF segments.

### 8.3 CADMATIC Adapter (`pmef-adapter-cadmatic`)

**Approach:** CADMATIC REST Web API (Swagger-documented).

| PMEF Entity | CADMATIC Source | API Endpoint |
|-------------|----------------|-------------|
| `PipingNetworkSystem` | Pipeline objects | `GET /pipelines` |
| Piping components | Component objects | `GET /components/{lineId}` |
| `EquipmentObject` | Equipment objects | `GET /equipment` |
| `Nozzle` | Connection points | `GET /equipment/{id}/connections` |
| 3D geometry | 3DDX export | `GET /export/3ddx` |

**Identity:** CADMATIC `ObjectGUID` stored in `authoringToolObjectId`.

### 8.4 Tekla Structures Adapter (`pmef-adapter-tekla`)

**Approach:** Tekla Open API (.NET, C#).

| PMEF Entity | Tekla Source | API |
|-------------|-------------|-----|
| `SteelMember` | `Beam`, `Column`, `BracingMember` | `Model.GetObjects()` |
| `SteelConnection` | `BasePoint`, `Connection` | `Connection.GetObjects()` |
| `SteelNode` | `BasePoint` | Implicit from connection nodes |
| `SteelSystem` | `Assembly` | `Assembly.GetObjects()` |
| Geometry | `Solid.ToBrep()` | STEP AP242 export |

**Identity:** Tekla GUID (`ModelObject.GetReference().GlobalId`) stored in `SteelMember.teklaGUID` and `authoringToolObjectId`.

### 8.5 Siemens COMOS Adapter (`pmef-adapter-comos`)

**Approach:** COMOS .NET API (COM interop) + AML export for E&I data.

| PMEF Entity | COMOS Source | Method |
|-------------|-------------|--------|
| `InstrumentObject` | Instrument objects | COMOS COM API |
| `PLCObject` | PLC hardware | AML AR APC export |
| `InstrumentLoop` | Loop sheets | COM API |
| `CableObject` | Cable list | COM API |
| `PipingNetworkSystem` | DEXPI export | COMOS DEXPI XML export |

**Identity:** COMOS CUID stored in `InstrumentObject.comosCUID` and `authoringToolObjectId`.

### 8.6 EPLAN Electric P8 Adapter (`pmef-adapter-eplan`)

**Approach:** EPLAN Scripting API (C# / VB.NET) + AML export.

| PMEF Entity | EPLAN Source | Method |
|-------------|-------------|--------|
| `InstrumentObject` | Function symbols | Scripting API |
| `CableObject` | Cables | Scripting API |
| `PLCObject` | PLC hardware via AML | AML AR APC export |
| `CableTrayRun` | Cable duct | Scripting API |

**Identity:** EPLAN BKZ (IEC 81346 designation) stored in `InstrumentObject.eplanBKZ`.

---

## 9 PCF Round-Trip Mapping

### 9.1 PCF File Structure

A PCF (Piping Component File) is a text file format used by isometric drawing software (Alias IsoDraft, Caesar II, AutoCAD Plant 3D). PCF uses a flat structure with component-type keywords and coordinate triplets.

### 9.2 PCF → PMEF Field Mapping

| PCF Field | PMEF Field | Conversion |
|-----------|-----------|------------|
| `PIPELINE-REFERENCE` | `PipingNetworkSystem.lineNumber` | Direct |
| `PIPELINE-REF SPOOL-ID` | `PipingSegment` (create one per spool) | — |
| Component keyword (`PIPE`, `ELBOW`, etc.) | `PipingComponent.componentSpec.componentClass` | PCF→PMEF class table |
| `SKEY` | `PipingComponent.componentSpec.skey` | Pad to 8 chars |
| `END-POINT X Y Z BORE` | `Port.coordinate` + `Port.nominalDiameter` | Units: see §9.3 |
| `MATERIAL-IDENTIFIER` | `PipingComponent.catalogRef.catalogId` | Direct |
| `ATTRIBUTE0` (item number) | `PipingComponent.componentSpec.itemNumber` | Direct |
| `TEMPERATURE` | `PipingNetworkSystem.designConditions.operatingTemperature` | °F → K or °C → K |
| `MAX-TEMPERATURE` | `PipingNetworkSystem.designConditions.designTemperature` | °F → K or °C → K |
| `MAX-PRESSURE` | `PipingNetworkSystem.designConditions.designPressure` | psi → Pa or bar → Pa |

### 9.3 PCF Unit Handling

PCF files may use either metric (mm) or imperial (inch) units. The unit system is declared in the PCF header (`UNITS-BORE MILLIMETERS` or `UNITS-BORE INCHES`).

Converters **MUST** detect the unit system declaration and convert all coordinates and bores to mm before writing PMEF.

| PCF unit | Conversion to PMEF mm |
|----------|-----------------------|
| INCHES | × 25.4 |
| MILLIMETERS | × 1 (no conversion) |
| Temperature FAHRENHEIT | (F − 32) × 5/9 + 273.15 |
| Temperature CELSIUS | + 273.15 |
| Pressure PSI | × 6894.76 |
| Pressure BAR | × 100000 |

### 9.4 PCF Component Class Table

| PCF keyword | PMEF `componentClass` |
|-------------|----------------------|
| `PIPE` | `PIPE` |
| `ELBOW` | `ELBOW` |
| `TEE` | `TEE` |
| `CROSS` | `CROSS` |
| `REDUCER-CONCENTRIC` | `REDUCER_CONCENTRIC` |
| `REDUCER-ECCENTRIC` | `REDUCER_ECCENTRIC` |
| `FLANGE` | `FLANGE` |
| `FLANGE-BLIND` | `BLIND_FLANGE` |
| `VALVE` | (use `SKEY` prefix: `GATE`→`VALVE_GATE`, etc.) |
| `INSTRUMENT` | `INSTRUMENT_CONNECTION` |
| `OLET` | (use `SKEY` prefix: `WOL`→`OLET_WELDOLET`, etc.) |
| `WELD` | `WELD_BUTT` |
| `SUPPORT` | `PIPE_SUPPORT` |
| `GASKET` | `GASKET` |
| `BOLT-SET` | `BOLT_SET` |

---

## 10 Unit Conversion Rules

Adapters **MUST** convert from native tool units to PMEF units. The following conversion factors apply:

### 10.1 Length

| From | To mm |
|------|-------|
| inch (in) | × 25.4 |
| foot (ft) | × 304.8 |
| metre (m) | × 1000 |
| centimetre (cm) | × 10 |

### 10.2 Pressure

| From | To Pa |
|------|-------|
| psi (lbf/in²) | × 6894.757 |
| bar | × 100000 |
| barg | × 100000 (add 101325 for absolute) |
| MPa | × 1000000 |
| kPa | × 1000 |
| atm | × 101325 |

**Note:** PMEF stores absolute pressures in Pa. Adapters **MUST** convert gauge pressures to absolute by adding 101325 Pa (1 atm).

### 10.3 Temperature

| From | To K |
|------|------|
| Celsius (°C) | + 273.15 |
| Fahrenheit (°F) | (F − 32) × 5/9 + 273.15 |
| Rankine (°R) | × 5/9 |

### 10.4 Flow Rate

| From | To m³/h |
|------|---------|
| l/h | ÷ 1000 |
| l/min | × 0.060 |
| m³/min | × 60 |
| US gal/min (USGPM) | × 0.22712 |
| USGPH | × 0.0037854 |
| BBL/day | × 0.0066245 |

### 10.5 Power

| From | To kW |
|------|-------|
| W | ÷ 1000 |
| MW | × 1000 |
| hp (metric) | × 0.7355 |
| hp (US) | × 0.7457 |

### 10.6 Rounding

After unit conversion, values **SHOULD** be rounded to preserve the meaningful precision of the original value:

- Length: round to 0.001 mm (1 micrometre).
- Pressure: round to 1 Pa.
- Temperature: round to 0.01 K.
- Flow: round to 0.001 m³/h.

Adapters **MUST NOT** introduce rounding that would cause round-trip fidelity failures on the benchmark datasets.

---

*End of Chapter 05.*

**[← Chapter 04](04-geometry.md)** · **[Chapter 06 — Conformance →](06-conformance.md)**
