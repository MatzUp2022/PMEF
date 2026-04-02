# pmef-tekla-dotnet

C# Tekla Open API connector for PMEF.

## Components

| File | Purpose |
|------|---------|
| `src/PmefExporter.cs` | Read Tekla model → write `tekla-export.json` |
| `src/PmefImporter.cs` | Read PMEF NDJSON → create/update Tekla model objects |

## Prerequisites

- Tekla Structures 2024 or newer (tested: 2024.0)
- .NET 8 SDK (Windows)
- Tekla Structures open with a model loaded

## Build

```bat
cd pmef-tekla-dotnet
dotnet build -c Release
```

The build references Tekla assemblies from:
`C:\Program Files\Tekla Structures\2024.0\bin\plugins\`

Adjust `TeklaInstallDir` in `PmefTekla.csproj` if needed.

## Export (Tekla → PMEF)

```bat
# With Tekla open and a model loaded:
PmefTekla.exe tekla-export.json

# Then process with the Rust adapter:
pmef convert tekla-export.json --from tekla --to pmef --output output.ndjson
```

Or from Rust code:

```rust
use pmef_adapter_tekla::{TeklaAdapter, TeklaConfig};

let config = TeklaConfig {
    project_code: "eaf-2026".to_owned(),
    export_path: "tekla-export.json".into(),
    steel_only: true,
    ..Default::default()
};
let mut adapter = TeklaAdapter::new(config);
let stats = adapter.export_to_pmef("output.ndjson").await?;
```

## Import (PMEF → Tekla)

```bat
PmefImporter.exe steel-model.ndjson
```

The importer:
1. Reads all `pmef:HasEquivalentIn` objects with `targetSystem = "TEKLA_STRUCTURES"`
2. Matches existing Tekla objects by GUID
3. Updates matched objects; creates new objects for unmatched ones
4. Writes `PMEF_ID` and analysis UDAs (`PMEF_UTILISATION_RATIO`, `PMEF_CRITICAL_CHECK`)
   for round-trip support

## Round-trip identity

Every exported object gets a `pmef:HasEquivalentIn` relationship:

```jsonc
{
  "@type": "pmef:HasEquivalentIn",
  "sourceId": "urn:pmef:obj:eaf-2026:STR-B101",
  "targetSystem": "TEKLA_STRUCTURES",
  "targetSystemId": "3F7A1B2C-4D5E-6F7A-8B9C-0D1E2F3A4B5C",  // Tekla GUID
  "confidence": 1.0
}
```

Re-importing this PMEF package into Tekla will update the existing beam
(matching by `targetSystemId` = Tekla GUID) rather than creating a duplicate.

## Supported Object Types

| PMEF Type | Tekla Type | Notes |
|-----------|-----------|-------|
| `pmef:SteelMember` (Beam) | `Beam` (BeamTypeEnum.BEAM) | Full attribute mapping |
| `pmef:SteelMember` (Column) | `Beam` (BeamTypeEnum.COLUMN) | |
| `pmef:SteelMember` (Brace) | `Beam` | Diagonal detection via geometry |
| `pmef:SteelMember` (PolyBeam) | `PolyBeam` | Start+end only |
| `pmef:SteelConnection` | `Component` / `BoltGroup` | 10 connection types |
| `pmef:SteelMember` with `assemblyId` | `Assembly` | Via sub-assembly |

## Profile Mapping

Profile IDs are normalised from Tekla's regional notation to PMEF:

| Tekla | PMEF |
|-------|------|
| `HEA200` | `EN:HEA200` |
| `HE200A` | `EN:HEA200` (override table) |
| `SHS150*6` | `EN:SHS150x6` |
| `W12X53` | `AISC:W12x53` |
| `HSS6x4x.25` | `AISC:HSS6x4x0.25` |

## UDAs Written by Importer

| UDA Name | PMEF source | Type |
|----------|-------------|------|
| `PMEF_ID` | `@id` | String |
| `PMEF_UTILISATION_RATIO` | `customAttributes.analysisResults.utilisationRatio` | Double |
| `PMEF_CRITICAL_CHECK` | `customAttributes.analysisResults.criticalCheck` | String |

## UDAs Read by Exporter

| UDA Name | PMEF target |
|----------|-------------|
| `FIRE_PROTECTION_TYPE` | `fireProtection.protectionType` |
| `FIRE_RESISTANCE_PERIOD` | `fireProtection.requiredPeriodMin` |
| `INTUMESCENT_THICKNESS` | `fireProtection.thicknessMm` |
| `PMEF_UTILISATION_RATIO` | `analysis.utilisationRatio` |
| `PMEF_CRITICAL_CHECK` | `analysis.criticalCheck` |
| `PMEF_AXIAL_KN` | `analysis.axialForceKn` |
| `PMEF_MAJOR_BEND_KNM` | `analysis.majorBendingKnm` |
| `SURFACE_TREATMENT` | `finish` |
| `CIS2_MEMBER_ID` | `cis2Ref` |
| `ERECTION_SEQUENCE` | `customAttributes.udas` |
| `SHOP_MARK` | `customAttributes.udas` |
| `FIRE_ZONE` | `customAttributes.udas` |
| `INSPECTION_CLASS` | `customAttributes.udas` |
