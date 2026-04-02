# PMEF Open Catalog Library

This directory contains the open PMEF reference catalogs.
All catalogs are licensed under **CC0 1.0 Universal** (public domain).

## Catalog Index

| File | Content | Standard | Entries |
|------|---------|----------|---------|
| `profiles-en.json` | European structural steel profiles | EN 10034, EN 10279, EN 10210, EN 10219 | 312 |
| `profiles-aisc.json` | AISC structural steel sections (US) | AISC Steel Construction Manual 16th ed. | 287 |
| `piping-class-a1a2.json` | Piping class A1A2 (CS, ANSI-150, DN15–DN600) | ASME B16.5, B16.9, B16.11 | 284 components |
| `piping-class-b3c1.json` | Piping class B3C1 (CS, ANSI-300, DN15–DN400) | ASME B16.5, B16.9 | 198 components |
| `pipe-dimensions.json` | Pipe OD and wall thickness (ASME B36.10M + B36.19M) | ASME B36.10M, B36.19M | 240 entries |
| `flange-dimensions.json` | Flange face dimensions per ASME B16.5 | ASME B16.5 | 168 entries |
| `rdl-uri-map.json` | PCA-RDL URI cross-reference for catalog entries | ISO 15926-4 | — |

## Catalog Schema

Each catalog file follows a common JSON structure:

```jsonc
{
  "$schema": "https://pmef.org/schemas/0.9/pmef-catalog.schema.json",
  "catalogId": "<id>",
  "catalogType": "STEEL_PROFILES | PIPING_CLASS | PIPE_DIMENSIONS | FLANGE_DIMENSIONS",
  "standard": "<applicable standard>",
  "version": "<version string>",
  "description": "<human-readable description>",
  "units": { "<quantity>": "<unit>" },
  "entries": [ { ... } ]
}
```

## Profile ID Convention

Steel profile IDs follow the format `<standard>:<designation>`:

```
EN:HEA200       — European wide-flange beam, h=200mm
EN:IPE300       — European I-beam, h=300mm
EN:RHS200x100x6 — Rectangular hollow section
EN:CHS219.1x8   — Circular hollow section
AISC:W12x53     — Wide-flange (US), 12in, 53 lb/ft
AISC:HSS6x4x0.25 — Rectangular hollow section (US)
```

## Piping Class Structure

Each piping class entry specifies for every component:
- `componentClass` — PMEF componentClass enum value
- `skey` — 8-character PMEF SKEY
- `nominalDiameter` — DN [mm]
- `schedule` / `rating` — pressure class
- `material` — material designation
- `standard` — applicable dimensional standard
- `weight_kg` — component weight
- `catalogId` — unique ID within the class
- `rdlUri` — ISO 15926-4 PCA-RDL URI (where available)
- `vendorMappings` — tool-specific catalog IDs
