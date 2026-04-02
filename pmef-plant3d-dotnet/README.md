# pmef-plant3d-dotnet

C# AutoCAD Plant 3D SDK connector for PMEF.

## Components

| File | Purpose |
|------|---------|
| `src/PlantExporter.cs` | AutoCAD command `PMEFEXPORT` — reads Plant 3D PDS → writes `plant3d-equipment.json` |
| (from Rust) `pmef-adapter-plant3d` | Reads PCF/IDF files + equipment JSON → writes PMEF NDJSON |
| `src/PlantExporter.cs` | AutoCAD command `PMEFIMPORT` — reads PMEF NDJSON → updates Plant 3D PDS |

## Prerequisites

- AutoCAD Plant 3D 2024 or newer
- .NET 8 SDK (Windows, x64)
- AutoCAD Plant SDK (installed with Plant 3D)

## Build

```bat
cd pmef-plant3d-dotnet
dotnet build -c Release
```

Copy the output DLL to the Plant 3D support path.

## Usage — Export

**Inside AutoCAD Plant 3D:**

```
Command: NETLOAD
Select: PmefPlant3D.dll

Command: PMEFEXPORT
PMEF export path [equipment.json]: plant3d-equipment.json
```

This writes a JSON file with all equipment objects and their attributes.

**Then process with Rust:**

```bash
# Process PCF files (one per line) + equipment JSON
pmef convert --from plant3d \
    --pcf-dir exports/pcf/ \
    --equipment plant3d-equipment.json \
    --to pmef \
    --output output.ndjson
```

## Usage — Import

```
Command: PMEFIMPORT
PMEF import path [output.ndjson]: output.ndjson
```

The importer:
1. Reads all `pmef:HasEquivalentIn` with `targetSystem = "PLANT3D"`
2. Matches existing Plant 3D equipment by DWG handle
3. Updates engineering properties (design pressure, temperature, material)
4. Note: geometry creation is not supported — import only updates existing objects

## Data flow

```
AutoCAD Plant 3D Model
  │
  ├─ PCF Export (per line) ──────────────────────→ line-CW201.pcf
  │   (File → Export → Piping → PCF)                    │
  │                                                      │
  ├─ IDF Export (per spool)  ─────────────────────→ spool-001.idf
  │   (AutoISO)                                          │
  │                                                      │
  └─ PMEFEXPORT command ──────────────────────────→ plant3d-equipment.json
              │                                          │
              └──────────────────────────────────────────┘
                                                         │
                              pmef-adapter-plant3d (Rust)
                                                         │
                                                         ▼
                                                  output.ndjson
```

## Property Mapping

### Equipment → PMEF (export)

| Plant 3D Property | PMEF field | Unit conversion |
|------------------|-----------|----------------|
| `TagNumber` | `equipmentBasic.tagNumber` | — |
| `Category.CategoryName` | `equipmentBasic.equipmentClass` | → PMEF class |
| `Description` | `equipmentBasic.serviceDescription` | — |
| `DesignPressure` | `customAttributes.designPressure_Pa` | psig → Pa abs |
| `DesignTemperature` | `customAttributes.designTemperature_K` | °F → K |
| `OperatingPressure` | `customAttributes.operatingPressure_Pa` | psig → Pa abs |
| `OperatingTemperature` | `customAttributes.operatingTemperature_K` | °F → K |
| `Material` | `customAttributes.material` | → PMEF material string |
| `DesignCode` | `equipmentBasic.designCode` | — |
| `Weight` | `customAttributes.weightKg` | lbs × 0.453592 |
| `MotorPower` | `customAttributes.motorPower_W` | hp × 745.7 |
| `DesignFlow` | `customAttributes.designFlow_m3h` | gpm × 0.227125 |
| `DesignHead` | `customAttributes.designHead_m` | ft × 0.3048 |
| `Volume` | `customAttributes.volume_m3` | gal × 0.003785 |
| `HeatDuty` | `customAttributes.heatDuty_W` | BTU/hr × 0.29307 |
| `HeatTransferArea` | `customAttributes.heatTransferArea_m2` | ft² × 0.092903 |
| `GeometricExtents` | `geometry.boundingBox` | in × 25.4 → mm |

### PMEF → Plant 3D (import)

| PMEF field | Plant 3D Property | Unit conversion |
|-----------|------------------|----------------|
| `equipmentBasic.serviceDescription` | `Description` | — |
| `customAttributes.designPressure_Pa` | `DesignPressure` | Pa abs → psig |
| `customAttributes.designTemperature_K` | `DesignTemperature` | K → °F |
| `customAttributes.material` | `Material` | — |

## PCF Line Tag → Line Number Mapping

Plant 3D generates PCF files with `PIPELINE-REFERENCE` set to the line number
tag from the Line List. The Rust adapter uses this to generate the PMEF `@id`:

```
PIPELINE-REFERENCE 8"-CW-201-A1A2
→ urn:pmef:line:eaf-2026:8-CW-201-A1A2
```

## IDF vs PCF

| Feature | PCF | IDF |
|---------|-----|-----|
| Piping routing | ✓ | ✓ |
| Material per component | ✓ | ✓ |
| Weld numbers | ✗ | ✓ |
| Item codes (BOM) | ✗ | ✓ |
| Test data | ✗ | ✓ |
| Spool marks | ✗ | ✓ |

Use IDF for projects requiring weld tracking and spool management.
