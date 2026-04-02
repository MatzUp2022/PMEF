# PMEF E&I Domain — Detailed Class Diagram

```mermaid
classDiagram
    direction TB

    class InstrumentObject {
        +type = pmef:InstrumentObject
        +string tagNumber
        +string instrumentClass
        +string processVariable
        +string serviceDescription
        +string loopNumber
        +MeasuredRange measuredRange
        +AlarmLimits alarmLimits
        +SafetySpec safetySpec
        +ConnectionSpec connectionSpec
        +OpcuaSpec opcuaSpec
        +string comosCUID
        +string eplanBKZ
        +TiaPLCAddress tiaPLCAddress
        +Iec81346Designation iec81346
        +GeometryReference geometry
        +RevisionMetadata revision
    }

    class SafetySpec {
        +integer safetyIntegrityLevel 0-4
        +string safetyFunction
        +string architectureType
        +number pfh per hour
        +number pfd
        +number proofTestInterval hours
        +string safeState
    }

    class ConnectionSpec {
        +string signalType
        +boolean failSafe
        +boolean loopPowered
        +boolean intrinsicSafe
        +string hazardousArea
        +string protectionType
        +string ipRating
    }

    class OpcuaSpec {
        +string nodeRef URI
        +string padimType URI
        +string mtpDataAssemblyRef
    }

    class TiaPLCAddress {
        +integer rack
        +integer slot
        +integer channel
        +string symbol
        +string dataType
    }

    class InstrumentLoop {
        +type = pmef:InstrumentLoop
        +string loopNumber
        +string loopType
        +PmefId[] memberIds
        +string controllerTagId
        +string finalElementTagId
        +Setpoint setpoint
        +integer silLevel 0-4
        +RevisionMetadata revision
    }

    class PLCObject {
        +type = pmef:PLCObject
        +string plcClass
        +string vendor
        +string family
        +string articleNumber
        +integer rack
        +integer slot
        +string ipAddress
        +boolean safetyCPU
        +string opcuaNodeRef
        +string amlRef
        +RevisionMetadata revision
    }

    class CableObject {
        +type = pmef:CableObject
        +string cableNumber
        +string cableType
        +number crossSection mm²
        +integer numberOfCores
        +number voltageRating V
        +string screenType
        +boolean armoured
        +boolean intrinsicSafe
        +PmefId fromId
        +string fromTerminal
        +PmefId toId
        +string toTerminal
        +PmefId cableTrayId
        +number routeLength m
        +RevisionMetadata revision
    }

    class CableTrayRun {
        +type = pmef:CableTrayRun
        +string trayMark
        +string trayType
        +number width mm
        +number height mm
        +number fillLevel pct
        +PmefId structuralSupportId
        +RevisionMetadata revision
    }

    class MTPModule {
        +type = pmef:MTPModule
        +string moduleName
        +string mtpVersion
        +DocumentLink mtpFileRef
        +string polEndpoint URI
        +PmefId[] memberIds
        +RevisionMetadata revision
    }

    InstrumentObject *-- SafetySpec : safetySpec
    InstrumentObject *-- ConnectionSpec : connectionSpec
    InstrumentObject *-- OpcuaSpec : opcuaSpec
    InstrumentObject *-- TiaPLCAddress : tiaPLCAddress

    InstrumentLoop "1" --> "1..*" InstrumentObject : memberIds
    PLCObject "1" --> "0..*" InstrumentObject : hosts channels

    CableObject --> InstrumentObject : fromId/toId
    CableObject --> PLCObject : fromId/toId
    CableTrayRun "1" --> "0..*" CableObject : routes

    MTPModule "1" --> "1..*" InstrumentObject : memberIds
    MTPModule --> OpcuaSpec : polEndpoint
```

---

## Standards Alignment

| PMEF Field | Upstream Standard |
|-----------|------------------|
| `tagNumber` | ISA 5.1, IEC 62424 |
| `instrumentClass` | DEXPI 2.0 instrument types |
| `safetySpec.safetyIntegrityLevel` | IEC 61508, IEC 61511 |
| `safetySpec.architectureType` | IEC 61508-6 Annex B |
| `connectionSpec.signalType` | IEC 61158 (fieldbus standards) |
| `connectionSpec.protectionType` | IEC 60079 (ATEX/IECEx) |
| `opcuaSpec.nodeRef` | OPC UA Part 6 (URI addressing) |
| `opcuaSpec.padimType` | PA-DIM (OPC 30500) DeviceType |
| `tiaPLCAddress` | TIA Portal Openness API |
| `comosCUID` | COMOS platform UID |
| `eplanBKZ` | EPLAN IEC 81346 BKZ |
| `MTPModule.polEndpoint` | MTP 2.0 (VDI/VDE/NAMUR 2658) |
| `MTPModule.mtpFileRef` | AML/AASX (IEC 62424) |

---

## Signal Type → Physical Interface Mapping

| `signalType` | Cable type | Typical loop | Protocol |
|-------------|-----------|-------------|---------|
| `4_20MA` | Instrumentation pair, shielded | AI/AO channel | Analog |
| `HART` | Same as 4-20mA | AI channel + HART modem | Digital overlay |
| `PROFIBUS_PA` | Special PA cable (blue) | DP/PA coupler | IEC 61158-2 |
| `FOUNDATION_FIELDBUS` | FF cable (orange) | FF segment | IEC 61158 |
| `PROFINET` | CAT5e/6 or fiber | Profinet switch | IEEE 802.3 |
| `DISCRETE_24VDC` | Control cable | DI/DO channel | — |

---

## COMOS ↔ PMEF Round-Trip Key

| COMOS attribute | PMEF field |
|----------------|-----------|
| `CUID` | `InstrumentObject.comosCUID` |
| `Tag` | `InstrumentObject.tagNumber` |
| `BKZ (IEC 81346)` | `InstrumentObject.iec81346.functionalAspect` |
| `SIL Level` | `InstrumentObject.safetySpec.safetyIntegrityLevel` |
| `AML InternalElement ID` | `PLCObject.amlRef` |
| `MTP file reference` | `MTPModule.mtpFileRef` |
