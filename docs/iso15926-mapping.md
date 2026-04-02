# PMEF ↔ ISO 15926 / DEXPI Alignment

## ISO 15926-2 Core Entity Mapping

| ISO 15926-2 Class | PMEF Type | Notes |
|------------------|-----------|-------|
| `PossibleIndividual` | Any PMEF object | Base type for all spatio-temporal entities |
| `InanimatePhysicalObject` | `PipingComponent`, `EquipmentObject` | Physical hardware |
| `FunctionalObject` | Mapped via `isDerivedFrom` | P&ID tag / functional position |
| `ClassificationOfIndividual` | `rdlType` URI | Links to PCA-RDL class |
| `CompositionOfIndividual` | `isPartOf` relationship | Containment hierarchy |
| `ConnectionOfIndividual` | `Port.connectedTo` | Topological connection |
| `WholeLifeIndividual` | `revisionId` chain | Full lifecycle identity |
| `NonActualIndividual` | `changeState: WIP` | Design objects not yet built |

## DEXPI 2.0 Entity → PMEF Type Mapping

| DEXPI 2.0 Class | PMEF Type | Mapping Notes |
|----------------|-----------|----------------|
| `PipingNetworkSystem` | `pmef:PipingNetworkSystem` | Direct 1:1; PMEF adds 3D geometry |
| `PipingNetworkSegment` | `pmef:PipingSegment` | PMEF adds ordered component list |
| `PipingComponent` (abstract) | `pmef:PipingComponent` (abstract) | PMEF extends with full Port model |
| `Valve` | `pmef:Valve` | PMEF adds ValveSpec with actuator data |
| `Equipment` (abstract) | `pmef:EquipmentObject` (abstract) | PMEF adds nozzle array and geometry |
| `Vessel` | `pmef:Vessel` | PMEF adds VesselDesign property set |
| `Pump` | `pmef:Pump` | PMEF adds full PumpSpec (API 610) |
| `HeatExchanger` | `pmef:HeatExchanger` | PMEF adds full HeatExchangerSpec (TEMA) |
| `ProcessInstrumentationFunction` | `pmef:InstrumentObject.isDerivedFrom` | Functional → Physical linking |
| `SignalConveyingFunction` | `pmef:InstrumentLoop` (E&I module) | Signal chain |
| (no DEXPI equivalent) | `pmef:Elbow`, `pmef:Flange`, etc. | PMEF provides full PCF-level component granularity |

## P&ID → 3D Linking Pattern

The `isDerivedFrom` relationship is the PMEF mechanism for linking the functional
(P&ID) world to the physical (3D) world:

```
DEXPI FunctionalObject                PMEF PhysicalObject
──────────────────────                ───────────────────
PipingNetworkSystem                   pmef:PipingNetworkSystem
  @id: urn:pmef:functional:CW-201       @id: urn:pmef:line:CW-201
  (from DEXPI XML)                      isDerivedFrom: urn:pmef:functional:CW-201

Pump (DEXPI tag)                      pmef:Pump
  @id: urn:pmef:functional:P-201A       @id: urn:pmef:obj:P-201A
  tagNumber: P-201A                     isDerivedFrom: urn:pmef:functional:P-201A
```

This mapping enables:
1. Tag-based cross-referencing (P&ID tag = 3D equipment tag)
2. Completeness checking (every DEXPI tag should have a PMEF PhysicalObject)
3. Round-trip to ERP/EAM (SAP Equipment = PMEF PhysicalObject = DEXPI FunctionalObject)

## PCA-RDL SPARQL Query Example

To resolve a `rdlType` URI to its human-readable class label:

```sparql
PREFIX rdl: <http://data.posccaesar.org/rdl/>
PREFIX skos: <http://www.w3.org/2004/02/skos/core#>

SELECT ?label ?definition WHERE {
  <http://data.posccaesar.org/rdl/RDS354645>
    skos:prefLabel ?label ;
    skos:definition ?definition .
  FILTER(LANG(?label) = "en")
}
```

SPARQL endpoint: `https://data.posccaesar.org/rdl/sparql`
