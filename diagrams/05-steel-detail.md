# PMEF Structural Steel Domain — Class Diagram

```mermaid
classDiagram
    direction TB

    class SteelSystem {
        +type = pmef:SteelSystem
        +string systemName
        +string systemType
        +string designCode
        +string steelGrade
        +PmefId[] memberIds
        +RevisionMetadata revision
    }

    class SteelMember {
        +type = pmef:SteelMember
        +string memberMark
        +string memberType
        +string profileId
        +Coordinate3D startPoint mm
        +Coordinate3D endPoint mm
        +number rollAngle degrees
        +number length mm
        +Material material
        +number weight kg
        +string finish
        +FireProtection fireProtection
        +AnalysisResults analysisResults
        +string cis2Ref
        +string teklaGUID
        +GeometryReference geometry
        +RevisionMetadata revision
    }

    class Material {
        +string grade
        +string standard
        +number density kg/m³
        +number fy MPa
        +number fu MPa
    }

    class FireProtection {
        +string type
        +number requiredPeriod minutes
        +number sectionFactor 1/m
    }

    class AnalysisResults {
        +number utilisationRatio 0-1
        +string criticalCheck
        +number axialForce kN
        +number majorBending kN·m
        +number minorBending kN·m
        +number shearForce kN
    }

    class SteelNode {
        +type = pmef:SteelNode
        +integer nodeNumber
        +Coordinate3D coordinate
        +PmefId[] memberIds
        +PmefId connectionId
        +string supportType
        +RevisionMetadata revision
    }

    class SteelConnection {
        +type = pmef:SteelConnection
        +string connectionMark
        +string connectionType
        +PmefId[] memberIds min 2
        +Coordinate3D coordinate
        +BoltSpec boltSpec
        +WeldSpec weldSpec
        +DesignCapacity designCapacity
        +number utilisationRatio
        +integer teklaConnectionNumber
        +RevisionMetadata revision
    }

    class BoltSpec {
        +string boltGrade
        +number boltDiameter mm
        +integer numberOfBolts
        +string holeType
        +boolean preloaded
    }

    class DesignCapacity {
        +number shear kN
        +number moment kN·m
        +number axial kN
    }

    SteelSystem "1" *-- "1..*" SteelMember : memberIds
    SteelMember *-- Material : material
    SteelMember *-- FireProtection : fireProtection
    SteelMember *-- AnalysisResults : analysisResults
    SteelNode "1" --> "2..*" SteelMember : connects
    SteelNode "1" --> "0..1" SteelConnection : connectionId
    SteelConnection *-- BoltSpec : boltSpec
    SteelConnection *-- DesignCapacity : designCapacity
    SteelConnection "1" --> "2..*" SteelMember : memberIds
```

---

## Profile ID Convention

PMEF profile IDs use the format `<standard>:<designation>`:

| Standard | Examples |
|----------|---------|
| `EN` | `EN:HEA200`, `EN:IPE300`, `EN:UPE200`, `EN:RHS200x100x6`, `EN:CHS219.1x8` |
| `AISC` | `AISC:W12x53`, `AISC:HSS6x4x0.25`, `AISC:L4x4x0.5` |
| `FLAT` | `FLAT:200x20` (flat bar, width×thickness) |
| `ROUND` | `ROUND:50` (solid rod, diameter) |
| `CUSTOM` | `CUSTOM:<project-code>` (project-specific profiles) |

---

## CIS/2 → PMEF Mapping

| CIS/2 entity | PMEF type | Notes |
|-------------|-----------|-------|
| `StructuralMember` | `pmef:SteelMember` | 1:1 |
| `Connection` | `pmef:SteelConnection` | 1:1 |
| `Node` | `pmef:SteelNode` | 1:1 |
| `Structure` | `pmef:SteelSystem` | 1:1 |
| `Material` | `SteelMember.material` | embedded |
| `CrossSection` | `SteelMember.profileId` | PMEF catalog ref |
| `EndRelease` | `SteelConnection.connectionType` | PINNED / MOMENT_RIGID |
| `BoundaryCondition` | `SteelNode.supportType` | FIXED / PINNED / ROLLER_* |

---

## Tekla Structures ↔ PMEF Round-Trip

| Tekla attribute | PMEF field |
|----------------|-----------|
| `GUID` | `SteelMember.teklaGUID` |
| `Name` (mark) | `SteelMember.memberMark` |
| `Profile` | `SteelMember.profileId` |
| `Material.Grade` | `SteelMember.material.grade` |
| `StartPoint` | `SteelMember.startPoint` |
| `EndPoint` | `SteelMember.endPoint` |
| `ConnectionNumber` | `SteelConnection.teklaConnectionNumber` |
| `PartNumber` | `SteelMember.memberMark` |
