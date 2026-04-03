# pmef-inventor-addin

Autodesk Inventor COM Add-in / iLogic Rule for PMEF export.

## Components

| File | Purpose |
|------|---------|
| `src/InventorExporter.cs` | Reads Inventor model → writes `inventor-export.json` |

## Prerequisites

- Autodesk Inventor 2024 or newer
- .NET 8 SDK (Windows, x64)
- Autodesk.Inventor.Interop.dll (installed with Inventor)

## Build

```bat
cd pmef-inventor-addin
dotnet build -c Release
```

## Usage — as iLogic External Rule

1. Copy `PmefInventor.dll` to a folder accessible from Inventor
2. Tools → iLogic → iLogic Configuration → External Rule Directories → add folder
3. Open an assembly in Inventor
4. Tools → iLogic → External Rules → select `PmefInventor.dll` → Run
5. Choose output path in the file dialog

## Usage — process with Rust

```bash
pmef convert inventor-export.json --from inventor \
    --to pmef --output output.ndjson

# With STEP files for LOD3 geometry:
pmef convert inventor-export.json --from inventor \
    --step-dir ./step-exports/ \
    --to pmef --output output.ndjson
```

## PMEF Parameter Convention

Set these **model parameters** in Inventor assembly documents:

| Parameter | Type | Example | PMEF field |
|-----------|------|---------|-----------|
| `PMEF_TAG` | Text | `P-201A` | `equipmentBasic.tagNumber` |
| `PMEF_CLASS` | Text | `PUMP` | `equipmentBasic.equipmentClass` |
| `PMEF_DESIGN_PRESSURE` | Real [bar g] | `15.0` | `customAttributes.designPressure_Pa` |
| `PMEF_DESIGN_TEMP` | Real [°C] | `60.0` | `customAttributes.designTemperature_K` |
| `PMEF_DESIGN_CODE` | Text | `API 610` | `equipmentBasic.designCode` |

Set parameters: Parameters dialog (Manage → Parameters) or iLogic rule.

## Nozzle Work Point Convention

Create Inventor Work Points named `PMEF_NOZZLE_<mark>`:

```
PMEF_NOZZLE_N1       → nozzle mark "N1"
PMEF_NOZZLE_SUCTION  → nozzle mark "SUCTION"
PMEF_NOZZLE_INLET    → nozzle mark "INLET"
```

Optional per-nozzle parameters (prefix `PMEF_NOZZLE_<mark>_`):

| Parameter | Example | PMEF field |
|-----------|---------|-----------|
| `NZ_DN` | `203.2` | `nominalDiameter` [mm] |
| `NZ_RATING` | `150` | `flangeRating` → `ANSI-150` |
| `NZ_FACING` | `RF` | `facingType` |
| `NZ_SERVICE` | `Suction` | `service` |

Or set globally (applies to all nozzles):
`NZ_DN`, `NZ_RATING`, `NZ_FACING`, `NZ_SERVICE`

## Equipment Class Values (`PMEF_CLASS`)

| Value | PMEF type | Class |
|-------|-----------|-------|
| `PUMP` | `pmef:Pump` | `CENTRIFUGAL_PUMP` |
| `RECIPROCATING_PUMP` | `pmef:Pump` | `RECIPROCATING_PUMP` |
| `COMPRESSOR` | `pmef:Compressor` | `CENTRIFUGAL_COMPRESSOR` |
| `HEAT_EXCHANGER` | `pmef:HeatExchanger` | `SHELL_AND_TUBE_HEAT_EXCHANGER` |
| `VESSEL` | `pmef:Vessel` | `PRESSURE_VESSEL` |
| `TANK` | `pmef:Tank` | `STORAGE_TANK` |
| `EAF` | `pmef:Reactor` | `ELECTRIC_ARC_FURNACE` |
| `CONVERTER` | `pmef:Reactor` | `CONVERTER` |
| `LADLE` | `pmef:Reactor` | `LADLE` |
| `ROLLING_MILL` | `pmef:GenericEquipment` | `ROLLING_MILL` |
| `GEARBOX` | `pmef:GenericEquipment` | `GEARBOX` |
| `HPU` | `pmef:GenericEquipment` | `HYDRAULIC_UNIT` |

## Frame Generator Support

Frame Generator members are automatically detected and exported as
`pmef:SteelMember` objects with:

- `profileId`: `EN:HEA200`, `AISC:W12x53`, etc. (normalised from section name)
- `memberType`: `BEAM`, `COLUMN`, `BRACE`
- `material.fy`, `material.fu`: from steel grade
- Start/end points in world coordinates [mm]

## Tube & Pipe Support

Tube & Pipe runs are exported as `pmef:PipingNetworkSystem` + `pmef:Pipe`.
Units are converted: Inventor stores pipe dimensions in inches internally,
exported to mm in the JSON.

## iProperties Mapping

| iProperty | PMEF field |
|-----------|-----------|
| `Part Number` | `customAttributes.partNumber` |
| `Description` | `equipmentBasic.serviceDescription` |
| `Revision Number` | `revision.revisionId` |
| `Designer` | `customAttributes.designer` |
| `Material` (Physical) | `customAttributes.material` |
| `Mass` (Physical) [kg] | `customAttributes.massKg` |
| `Vendor` | `equipmentBasic.manufacturer` |

## Vault Integration

If Vault PDM is connected, the add-in reads the Vault document number
from the `Number` iProperty and includes it in:
- `vaultNumber` field of each assembly/part
- `HasEquivalentIn.vaultNumber` relationship attribute
