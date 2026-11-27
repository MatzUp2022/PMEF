# RFC-0013: Steel plates, project profiles, nested steel systems and reserved volumes

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust model, E3D adapter mapping, example and tests in this repository |
| **Discussion** | [#54](https://github.com/MatzUp2022/PMEF/issues/54) (finding from the Huldra test case) |
| **Target version** | 0.10.0 (additive: one object type, optional fields, one catalog field, catalog entries) |
| **Depends on** | spec 02 §5.3, §7; spec 05 §7.2, §8.1; `pmef-steel`, `pmef-catalog` schemas |
| **Working groups** | WG-Steel, WG-Catalogs, WG-Adapters |
| **Date** | 2026-11-27 |

## Abstract

The Huldra test case (24,091 members, 16,854 E3D panels) showed four gaps in the steel domain:
plates and gratings have no type, most members use project profiles that no shared catalog
defines, E3D's structure hierarchy (STRU → FRMW → SBFR) cannot be expressed, and reserved
volumes (obstruction, access routes) end up as anonymous equipment. This RFC adds
`pmef:SteelPlate`, project profile catalogs, `SteelSystem.systemLevel` with nesting, and two
reserved `equipmentClass` values.

## Motivation

- **UC-1 Plates.** Decks, gratings, chequered plates and wall panels are a large share of a
  platform's steel. Without a type they are lost or carried as meshes without semantics, so
  material take-off and clash checks against walkways miss them.
- **UC-2 Project profiles.** 19,797 of 24,091 Huldra members use profiles that no shared catalog
  lists (bulb flats, welded tees, flat bars, round bars, rails). The profile ID must stay
  resolvable, so bounds, weights and drawings work.
- **UC-3 Hierarchy.** E3D groups members in STRU → FRMW → SBFR. A flat `SteelSystem` loses the
  frameworks that drawings and work packages are built on.
- **UC-4 Reserved volumes.** Obstruction volumes and access routes are clash-relevant but not
  process equipment.

## Detailed Design

### 1. `pmef:SteelPlate` (spec 02 §7.4)

Planar outline in a local plane plus a thickness: `origin`, `normal` (local z), `refDirection`
(local x, perpendicular to `normal`), `outline` (`[x, y]` points, ≥ 3, closed implicitly),
optional `holes`, `thickness` (> 0, along +`normal`), `plateType` (`PLATE`, `GRATING`,
`CHEQUERED`, `PANEL`, `GENERIC`), plus the usual steel fields (`material`, `weight`, `finish`,
`geometry`, provenance blocks, `customAttributes`). `SteelSystem.plateIds` lists plates like
`memberIds` lists members. IFC 4.3: `IfcPlate` with an `IfcArbitraryProfileDefWithVoids`
extruded by the thickness.

### 2. Project profiles (spec 02 §7.1, spec 05 §7.2)

- A profile outside the shared catalogs is `<PROJECT>:<designation>`.
- Catalogs gain `scope` (`SHARED` default, `PROJECT`). A package **SHOULD** ship a `PROJECT`
  `STEEL_PROFILES` catalog for its project profiles (PMEFX `catalogs/`; next to an NDJSON file
  in `catalogs/`). Readers resolve the package's project catalogs first.
- Adapters write `<PROJECT>:<native>` and `customAttributes.nativeProfileName` instead of
  `CUSTOM:<native>`; readers still accept `CUSTOM:`.
- Shared catalog: round bars `EN:RB10` … `EN:RB60` (EN 10060) in `profiles-en` 2024.4; flat
  bars already exist as `EN:FLAT<b>x<t>`.

### 3. Nested steel systems (spec 02 §7.5)

`SteelSystem.isPartOf` may reference another `SteelSystem`; optional `systemLevel`
(`STRUCTURE`, `FRAMEWORK`, `SUBFRAMEWORK`). No new object type.

### 4. Reserved volumes (spec 02 §5.3)

`pmef:GenericEquipment` with `equipmentBasic.equipmentClass` `OBSTRUCTION_VOLUME` or
`ACCESS_ROUTE`; clash checks treat the first as solid and the second as a soft clash.

### 5. E3D adapter (spec 05 §8.1)

STRU/FRMW/SBFR → nested `SteelSystem`; SCTN `POSS`/`POSE`/`BANG`/`SPRE`/`GTYP` →
`SteelMember` (`rollAngle` from `BANG`); PANE with PLOO (`HEIG` = thickness) and PAVE vertices
→ `SteelPlate` (plane by Newell's method, refDirection along the first edge). This also closes
finding 4 of #54 (member axes): with the PML export, start, end and roll come from the model,
not from facet geometry.

## Backwards Compatibility

Additive within 0.10.0. Existing files stay valid. `profiles-en` changes version (2024.3 →
2024.4) because entries were added; catalog snaps that pin 2024.3 stay valid for the entries
they reference.

## Alternatives Considered

- **Plates as `SteelMember` with a flat-bar profile** — rejected: a plate has a 2D outline with
  openings, not a profile swept along an axis.
- **Plates as `ParametricGeometry` only** — rejected: loses material, thickness and plate type,
  which take-off and drawings need.
- **A new `SteelFramework` type** — rejected: the levels differ only in grouping; one type with
  `systemLevel` keeps queries simple.
- **Bulb flats (HP, EN 10067) in the shared catalog** — deferred: needs a verified dimension
  table; until then they are project profiles.

## Open Questions

1. Should `SteelPlate` carry a `profileId`-like reference to grating catalogs (bar size,
   mesh)? *Proposed:* later, via `customAttributes` until a grating catalog exists.
   *Decision: WG-Steel.*
2. Nozzle direction (finding 5 of #54, local X in Huldra) is an adapter matter: the E3D adapter
   reads `HDIR`; facet-based converters should derive it from the nozzle geometry.
   *Decision: WG-Adapters.*

## Reference Implementation

- Schemas: `pmef-steel` (`SteelPlate`, `Point2D`, `SteelSystem.systemLevel`/`plateIds`),
  `pmef-catalog` (`scope`).
- Rust: `pmef_core::steel::SteelPlate` (area and volume), `SteelSystem.system_level` /
  `plate_ids`; `pmef-adapter-e3d` module `steel` with `tests/steel.rs`.
- Checks: `scripts/check-geometry.py` (plate rules, project catalogs next to the file), negative
  cases `PL-*` in `tests/geometry/`.
- Catalogs: `profiles-en` 2024.4 (round bars), `examples/catalogs/profiles-demo-project.json`.
- Example: `examples/steel-platform-plates.ndjson`.
