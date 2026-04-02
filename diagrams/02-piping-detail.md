# PMEF Piping Domain — Detailed Class Diagram

```mermaid
classDiagram
    direction TB

    class PipingNetworkSystem {
        +string lineNumber
        +number nominalDiameter
        +string pipeClass
        +string mediumCode
        +string fluidPhase
        +PmefId isPartOf
        +PmefId isDerivedFrom
        +Iec81346Designation iec81346
        +PipingDesignConditions designConditions
        +PipingSpecification specification
        +PmefId[] segments
        +string pidSheetRef
        +string isometricRef
        +RevisionMetadata revision
    }

    class PipingSegment {
        +integer segmentNumber
        +PmefId isPartOf
        +PipingSpecification specification
        +PipingDesignConditions designConditions
        +PmefId[] components
        +RevisionMetadata revision
    }

    class PipingComponent {
        <<abstract>>
        +string @type
        +PmefId @id
        +PmefId isPartOf
        +string tagNumber
        +string itemNumber
        +string heatNumber
        +PipingComponentSpec componentSpec
        +CatalogReference catalogRef
        +Port[] ports
        +GeometryReference geometry
        +RevisionMetadata revision
    }

    class Port {
        +string portId
        +string portType
        +Coordinate3D coordinate
        +UnitVector3D direction
        +number nominalDiameter
        +string endType
        +PmefId connectedTo
    }

    class PipingComponentSpec {
        +string componentClass
        +string skey
        +string endType1
        +string endType2
        +string facingType
        +number faceToFace
        +number centreToFace
        +number weight
        +string itemNumber
        +string spoolId
    }

    class PipingDesignConditions {
        +number designPressure Pa
        +number designTemperature K
        +number operatingPressure Pa
        +number operatingTemperature K
        +number minOperatingTemp K
        +number testPressure Pa
        +string testMedium
        +boolean vacuumService
        +string fluidCategory
        +string pedCategory
    }

    class PipingSpecification {
        +number nominalDiameter mm
        +number outsideDiameter mm
        +number wallThickness mm
        +string schedule
        +string pipeClass
        +string material
        +string pressureRating
        +number corrosionAllowance mm
        +string lineNumber
        +string insulationType
        +number insulationThickness mm
        +string heatTracingType
    }

    class Pipe {
        +type = pmef:Pipe
        +number pipeLength mm
        +string spoolMark
    }

    class Elbow {
        +type = pmef:Elbow
        +number angle degrees
        +string radius
        +number radiusMm
    }

    class Tee {
        +type = pmef:Tee
        +string teeType
        +number branchDiameter mm
        +number branchAngle degrees
    }

    class Reducer {
        +type = pmef:Reducer
        +string reducerType
        +number largeDiameter mm
        +number smallDiameter mm
        +string eccentricFlat
    }

    class Flange {
        +type = pmef:Flange
        +string flangeType
        +string rating
        +string facing
        +number boreDiameter mm
    }

    class Valve {
        +type = pmef:Valve
        +ValveSpec valveSpec
        +string instrumentTag
        +string normalPosition
    }

    class ValveSpec {
        +string actuatorType
        +string failPosition
        +string leakageClass
        +number kvValue m³/h
        +number shutoffPressure Pa
        +string signalRange
        +boolean positionFeedback
    }

    class Olet {
        +type = pmef:Olet
        +string oletType
        +number branchDiameter mm
    }

    class Gasket {
        +type = pmef:Gasket
        +string gasketType
        +string gasketMaterial
    }

    class Weld {
        +type = pmef:Weld
        +WeldSpec weldSpec
        +PmefId[2] connects
    }

    class WeldSpec {
        +string weldNumber
        +string weldType
        +string weldingProcess
        +string wpsNumber
        +boolean pwht
        +string ndeMethod
        +number ndePercentage
        +string inspectionStatus
    }

    class PipeSupport {
        +type = pmef:PipeSupport
        +SupportSpec supportSpec
        +string supportsMark
        +PmefId structuralAttachmentId
    }

    class SupportSpec {
        +string supportType
        +number designLoadFx N
        +number designLoadFy N
        +number designLoadFz N
        +number springRate N/mm
        +number hotLoad N
        +number coldLoad N
    }

    class Spool {
        +type = pmef:Spool
        +string spoolMark
        +PmefId isPartOf
        +PmefId[] components
        +number totalWeight kg
        +string fabricationLocation
    }

    PipingNetworkSystem "1" *-- "1..*" PipingSegment : segments
    PipingSegment "1" *-- "1..*" PipingComponent : components
    PipingComponent "1" *-- "1..*" Port : ports
    PipingComponent *-- PipingComponentSpec : componentSpec

    PipingNetworkSystem *-- PipingDesignConditions : designConditions
    PipingNetworkSystem *-- PipingSpecification : specification
    PipingSegment *-- PipingSpecification : specification

    PipingComponent <|-- Pipe
    PipingComponent <|-- Elbow
    PipingComponent <|-- Tee
    PipingComponent <|-- Reducer
    PipingComponent <|-- Flange
    PipingComponent <|-- Valve
    PipingComponent <|-- Olet
    PipingComponent <|-- Gasket
    PipingComponent <|-- Weld
    PipingComponent <|-- PipeSupport

    Valve *-- ValveSpec : valveSpec
    Weld *-- WeldSpec : weldSpec
    PipeSupport *-- SupportSpec : supportSpec
    PipingNetworkSystem ..> Spool : references
```

---

## PCF → PMEF Field Mapping

| PCF Record/Field | PMEF Field | Notes |
|-----------------|-----------|-------|
| `PIPELINE-REFERENCE` | `PipingNetworkSystem.lineNumber` | Full line number tag |
| `PIPELINE-REF SPOOL-ID` | `PipingNetworkSystem.segments[].spoolId` | Segment-level |
| Component type keyword (e.g. `ELBOW`) | `PipingComponent.componentSpec.componentClass` | Normalised PMEF enum |
| `SKEY` | `PipingComponent.componentSpec.skey` | Extended 8-char in PMEF |
| `END-POINT X Y Z BORE` | `PipingComponent.ports[].coordinate` + `.nominalDiameter` | mm in PMEF (PCF may be inch) |
| `MATERIAL-IDENTIFIER` | `PipingComponent.catalogRef.catalogId` | Normalised via catalog |
| `ATTRIBUTE0..99` | `PipingComponent.customAttributes{}` | Typed in PMEF |
| `TEMPERATURE` | `PipingNetworkSystem.designConditions.operatingTemperature` | K in PMEF |
| `MAX-TEMPERATURE` | `PipingNetworkSystem.designConditions.designTemperature` | K |
| `MAX-PRESSURE` | `PipingNetworkSystem.designConditions.designPressure` | Pa in PMEF |

---

## Port Connectivity Model

```mermaid
graph LR
    P1[Pipe\nP-001\nport P2] -->|connectedTo| F1
    F1[Flange\nFL-001\nport P1] -->|connectedTo| P1
    F1 -->|bolted to| G1[Gasket\nGK-001]
    G1 -->|bolted to| F2[Flange\nFL-002\nport P1 on Valve]
    F2 -->|connectedTo| V1[Valve\nXV-101\nport P1]
    V1 -->|port P2| F3[Flange\nFL-003]
    F3 -->|connectedTo| NZ[Nozzle N1\non Vessel V-101]
```

Ports resolve to a topology graph: `PipingComponent.ports[].connectedTo` → `Port.@id` on the adjacent component.
This graph is the PMEF equivalent of PCF coordinate-based connectivity.
