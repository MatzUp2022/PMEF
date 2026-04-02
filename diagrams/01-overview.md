# PMEF Data Model — Domain Overview

Rendered with [Mermaid](https://mermaid.js.org). Open on [mermaid.live](https://mermaid.live) to view interactively.

---

## Top-Level Domain Diagram

```mermaid
classDiagram
    direction TB

    class PlantHierarchy {
        <<abstract>>
        +PmefId @id
        +string name
        +RevisionMetadata revision
    }

    class Plant {
        +string plantName
        +string location
        +Coordinate3D origin
        +string epsgCode
    }

    class Unit {
        +string unitNumber
        +string unitName
        +string processType
    }

    class Area {
        +string areaCode
        +string areaName
    }

    class PipingNetworkSystem {
        +string lineNumber
        +number nominalDiameter
        +string pipeClass
        +string mediumCode
        +PipingDesignConditions designConditions
        +PipingSpecification specification
    }

    class PipingSegment {
        +integer segmentNumber
        +PipingSpecification specification
    }

    class PipingComponent {
        <<abstract>>
        +PipingComponentSpec componentSpec
        +Port[] ports
        +GeometryReference geometry
    }

    class EquipmentObject {
        <<abstract>>
        +EquipmentBasic equipmentBasic
        +Nozzle[] nozzles
        +GeometryReference geometry
    }

    class Nozzle {
        +string nozzleId
        +string nozzleMark
        +Coordinate3D coordinate
        +UnitVector3D direction
        +number nominalDiameter
        +string flangeRating
    }

    class FunctionalObject {
        <<DEXPI/ISO 15926>>
        +string dexpiTag
        +string instrumentFunction
    }

    PlantHierarchy <|-- Plant
    PlantHierarchy <|-- Unit
    PlantHierarchy <|-- Area
    Plant "1" *-- "1..*" Unit : contains
    Unit  "1" *-- "0..*" Area : contains
    Unit  "1" *-- "0..*" PipingNetworkSystem : isPartOf
    Unit  "1" *-- "0..*" EquipmentObject : isPartOf
    Area  "1" *-- "0..*" PipingNetworkSystem : isPartOf

    PipingNetworkSystem "1" *-- "1..*" PipingSegment : segments
    PipingSegment "1" *-- "1..*" PipingComponent : components

    EquipmentObject "1" *-- "0..*" Nozzle : nozzles
    Nozzle "0..1" ..> "0..1" PipingComponent : connectedTo

    PipingNetworkSystem ..> FunctionalObject : isDerivedFrom
    EquipmentObject ..> FunctionalObject : isDerivedFrom
```

---

## Standards Mapping Overview

```mermaid
classDiagram
    direction LR

    class PMEF_Object {
        +PmefId @id
        +string @type
        +RdlUri rdlType
        +Iec81346Designation iec81346
        +CatalogReference catalogRef
        +RevisionMetadata revision
        +GeometryReference geometry
    }

    class ISO15926_IDO {
        <<ISO 15926-14>>
        +PossibleIndividual
        +PhysicalObject
        +FunctionalObject
        +ClassificationOfIndividual
    }

    class DEXPI_20 {
        <<DEXPI 2.0>>
        +PipingNetworkSystem
        +Equipment classes
        +ProcessInstrumentationFunction
    }

    class IEC81346 {
        <<IEC 81346>>
        +functionalAspect (=)
        +productAspect (-)
        +locationAspect (+)
    }

    class CFIHOS_V2 {
        <<CFIHOS V2.0>>
        +Tag class
        +Equipment class
        +665+ attributes
    }

    class PCA_RDL {
        <<ISO 15926-4>>
        +12,000+ classes
        +SPARQL endpoint
    }

    PMEF_Object --> ISO15926_IDO : rdlType maps to
    PMEF_Object --> DEXPI_20 : entity types from
    PMEF_Object --> IEC81346 : iec81346 uses
    PMEF_Object --> CFIHOS_V2 : property sets from
    PMEF_Object --> PCA_RDL : rdlType resolves in
    ISO15926_IDO --> PCA_RDL : Reference Data
    DEXPI_20 --> PCA_RDL : classifies via
    CFIHOS_V2 --> PCA_RDL : mapped to
```
