# pmef-comos-dotnet

C# COMOS .NET API connector for PMEF.

## Components

| File | Purpose |
|------|---------|
| `src/ComosExporter.cs` | Read COMOS plant model → write `comos-export.json` |

## Prerequisites

- Siemens COMOS 10.4 or newer
- .NET 8 SDK (Windows)
- COMOS open with a project loaded

## Build

```bat
cd pmef-comos-dotnet
dotnet build -c Release
```

## Export (COMOS → PMEF)

```bat
# With COMOS open and a project loaded:
ComosExporter.exe comos-export.json

# Then process with the Rust adapter:
pmef convert comos-export.json --from comos --to pmef --output output.ndjson
```

## COMOS Coverage

| COMOS Class | Objects Exported | PMEF Types |
|-------------|-----------------|-----------|
| `@E03` / `@E03.1` | Pumps (centrifugal/reciprocating) | `pmef:Pump` |
| `@E04` | Compressors | `pmef:Compressor` |
| `@E05` / `@E05.1/2` | Heat exchangers (S&T, plate, air) | `pmef:HeatExchanger` |
| `@E06` / `@E06.3` | Reactors / EAF | `pmef:Reactor` |
| `@E07` | Pressure vessels / drums | `pmef:Vessel` |
| `@E08` | Tanks | `pmef:Tank` |
| `@E09` | Filters / strainers | `pmef:Filter` |
| `@E10` | Turbines | `pmef:Turbine` |
| `@L10` | Piping lines | `pmef:PipingNetworkSystem` |
| `@I10.F/P/T/L/A` | Transmitters (flow/press/temp/level/analysis) | `pmef:InstrumentObject` |
| `@I20` | Controllers | `pmef:InstrumentObject` |
| `@I30` / `@I30.V` | Control valves / final elements | `pmef:InstrumentObject` |
| `@I40` | Safety elements (SIL) | `pmef:InstrumentObject` |
| `@I05` | Instrument loops | `pmef:InstrumentLoop` |
| `@K` | Cables | `pmef:CableObject` |
| `@S10` | PLC CPUs (incl. safety CPUs) | `pmef:PLCObject` |
| `@S20` | I/O modules | `pmef:PLCObject` |
| `@N` | Nozzles (sub-objects of equipment) | Embedded in equipment |

## COMOS Attributes Mapped

The exporter reads the following standard COMOS CTA (COMOS Technical Attribute) fields:

### Equipment (`@E`)
`CTA_DesignPressure`, `CTA_DesignTemperature`, `CTA_OperatingPressure`,
`CTA_OperatingTemperature`, `CTA_Volume`, `CTA_Material`, `CTA_DesignCode`,
`CTA_Weight`, `CTA_OperatingWeight`, `CTA_Manufacturer`, `CTA_Type`,
`CTA_MotorPower`, `CTA_FlowDesign`, `CTA_Head`, `CTA_Duty`,
`CTA_HeatTransferArea`, `CTA_TEMAType`, `CTA_InsideDiameter`,
`CTA_LengthTangentTangent`, `CTA_ShellPressure`, `CTA_TubePressure`,
`CTA_PIDReference`, `CTA_Status`, `CTA_FunctionalDesignation`, `CTA_ProductDesignation`

### Instruments (`@I`)
`CTA_ProcessVariable`, `CTA_RangeMin`, `CTA_RangeMax`, `CTA_Unit`,
`CTA_SignalType`, `CTA_FailSafe`, `CTA_SIL`, `CTA_ProofTestInterval`,
`CTA_PFD`, `CTA_PFH`, `CTA_Architecture`, `CTA_SafeState`,
`CTA_ExProtection`, `CTA_HazArea`, `CTA_IPRating`,
`CTA_TIAAddress`, `CTA_EPLANFunctionText`, `CTA_Kv`,
`CTA_ShutoffClass`, `CTA_ActuatorType`, `CTA_Manufacturer`, `CTA_Model`

### Piping lines (`@L10`)
`CTA_LineNumber`, `CTA_NominalDiameter`, `CTA_PipeClass`, `CTA_MediumCode`,
`CTA_Medium`, `CTA_DesignPressure`, `CTA_DesignTemperature`,
`CTA_OperatingPressure`, `CTA_OperatingTemperature`, `CTA_TestPressure`,
`CTA_Material`, `CTA_Insulation`, `CTA_HeatTracing`

### PLC (`@S`)
`CTA_Manufacturer`, `CTA_Family`, `CTA_ArticleNumber`, `CTA_Rack`,
`CTA_Slot`, `CTA_IPAddress`, `CTA_SafetyCPU`, `CTA_TIAPortalReference`,
`CTA_AMLReference`

## P&ID Integration

COMOS is the primary source for P&ID attributes. When a 3D model (from E3D or
Plant3D) is also available, the two PMEF packages are linked via:

```jsonc
{
  "@type": "pmef:HasEquivalentIn",
  "sourceId":     "urn:pmef:obj:eaf-2026:P-201A",   // COMOS-sourced object
  "targetSystem": "AVEVA_E3D",
  "targetSystemId": "/SITE01/ZONE-U100/EQUI-P-201A", // E3D DB address
}
```

This allows combining the full P&ID attribute set from COMOS with the 3D
geometry from the 3D tool adapter into a single PMEF package.
