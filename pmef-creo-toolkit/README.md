# pmef-creo-toolkit

PTC Creo Parametric Toolkit plugin for PMEF export.

## Components

| File | Purpose |
|------|---------|
| `src/CreoExporter.c` | Creo Toolkit C plugin — reads model → writes `creo-export.json` |

## Prerequisites

- PTC Creo Parametric 10.0 or newer
- PTC Creo Toolkit SDK (included with Creo installation)
- Visual Studio 2022 (Windows) or GCC 13 (Linux)

## Build (Windows)

```bat
set CREO_HOME=C:\PTC\Creo 10.0.0.0\Parametric
set PROTK_INCLUDE=%CREO_HOME%\bin\protoolkit\includes
set PROTK_LIB=%CREO_HOME%\bin\protoolkit\i486_nt\obj

cl /c /W3 /DPRO_USE_VAR_ARGS /I%PROTK_INCLUDE% src\CreoExporter.c
link /DLL /OUT:CreoExporter.dll CreoExporter.obj %PROTK_LIB%\protoolkit.lib
```

## Deployment

1. Copy `CreoExporter.dll` to `%CREO_HOME%\text\usascii\protk_dll\`
2. Create or edit `%CREO_HOME%\text\protk.dat`:
   ```
   NAME pmef_exporter
   EXEC_FILE <path>\CreoExporter.dll
   TEXT_DIR <path>\pmef-creo-toolkit\text
   END
   ```
3. Restart Creo Parametric

## Usage

1. Open the assembly to export in Creo
2. Menu: **PMEF → Export**
3. Enter the output path (e.g. `creo-export.json`)
4. Process with the Rust adapter:

```bash
pmef convert creo-export.json --from creo --to pmef \
    --step-dir ./step-exports/ \
    --output output.ndjson
```

## Required Creo Parameters

Set these user parameters on assembly models for best PMEF mapping:

| Parameter | Type | Example | PMEF field |
|-----------|------|---------|-----------|
| `PLANT_TAG` | String | `P-201A` | `equipmentBasic.tagNumber` |
| `EQUIPMENT_CLASS` | String | `PUMP` | `equipmentBasic.equipmentClass` |
| `DESIGN_CODE` | String | `API 610` | `equipmentBasic.designCode` |
| `MATERIAL` | String | `Carbon Steel` | `customAttributes.material` |
| `DESIGN_PRESSURE` | Real | `15.0` (bar g) | `customAttributes.designPressure_Pa` |
| `DESIGN_TEMPERATURE` | Real | `60.0` (°C) | `customAttributes.designTemperature_K` |
| `WEIGHT` | Real | `1850.0` (kg) | `customAttributes.weightKg` |
| `DESCRIPTION` | String | `Cooling water pump` | `equipmentBasic.serviceDescription` |

## Nozzle Modelling Convention

Model nozzle connection points as Creo coordinate systems (CSYS) with the naming convention:

```
CS_NOZZLE_N1      → PMEF nozzle mark "N1"
CS_NOZZLE_SUCTION → PMEF nozzle mark "SUCTION"
CS_NOZZLE_N2_DISCHARGE → PMEF nozzle mark "N2_DISCHARGE"
```

The Z-axis of the coordinate system must point **outward** from the equipment
(in the flow direction at the nozzle face). This Z-axis becomes the PMEF
nozzle `direction` vector.

Optional nozzle parameters (set on the part containing the CS):

| Parameter | Example | PMEF field |
|-----------|---------|-----------|
| `NZ_DIAMETER` | `8.0` (inches) | `nominalDiameter` (×25.4 → mm) |
| `NZ_SERVICE` | `Suction` | `service` |
| `NZ_RATING` | `150` | `flangeRating` (`ANSI-150`) |
| `NZ_FACING` | `RF` | `facingType` |

## STEP Export for LOD3 Geometry

For full B-Rep geometry (LOD3), export STEP files alongside the JSON:

1. In Creo: File → Save a Copy → STEP AP214 → Export each assembly
2. Name each file `<assembly_name>.stp` (same as `modelName` in JSON)
3. Place in a `step-exports/` directory
4. Pass `--step-dir step-exports/` to the Rust adapter

The Rust adapter will:
- Use the STEP bounding box (more accurate than Creo JSON bbox)
- Extract nozzle CS positions from STEP `AXIS2_PLACEMENT_3D` entities
- Set `geometry.lod = "LOD3_FINE"` and `geometry.refUri` → Windchill URL

## SMS Group Equipment Classes

The adapter supports SMS Group specific equipment classes in addition to
standard process plant classes:

| `EQUIPMENT_CLASS` | PMEF result |
|-------------------|------------|
| `ROLLING_MILL` | `pmef:GenericEquipment` / `ROLLING_MILL` |
| `MILL_FRAME` | `pmef:GenericEquipment` / `ROLLING_MILL` |
| `EAF` | `pmef:Reactor` / `ELECTRIC_ARC_FURNACE` |
| `CONVERTER` | `pmef:Reactor` / `CONVERTER` |
| `LADLE` | `pmef:Reactor` / `LADLE` |
| `GEARBOX` | `pmef:GenericEquipment` / `GEARBOX` |
| `HYDRAULIC_UNIT` / `HPU` | `pmef:GenericEquipment` / `HYDRAULIC_UNIT` |

## Windchill Integration

If Windchill is connected, the exporter automatically reads the WTPart number
for each assembly and includes it in the export. This enables:

- Bidirectional link from PMEF objects to Windchill documents
- Version tracking via `windchillNumber` in `HasEquivalentIn`
- Direct hyperlink to Windchill document viewer (if `windchill_url` configured)
