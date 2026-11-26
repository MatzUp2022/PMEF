# RFC-0006: HVAC duct domain, cable tray fittings, support and cable catalogs

| Field | Value |
|---|---|
| **Status** | `DRAFT` — reference implementation, schemas, example and tests in this repository |
| **Discussion** | issue opened with the implementing pull request, 30-day period |
| **Target version** | 0.10.0 (additive: new types, new optional fields, new catalog types) |
| **Depends on** | spec 02 §2.5 (`CatalogReference`), §2.6 (`Port`), §4 (piping pattern), §6 (E&I) |
| **Working groups** | WG-Information-Model, WG-Catalogs |
| **Date** | 2026-11-26 |

## Abstract

PMEF models piping, equipment, steel and E&I, but has no data model for air ducts, no cable
tray fittings, and no catalogs for pipe supports or cable types. These are the gaps the
ecosystem capability map of 2026-11-26 lists first, because the design tool, isometric
derivation, auto-routing and the engineering platforms all need the objects before they can
use them. This RFC adds a new HVAC domain (`pmef-hvac.schema.json`), a `CableTrayFitting` type,
a `catalogRef` on `CableObject`, and three catalog types (`DUCT_DIMENSIONS`,
`PIPE_SUPPORT_TYPES`, `CABLE_TYPES`) with one open catalog each.

## Motivation

- **HVAC is a design discipline of its own.** Ducts have cross-sections that change shape
  (round → square), connections that are flanged, welded or slip-jointed, insulation per
  component, and inline items (fire dampers, access doors, test ports) that piping does not
  have. Mapping them onto piping types would lose this information. The only existing code
  (`mvcache-core/mechanical/duct.rs` in PlantView, frozen) is a sizing register without
  geometry or topology.
- **Exactness from catalogs.** Duct sizes (EN 1505/1506), support assemblies and cable types
  are catalog data. PMEF is the single home of engineering catalogs (`docs/ECOSYSTEM.md`), so
  they belong in `catalogs/`, not in the consuming repositories.
- **Routing needs them.** `pmef-route` routes cables over trays using cable diameter and bend
  radius; the cable catalog provides those values, and tray fittings make a routed tray network
  representable.

## Detailed Design

Normative text: spec 02 §6.6–§6.7 and §11; schemas `pmef-hvac.schema.json`,
`pmef-ei.schema.json`, `pmef-catalog.schema.json`.

1. **HVAC domain** — new schema module, same three-level pattern as piping:
   `pmef:DuctNetworkSystem` (air system, `airType` per EN 16798-3, design conditions) →
   `pmef:DuctSegment` → components `pmef:Duct`, `pmef:DuctBend`, `pmef:DuctTransition`,
   `pmef:DuctBranch`, `pmef:DuctDamper`, `pmef:DuctAccessDoor`, `pmef:DuctTestPort`.
2. **Cross-section per port.** `DuctCrossSection {shape, diameter, width, height}` with
   `shape` ∈ ROUND, SQUARE, RECTANGULAR, FLAT_OVAL, and conditional required dimensions. A
   transition round → square is a `DuctTransition` whose two ports carry a ROUND and a SQUARE
   section; no separate "shape transition" type is needed.
3. **Connections on the port.** `DuctPort.connectionType` ∈ FLANGED, WELDED, SLIP_JOINT,
   SOCKET_SPIGOT, CLAMPED, PLAIN, plus `flangeProfile`. `connectedTo` follows spec 02 §4.5.
4. **Insulation per component.** `DuctInsulation {insulationType, thickness, material,
   internal, cladding, fireRating}` on segments and components.
5. **Mounted items.** Access doors and test ports have no flow ports; they reference their
   host component (`hostId`) and a `position` (+ `normal`).
6. **`pmef:CableTrayFitting`** (E&I) — bends (horizontal, vertical up/down), tee, cross,
   reducer, offset, riser, end piece; `bendRadius` limits the cable bend radius;
   `connectedTrayIds` names the joined runs.
7. **`CableObject.catalogRef`** — optional `CatalogReference` to a `CABLE_TYPES` entry.
8. **Catalog types** — `DUCT_DIMENSIONS` (`duct-dimensions-en`), `PIPE_SUPPORT_TYPES`
   (`pipe-supports-generic`; `PipeSupport.catalogRef` references `supportCode`),
   `CABLE_TYPES` (`cable-types-en`). Consistency rules in `scripts/check-catalogs.py`.

Out of scope (follow-up RFCs): air terminals and air handling units (equipment domain),
duct hangers, cable tray supports, terminal plans, single-line diagrams.

## Backwards Compatibility

Additive only. Readers that do not know the new `@type`s must preserve them (spec 02 §1.4).
No existing field changes meaning; existing examples and fixtures are unchanged.

## Reference Implementation

- Schemas: `schemas/pmef-hvac.schema.json` (new), `pmef-ei.schema.json`,
  `pmef-catalog.schema.json`.
- Rust: `pmef-core::hvac` (types + cross-section area, hydraulic diameter, velocity),
  `pmef-core::ei::CableTrayFitting`, `pmef-validate` type map, `pmef-catalogs`
  (`duct_sizes`, `next_duct_size`, `supports_for`, `cable_type`).
- Example: `examples/utilities-hvac-cable.ndjson` (synthetic, CC0).
- Tests: conformance `HVAC-U-001…013`, `EI-U-009/010`; schema drift test covers all new types;
  `tests/catalogs/` broken fixtures for the new catalog rules.
