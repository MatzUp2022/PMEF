# PMEF Specification · Chapter 02 · Information Model

**Document number:** PMEF-SPEC-02  
**Version:** 0.9.0-rc  
**Status:** Normative  
**Date:** 2026-03-31

---

## Table of Contents

1. [General](#1-general)
2. [Base Types](#2-base-types)
3. [Plant Hierarchy](#3-plant-hierarchy)
4. [Piping Domain](#4-piping-domain)
5. [Equipment Domain](#5-equipment-domain)
6. [E&I Domain](#6-ei-domain)
7. [Structural Steel Domain](#7-structural-steel-domain)
8. [Relationship Model](#8-relationship-model)
9. [Property Sets](#9-property-sets)
10. [Extension Mechanism](#10-extension-mechanism)

---

## 1 General

### 1.1 Normative Requirement Level

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**,
**SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **MAY**, and **OPTIONAL** in this
chapter are to be interpreted as described in RFC 2119.

### 1.2 Schema Binding

The PMEF information model is expressed normatively as JSON Schema
Draft 2020-12 files located in the `schemas/` directory of the
specification repository. This chapter provides prose descriptions
of the model; the schemas are the authoritative normative reference.
Where this chapter and the schemas conflict, the schemas prevail.

Every PMEF object **MUST** validate against the corresponding schema before being considered a conformant PMEF instance.

### 1.3 Object Structure

Every PMEF object **MUST** contain the following fields:

| Field | Type | Description |
|-------|------|-------------|
| `@type` | string | Entity type URI. **MUST** be one of the defined PMEF entity type strings (see §1.4). |
| `@id` | string (URI) | Globally unique object identifier. **MUST** conform to the `PmefId` pattern. |

Every non-geometry, non-relationship PMEF object **SHOULD** contain:

| Field | Type | Description |
|-------|------|-------------|
| `pmefVersion` | string | PMEF specification version this object conforms to. Format: `MAJOR.MINOR.PATCH`. |
| `revision` | object | `RevisionMetadata` block (see §2.3). |

### 1.4 Entity Type Registry

The following `@type` values are normatively defined in PMEF v0.9:

**Plant Hierarchy:** `pmef:FileHeader`, `pmef:Plant`, `pmef:Unit`, `pmef:Area`

**Piping:** `pmef:PipingNetworkSystem`, `pmef:PipingSegment`,
`pmef:Pipe`, `pmef:Elbow`, `pmef:Tee`, `pmef:Reducer`, `pmef:Flange`,
`pmef:Valve`, `pmef:Olet`, `pmef:Gasket`, `pmef:Weld`,
`pmef:PipeSupport`, `pmef:Spool`

**Equipment:** `pmef:Vessel`, `pmef:Tank`, `pmef:Pump`,
`pmef:Compressor`, `pmef:HeatExchanger`, `pmef:Column`,
`pmef:Reactor`, `pmef:Filter`, `pmef:Turbine`,
`pmef:GenericEquipment`

**E&I:** `pmef:InstrumentObject`, `pmef:InstrumentLoop`,
`pmef:PLCObject`, `pmef:CableObject`, `pmef:CableTrayRun`,
`pmef:MTPModule`

**Structural Steel:** `pmef:SteelSystem`, `pmef:SteelMember`, `pmef:SteelNode`, `pmef:SteelConnection`

**Geometry:** `pmef:ParametricGeometry`

**Relationships:** `pmef:IsPartOf`, `pmef:IsConnectedTo`,
`pmef:IsDerivedFrom`, `pmef:Supports`, `pmef:ControlledBy`,
`pmef:IsDocumentedBy`, `pmef:IsRevisionOf`,
`pmef:HasEquivalentIn`, `pmef:IsCollocatedWith`,
`pmef:ReplacedBy`

A PMEF reader encountering an unknown `@type` value **MUST NOT**
fail. It **SHOULD** preserve the object as-is and **SHOULD** issue
a warning.

---

## 2 Base Types

The base types defined in `pmef-base.schema.json` are reused throughout the information model.

### 2.1 PmefId

A globally unique, stable identifier for a PMEF object.

**Format:** `urn:pmef:<domain>:<project>:<local-id>`

| Segment | Constraints |
|---------|------------|
| `urn:pmef:` | Fixed prefix |
| `<domain>` | Lowercase alphanumeric + hyphens. See §7.3 of Chapter 01 for conventions. |
| `<project>` | Lowercase alphanumeric + hyphens. Project code or namespace. |
| `<local-id>` | Alphanumeric + hyphens + dots + underscores. Case-sensitive. |

**Requirements:**
- The `@id` of every PMEF object within a package **MUST** be unique within that package.
- The `@id` **SHOULD** be stable across revisions of the same
  physical object (i.e., it is the identity of the object, not of
  the revision).
- Adapters **MUST** preserve `@id` values on re-import (round-trip).

### 2.2 RdlUri

A URI pointing to a class in the ISO 15926-4 PCA-RDL, CFIHOS-RDL, or a project-level catalog.

**Requirements:**
- `rdlType` is **RECOMMENDED** on all physical and functional objects.
- When present, `rdlType` **SHOULD** resolve via the PCA-RDL
  SPARQL endpoint
  (`https://data.posccaesar.org/rdl/sparql`) or equivalent.
- Project-level catalog URIs **MUST** use the `urn:pmef:catalog:` prefix.

### 2.3 RevisionMetadata

The `RevisionMetadata` object carries ISO 19650 CDE workflow state and version history information.

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `revisionId` | **MUST** | string | Unique revision identifier. Format: `r<YYYY>-<MM>-<DD>-<NNN>` where NNN is a 3-digit sequence number. |
| `changeState` | **MUST** | enum | CDE workflow state: `WIP`, `SHARED`, `PUBLISHED`, `ARCHIVED`. |
| `parentRevisionId` | SHOULD | string \| null | RevisionId of the predecessor. Null for initial revisions. |
| `changeReason` | SHOULD | string | Human-readable description of why this revision was created. |
| `changedBy` | SHOULD | string | Identifier of the person or system that created this revision. |
| `changedAt` | SHOULD | string (ISO 8601) | Timestamp of the revision. |
| `authoringTool` | SHOULD | string | Source tool name and version, e.g. `"AVEVA E3D 4.2"`. |
| `authoringToolObjectId` | SHOULD | string | Native object ID in the authoring tool. Enables round-trip re-import. |
| `checksum` | MAY | string (SHA-256) | SHA-256 hash of the serialised object (excluding this field). |

### 2.4 GeometryReference

Links a PMEF object to one or more geometry representations.

| Field | Required | Type | Description |
|-------|----------|------|-------------|
| `type` | **MUST** | enum | `parametric`, `mesh_ref`, `step_ref`, `usd_ref`, `none` |
| `ref` | SHOULD | string (URI) | URI to the geometry object. |
| `boundingBox` | MAY | object | Axis-aligned bounding box in project coordinates [mm]. |
| `lod` | SHOULD | enum | Level of detail: `BBOX_ONLY`, `LOD1_COARSE`, `LOD2_MEDIUM`, `LOD3_FINE`, `LOD4_FABRICATION`. |

A PMEF object **MAY** have multiple geometry representations at
different LOD levels. In this case, the `geometry` field carries the
highest-LOD representation available, and lower-LOD representations
are referenced via `pmef:ParametricGeometry` objects.

### 2.5 CatalogReference

Links a PMEF object to a catalog entry in a piping specification
catalog, equipment class catalog, or steel profile catalog.

| Field | Description |
|-------|-------------|
| `catalogId` | Internal catalog identifier, e.g. `"A1A2"` for a pipe class. |
| `standard` | Applicable standard, e.g. `"ASME B16.5"`, `"EN 10253-2"`. |
| `rdlTypeUri` | RDL URI for the catalog entry. |
| `eclassIRDI` | eCl@ss IRDI classification code (required for CFIHOS/IOGP projects). |
| `vendorMappings` | Array of vendor-specific IDs for round-trip. |

### 2.6 Port

A physical connection point on a piping component or equipment nozzle.

| Field | Required | Description |
|-------|----------|-------------|
| `portId` | **MUST** | Unique ID within the component, e.g. `"P1"`, `"P2"`, `"BRANCH"`. |
| `portType` | SHOULD | `INLET`, `OUTLET`, `BIDIRECTIONAL`, `BRANCH`, `VENT`, `DRAIN`. Default: `BIDIRECTIONAL`. |
| `coordinate` | **MUST** | 3D coordinate of the port centreline [mm]. |
| `direction` | SHOULD | Unit vector pointing away from the component in the flow direction. |
| `nominalDiameter` | SHOULD | Port bore [mm]. |
| `endType` | SHOULD | Connection end type: `BW`, `FL`, `SW`, `SC`, `STUB`, `GROOVED`, `PLAIN`. |
| `connectedTo` | MAY | `PmefId` of the adjacent component's port. |

**Topology rules:**
- `connectedTo` **MUST** reference the `@id` of an adjacent
  `PipingComponent` or `Nozzle`, not a `Port` directly. The
  adjacent object is expected to have a reciprocal `connectedTo`
  reference.
- `connectedTo` references **MUST** resolve within the same PMEF
  package unless the reference carries the
  `urn:pmef:external:` prefix.

### 2.7 Coordinate System

All PMEF 3D coordinates are expressed in the project coordinate system with the following conventions:

| Axis | Direction | Unit |
|------|-----------|------|
| X | East (or project East) | mm |
| Y | North (or project North) | mm |
| Z | Up (elevation) | mm |

The coordinate system is right-handed. This convention is consistent
with AVEVA E3D, CADMATIC, AutoCAD Plant 3D, and PDMS.

**Conversion to other systems:**
- glTF 2.0 uses Y-up: apply a +90° rotation about the X axis when converting.
- OpenUSD uses Y-up by default but supports Z-up via stage metadata.
- STEP AP242 uses no fixed orientation; the world coordinate system is defined per file.

### 2.8 Units of Measure

PMEF uses the following units unconditionally. Adapters are responsible for all unit conversions.

| Quantity | PMEF Unit | QUDT symbol |
|----------|-----------|------------|
| Length | millimetre | `mm` |
| Pressure | pascal | `Pa` |
| Temperature | kelvin | `K` |
| Mass | kilogram | `kg` |
| Volumetric flow | cubic metre per hour | `m³/h` |
| Mass flow | kilogram per second | `kg/s` |
| Power | kilowatt | `kW` |
| Energy | kilojoule | `kJ` |
| Heat transfer coefficient | watt per square metre kelvin | `W/m²K` |
| Angle | degree | `°` (not radian) |
| Density | kilogram per cubic metre | `kg/m³` |

---

## 3 Plant Hierarchy

The plant hierarchy provides the containment context for all PMEF objects.

### 3.1 FileHeader

Every PMEF package **MUST** begin with a `pmef:FileHeader` object as the first line of the NDJSON file.

| Field | Required | Description |
|-------|----------|-------------|
| `@type` | **MUST** | `"pmef:FileHeader"` |
| `@id` | **MUST** | Package URI, e.g. `urn:pmef:pkg:proj:my-package` |
| `pmefVersion` | **MUST** | PMEF version string |
| `plantId` | **MUST** | `@id` of the associated `pmef:Plant` object |
| `projectCode` | SHOULD | Short project identifier |
| `coordinateSystem` | SHOULD | `"Z-up"` (default) or `"Y-up"` |
| `units` | SHOULD | `"mm"` (the only normalised value in v0.9) |
| `revisionId` | SHOULD | Package-level revision identifier |
| `changeState` | SHOULD | Package-level CDE state |
| `authoringTool` | SHOULD | Source tool |
| `description` | MAY | Human-readable description |

### 3.2 Plant

Represents the top-level facility.

| Field | Required | Description |
|-------|----------|-------------|
| `name` | **MUST** | Facility name |
| `location` | SHOULD | Geographic location (city, country) |
| `epsgCode` | MAY | EPSG coordinate system code for georeferencing |

### 3.3 Unit

Represents a process unit or functional area within the plant.

| Field | Required | Description |
|-------|----------|-------------|
| `unitNumber` | SHOULD | Unit number (e.g. `"U-100"`) |
| `unitName` | SHOULD | Unit name (e.g. `"Cooling Water Unit"`) |
| `processType` | MAY | Process type (e.g. `"UTILITIES"`, `"REACTION"`, `"SEPARATION"`) |
| `isPartOf` | **MUST** | `@id` of parent `pmef:Plant` |

### 3.4 Area

An optional subdivision of a Unit into physical areas.

| Field | Required | Description |
|-------|----------|-------------|
| `areaCode` | SHOULD | Area code (e.g. `"A-101"`) |
| `areaName` | SHOULD | Area name |
| `isPartOf` | **MUST** | `@id` of parent `pmef:Unit` |

---

## 4 Piping Domain

### 4.1 PipingNetworkSystem

Represents a complete piping line from the P&ID. Corresponds to
DEXPI `PipingNetworkSystem` and ISO 15926-14 `ProcessSystem`.

**Required fields:** `@type`, `@id`, `pmefVersion`, `lineNumber`, `isPartOf`

| Field | Description |
|-------|-------------|
| `lineNumber` | Full line number tag. **MUST** follow project tag convention, e.g. `"8"-CW-101-A1A2"`. |
| `nominalDiameter` | DN nominal diameter [mm]. |
| `pipeClass` | Piping class identifier (links to catalog). |
| `mediumCode` | Service medium code, e.g. `"CW"` (cooling water). |
| `fluidPhase` | `LIQUID`, `GAS`, `TWO_PHASE`, `SLURRY`, `STEAM`, `POWDER`. |
| `isPartOf` | `@id` of parent Unit or Area. |
| `isDerivedFrom` | `@id` of the DEXPI `PipingNetworkSystem` functional object. |
| `designConditions` | `PipingDesignConditions` property set. |
| `specification` | `PipingSpecification` property set. |
| `segments` | Ordered array of `PipingSegment` IDs. |

### 4.2 PipingSegment

A contiguous section of a piping line sharing a uniform specification. Maps to DEXPI `PipingNetworkSegment`.

**Required fields:** `@type`, `@id`, `isPartOf`

The `components` field carries an ordered list of `PipingComponent` IDs in routing sequence.

### 4.3 PipingComponent (Abstract Base)

All physical piping components inherit from this abstract base type.
Direct instantiation of `pmef:PipingComponent` is not permitted;
use the specific subtypes.

**Required fields for all subtypes:** `@type`, `@id`, `pmefVersion`, `isPartOf`, `componentSpec`

The `componentSpec` carries a `PipingComponentSpec` object with at minimum the `componentClass` field.

### 4.4 Component Subtypes

The following piping component subtypes are defined in v0.9:

| Subtype | Key additional fields | Notes |
|---------|----------------------|-------|
| `pmef:Pipe` | `pipeLength` [mm] | Straight run |
| `pmef:Elbow` | `angle` [°], `radius` enum | angle ∈ (0°, 360°] |
| `pmef:Tee` | `teeType`, `branchDiameter` [mm] | 3 ports |
| `pmef:Reducer` | `reducerType`, `largeDiameter`, `smallDiameter` [mm] | Concentric or eccentric |
| `pmef:Flange` | `flangeType`, `rating`, `facing` | 9 flange types |
| `pmef:Valve` | `valveSpec` (`ValveSpec`) | Includes actuator and leak data |
| `pmef:Olet` | `oletType`, `branchDiameter` [mm] | 8 olet types |
| `pmef:Gasket` | `gasketType`, `gasketMaterial` | |
| `pmef:Weld` | `weldSpec` (`WeldSpec`), `connects[2]` | `connects` MUST have exactly 2 IDs |
| `pmef:PipeSupport` | `supportSpec` (`SupportSpec`), `supportsMark` | 13 support types |
| `pmef:Spool` | `spoolMark`, `components[]`, `totalWeight` [kg] | Fabrication unit |

### 4.5 Topology Rules for Piping

1. Every `PipingComponent` that has physical connection points **MUST** have at least one `Port` in its `ports` array.
2. `Port.connectedTo` **MUST** reference the `@id` of an adjacent
   component. The fragment `#portId` MAY be appended to identify
   the specific port on the target object.
3. Connections between a piping component and an equipment nozzle
   are expressed via `Nozzle.connectedLineId` and
   `Nozzle.connectedPortId` on the equipment side.
4. The `Weld.connects` array **MUST** contain exactly two `PmefId`
   values identifying the two components joined by the weld.

---

## 5 Equipment Domain

### 5.1 EquipmentObject (Abstract Base)

All equipment objects inherit from this abstract base type.

**Required fields for all subtypes:** `@type`, `@id`, `pmefVersion`, `isPartOf`, `equipmentBasic`

The `equipmentBasic` carries an `EquipmentBasic` property set with at minimum `tagNumber` and `equipmentClass`.

### 5.2 Nozzle

The nozzle is the cross-domain connector between Equipment and
Piping. It is embedded as an array on each `EquipmentObject`.

**Required nozzle fields:** `nozzleId`, `nozzleMark`, `coordinate`, `direction`

| Field | Description |
|-------|-------------|
| `nozzleId` | Unique within the equipment object, e.g. `"SUCTION"`, `"DISCHARGE"`, `"N1"`. |
| `nozzleMark` | Nozzle mark from equipment drawing, e.g. `"A"`, `"N1"`. |
| `nominalDiameter` | Nozzle bore [mm]. |
| `flangeRating` | ANSI class or PN, e.g. `"ANSI-150"`, `"PN16"`. |
| `coordinate` | Position of nozzle face centreline [mm]. |
| `direction` | Unit vector pointing outward from the equipment (towards the piping). |
| `connectedLineId` | `@id` of the connected `PipingNetworkSystem`. |
| `connectedPortId` | Port ID on the first or last connected piping component. |

### 5.3 Equipment Subtypes

| Subtype | Key property set | Design code examples |
|---------|-----------------|---------------------|
| `pmef:Vessel` | `VesselDesign` | ASME VIII, EN 13445 |
| `pmef:Tank` | inline (capacity, diameter, etc.) | API 650, API 620, EN 14015 |
| `pmef:Pump` | `PumpSpec` | API 610, API 674 |
| `pmef:Compressor` | `CompressorSpec` | API 617, API 618, API 619 |
| `pmef:HeatExchanger` | `HeatExchangerSpec` | TEMA, ASME VIII |
| `pmef:Column` | `VesselDesign` + tray/packing | ASME VIII |
| `pmef:Reactor` | `VesselDesign` + reaction data | ASME VIII; EAF: proprietary |
| `pmef:Filter` | inline (filtration rating, ΔP) | — |
| `pmef:Turbine` | inline (pressures, temperatures) | API 611, API 612 |
| `pmef:GenericEquipment` | `EquipmentBasic` only | Fallback type |

### 5.4 Equipment Topology Rules

1. An equipment object's `nozzles` array **MAY** be empty for
   equipment without process connections (e.g. electrical panels).
2. `Nozzle.connectedLineId` **SHOULD** reference a
   `PipingNetworkSystem` present in the same PMEF package. If the
   line is in another package, the reference carries the
   `urn:pmef:external:` prefix.
3. The `isDerivedFrom` field on an equipment object **SHOULD**
   reference the DEXPI `Equipment` entity that represents the same
   functional position.

---

## 6 E&I Domain

### 6.1 InstrumentObject

Represents a physical field instrument, sensor, actuator, or transmitter.

**Required fields:** `@type`, `@id`, `pmefVersion`, `isPartOf`, `tagNumber`, `instrumentClass`

Key property groups:
- `measuredRange` — measurement/control range with unit.
- `alarmLimits` — L, LL, H, HH setpoints.
- `safetySpec` — SIL level, PFH, architecture (IEC 61508/61511).
- `connectionSpec` — signal type, ATEX zone, IP rating.
- `opcuaSpec` — OPC UA node reference and PA-DIM device type.
- `tiaPLCAddress` — TIA Portal I/O address for E-CAD synchronisation.

The `instrumentClass` field uses a controlled vocabulary of 30+ values based on DEXPI instrument types (TRANSMITTER, CONTROLLER, VALVE_CONTROL, SAFETY_ELEMENT, etc.).

### 6.2 InstrumentLoop

Groups the instruments in a functional control or safety loop.

| Field | Required | Description |
|-------|----------|-------------|
| `loopNumber` | **MUST** | Loop tag, e.g. `"FIC-101"`, `"PAHH-501"`. |
| `loopType` | SHOULD | `CONTROL`, `INDICATION`, `ALARM`, `SAFETY_INSTRUMENTED`, `MONITORING`, `SHUTDOWN`, `METERING`. |
| `memberIds` | SHOULD | Array of `InstrumentObject` IDs. |
| `controllerTagId` | SHOULD | ID of the primary controller instrument. |
| `finalElementTagId` | SHOULD | ID of the final control element (valve, motor). |
| `silLevel` | SHOULD | Required SIL level 0–4 for safety loops. |

### 6.3 PLCObject

Represents a PLC rack, CPU module, or I/O module. Derived from AutomationML AML AR APC hardware configuration.

The `amlRef` field carries the AutomationML `InternalElement` GUID, enabling synchronisation with TIA Portal, Rockwell Studio 5000, and Beckhoff TwinCAT AML exports.

### 6.4 CableObject

Represents an electrical cable or cable bundle between two termination points.

`fromId` and `toId` reference any PMEF object that can be a termination point: `InstrumentObject`, `PLCObject`, `CableTrayRun`, or another `CableObject` (for cable joints).

### 6.5 MTPModule

Represents a modular process unit described by an MTP 2.0 file. The MTP AML/AASX file is referenced via `mtpFileRef`. The `polEndpoint` carries the OPC UA endpoint URI for the Process Orchestration Layer.

---

## 7 Structural Steel Domain

### 7.1 SteelMember

The central structural entity. Represents a single beam, column, brace, or other structural member.

**Profile identifier convention:** `<standard>:<designation>`, e.g. `"EN:HEA200"`, `"AISC:W12x53"`, `"EN:RHS200x100x6"`.

The `cis2Ref` field carries the CIS/2 member identifier for round-trip with Tekla Structures and Advance Steel. The `teklaGUID` field carries the Tekla Open API GUID.

### 7.2 SteelConnection

Represents a structural connection — bolted, welded, or pinned. The `memberIds` array **MUST** contain at least two `SteelMember` IDs.

For bolted connections, `boltSpec` carries the bolt grade, diameter, and count. For welded connections, `weldSpec` carries weld type and size.

### 7.3 SteelNode

A topology node (joint) at which members connect. Used for FEM export and for associating boundary conditions with physical locations.

`supportType` defines the boundary condition: `FIXED`, `PINNED`, `ROLLER_X/Y/Z`, `SPRING`, `FREE`.

---

## 8 Relationship Model

### 8.1 Principles

PMEF relationship objects are first-class NDJSON objects (not just fields). This provides:

- **Versionability:** each relationship carries its own `RevisionMetadata`.
- **Provenance:** `derivedBy` and `confidence` fields document how the relationship was established.
- **Discoverability:** all relationships can be found by scanning for objects whose `@type` starts with `pmef:Is` or `pmef:Has`.
- **Cross-domain linking:** relationships connect objects from any two domains without modifying either object.

### 8.2 Base Relationship Fields

All relationship types extend a base structure:

| Field | Required | Description |
|-------|----------|-------------|
| `@type` | **MUST** | Relationship type URI |
| `@id` | **MUST** | Unique relationship object ID |
| `relationType` | **MUST** | Type code, mirrors `@type` suffix (e.g. `"IS_DERIVED_FROM"`) |
| `sourceId` | **MUST** | `@id` of the source object |
| `targetId` | **MUST** | `@id` of the target object |
| `confidence` | SHOULD | Float 0–1: 1.0=exact, <1.0=derived/inferred |
| `derivedBy` | SHOULD | `MANUAL`, `ADAPTER_IMPORT`, `AI_INFERRED`, `RULE_BASED` |
| `notes` | MAY | Free-text annotation |
| `revision` | SHOULD | `RevisionMetadata` |

### 8.3 Relationship Type Definitions

| Type | Direction | `sourceId` | `targetId` | Key additional fields |
|------|-----------|-----------|-----------|----------------------|
| `pmef:IsPartOf` | child → parent | any object | Plant/Unit/Area/System | — |
| `pmef:IsConnectedTo` | bidirectional | any object | any object | `connectionMedium`, `connectionPointSource/Target` |
| `pmef:IsDerivedFrom` | physical → functional | physical object | DEXPI/functional object | `sourceStandard`, `mappingVersion` |
| `pmef:Supports` | structure → load | SteelMember | PipeSupport/PipingLine | `loadTransferred` (6 DOF) |
| `pmef:ControlledBy` | equipment → instrument | valve/motor | instrument/loop | `controlMode`, `signalPath` |
| `pmef:IsDocumentedBy` | object → document | any object | document | `documentType`, `documentId`, `documentUri` |
| `pmef:IsRevisionOf` | new → old | new revision | old revision | `changeReason`, `changeType` |
| `pmef:HasEquivalentIn` | PMEF → native | any PMEF object | (itself) | `targetSystem`, `targetSystemId`, `mappingType` |
| `pmef:IsCollocatedWith` | bidirectional | any object | any object | — |
| `pmef:ReplacedBy` | old → new | old equipment | new equipment | `replacementDate`, `workOrderRef` |

### 8.4 HasEquivalentIn — Adapter Round-Trip

The `pmef:HasEquivalentIn` relationship **MUST** be written by every PMEF adapter for every exported object. It records the native tool's object identifier, enabling the adapter to find the object on re-import without coordinate-based or name-based matching.

```jsonc
{
  "@type": "pmef:HasEquivalentIn",
  "@id": "urn:pmef:rel:proj:P-201A-e3d",
  "relationType": "HAS_EQUIVALENT_IN",
  "sourceId": "urn:pmef:obj:proj:P-201A",
  "targetId": "urn:pmef:obj:proj:P-201A",
  "targetSystem": "AVEVA_E3D",
  "targetSystemId": "DB:PROJ:EQUIP:12345",
  "mappingType": "EXACT",
  "derivedBy": "ADAPTER_IMPORT",
  "confidence": 1.0
}
```

A Level 3 conformant adapter **MUST** write a `pmef:HasEquivalentIn` relationship for every exported object.

---

## 9 Property Sets

Property sets are reusable, typed collections of attributes. They are defined in `pmef-property-sets.schema.json` and referenced by entity types.

### 9.1 PipingDesignConditions

Carries the process design envelope for a piping line or segment.

All pressure values are in **Pa**. All temperature values are in **K**.

Key attributes: `designPressure`, `designTemperature`, `operatingPressure`, `operatingTemperature`, `testPressure`, `testMedium`, `vacuumService`, `fluidCategory` (PED), `pedCategory`.

### 9.2 PipingSpecification

Carries the pipe class and material specification.

Key attributes: `nominalDiameter` [mm], `outsideDiameter` [mm], `wallThickness` [mm], `schedule`, `pipeClass`, `material`, `pressureRating`, `corrosionAllowance` [mm], `insulationType`, `heatTracingType`.

### 9.3 PipingComponentSpec

Carries attributes common to all piping component subtypes.

Key attributes: `componentClass` (controlled vocabulary of 40+ values), `skey` (8-char PCF++ shape key), `endType1`, `endType2`, `facingType`, `faceToFace` [mm], `weight` [kg].

**SKEY convention:** 8 characters, format `<2-char type><2-char end-type><4-char variant>`. Example: `"ELBWLR90"` = elbow, butt-weld ends, long-radius 90°. The first 4 characters are PCF-compatible.

### 9.4 ValveSpec

Carries valve-specific attributes: `actuatorType`, `failPosition` (FO/FC/FL/FI), `leakageClass` (ANSI/FCI 70-2), `kvValue` [m³/h], `shutoffPressure` [Pa], `signalRange`, `positionFeedback`, `handwheelOverride`.

### 9.5 WeldSpec

Carries weld record attributes: `weldNumber`, `weldType` (BW/FW/SW), `weldingProcess`, `wpsNumber`, `pwht`, `ndeMethod`, `ndePercentage`, `inspectionLevel`, `inspectionStatus`.

### 9.6 EquipmentBasic

Carries tag number, class, service description, design code, manufacturer, model, serial number, and train identifier. This property set is common to all equipment subtypes.

### 9.7 VesselDesign

Carries pressure vessel design data per ASME VIII or EN 13445: design pressures (internal and external) [Pa], design temperatures [K], volume [m³], shell material, shell inside diameter [mm], tangent-to-tangent length [mm], head type, orientation, corrosion allowance [mm], NDE requirements, and fire protection.

### 9.8 PumpSpec

Carries centrifugal and positive displacement pump data per API 610/674/675: flow rates [m³/h], head [m], NPSH [m], efficiency [%], motor power [kW], speed [rpm], drive type, seal type, and H-Q curve reference.

### 9.9 HeatExchangerSpec

Carries shell-and-tube and other heat exchanger data per TEMA and ASME VIII: duty [W], heat transfer area [m²], overall U-value [W/m²K], tube details (OD, thickness, count, material), shell details, operating conditions for shell side and tube side, fouling factors.

### 9.10 CompressorSpec

Carries centrifugal and reciprocating compressor data per API 617/618/619: inlet flow [m³/h], pressure ratio, polytropic efficiency [%], shaft power [kW], driver type, number of stages, seal type.

### 9.11 SupportSpec

Carries pipe support design data: support type (13 types including spring hangers, constant hanger, anchor, guide), design loads [N, N·m], spring rate [N/mm], hot and cold loads [N], travel range [mm], attachment type.

---

## 10 Extension Mechanism

### 10.1 customAttributes

Every PMEF physical and functional object has an optional `customAttributes` field that accepts a JSON object with string, number, boolean, or null values:

```jsonc
"customAttributes": {
  "projectPBSCode": "PBS-U100-P-201A",
  "procurementPackage": "PP-0042",
  "clientReviewStatus": "APPROVED",
  "installationWeight_kg": 2450.0,
  "highPriorityEquipment": true
}
```

**Requirements:**
- Keys **MUST** be strings.
- Values **MUST** be one of: string, number, boolean, null.
- Keys **MUST NOT** shadow defined PMEF property names.
- `customAttributes` **MUST** be preserved on round-trip.

### 10.2 RFC Process for Promoting Extensions

Custom attributes that prove useful across multiple projects **SHOULD** be proposed for promotion to the PMEF information model via the RFC process (see [CONTRIBUTING.md](../CONTRIBUTING.md)). Once an RFC is accepted, the attribute moves from `customAttributes` to a named field in the relevant schema.

### 10.3 Custom Entity Types

Custom `@type` values **MAY** be used in PMEF packages, provided they are prefixed with a project- or vendor-specific namespace (not `pmef:`). PMEF readers **MUST NOT** fail on encountering unknown `@type` values.

```jsonc
{
  "@type": "myproject:CustomReactorType",
  "@id": "urn:pmef:obj:proj:CR-001",
  "pmefVersion": "0.9.0",
  "isPartOf": "urn:pmef:unit:proj:U-600"
}
```

---

*End of Chapter 02.*

**[← Chapter 01](01-introduction.md)** · **[Chapter 03 — Serialisation →](03-serialisation.md)**
