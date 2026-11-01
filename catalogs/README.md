# PMEF Open Catalog Library

This directory contains the open PMEF reference catalogs.
All catalogs are licensed under **CC0 1.0 Universal** (public domain).

## Catalog Index

| File | Content | Standard | Entries |
|------|---------|----------|---------|
| `profiles-en.json` | European structural steel profiles (HEA, HEB, IPE, UPE, RHS, SHS, CHS, angles, flats) | EN 10034, EN 10279, EN 10210, EN 10219 | 202 |
| `profiles-en-extended.json` | European T-sections and unequal angles | EN 10055, EN 10056-2 | 45 |
| `profiles-aisc.json` | AISC structural steel sections (W, HSS, L) | AISC Steel Construction Manual 16th ed. | 113 |
| `profiles-aisc-extended.json` | AISC channels (C, MC) and structural tees (WT) | AISC Steel Construction Manual 16th ed. | 51 |
| `piping-class-a1a2.json` | Piping class A1A2 (CS, ANSI-150, DN15–DN600) | ASME B16.5, B16.9, B16.11 | 203 |
| `piping-class-b3c1.json` | Piping class B3C1 (CS, ANSI-300, DN15–DN400) | ASME B16.5, B16.9 | 143 |
| `piping-class-c5d1.json` | Piping class C5D1 (316L SS, ANSI-150, DN15–DN300) | ASME B16.5, B16.9, B36.19M | 48 |
| `piping-class-en-p1a.json` | Piping class EN-P1A (CS, PN40, DN15–DN300) | EN 1092-1, EN 10253-2, EN 10216-2 | 61 |
| `pipe-dimensions.json` | Pipe OD and wall thickness (ASME) | ASME B36.10M, B36.19M | 31 |
| `pipe-dimensions-en.json` | Pipe OD and wall thickness (EN) | EN 10220 | 19 DN sizes |
| `flange-dimensions.json` | Flange face dimensions (ASME) | ASME B16.5 | 87 |
| `flange-dimensions-en.json` | Flange face dimensions (EN, PN10–PN40) | EN 1092-1 | 76 |
| `materials-en.json` | European steel material grades (structural + piping) | EN 10025-2, EN 10028, EN 10216, EN 10217, EN 10222 | 33 |
| `materials-astm.json` | US/ASTM steel material grades (structural, piping, bolting) | ASTM A36–A572, A106, A312, A182, A193 | 27 |
| `instruments-common.json` | Common field instruments (T, P, F, L transmitters, valves) | IEC 60770, IEC 60534, API 526 | 30 |
| `rdl-uri-map.json` | PCA-RDL URI cross-reference for catalog entries | ISO 15926-4 | — |
| `caesarII-cii-mapping.json` | PMEF ↔ CAESAR II field mapping | — | — |

## Catalog Schema

Each catalog file follows a common JSON structure:

```jsonc
{
  "$schema": "https://pmef.net/schemas/0.9/pmef-catalog.schema.json",
  "catalogId": "<id>",
  "catalogType": "STEEL_PROFILES | PIPING_CLASS | PIPE_DIMENSIONS | FLANGE_DIMENSIONS | MATERIAL_GRADES | INSTRUMENT_CATALOG | RDL_MAPPING | PIPE_STRESS_MAPPING",
  "standard": "<applicable standard>",
  "version": "<version string>",
  "description": "<human-readable description>",
  "units": { "<quantity>": "<unit>" },
  "entries": [ { ... } ]
}
```

All catalogs are validated in CI against
[`schemas/pmef-catalog.schema.json`](../schemas/pmef-catalog.schema.json).
Beyond the common envelope above, the schema refines the required fields of
each `entries` item based on `catalogType` — for example a `PIPING_CLASS`
entry must carry `catalogId`, `componentClass`, `skey`, and `standard`, while
an `RDL_MAPPING` entry must carry `pmefType`, `rdlUri`, and `label`. Run the
validator locally with:

```sh
python scripts/validate-catalogs.py
```

## Profile ID Convention

Steel profile IDs follow the format `<standard>:<designation>`:

```text
EN:HEA200       — European wide-flange beam, h=190mm
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

---

## Disclaimer

Dimensional data in these catalogs consists of factual values derived from the
standards listed above, reformatted for machine-readable interoperability.
These files are not a substitute for the official published standards. Users
should verify against current editions for engineering and construction
purposes. See [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md) for full
attribution and trademark notices.
