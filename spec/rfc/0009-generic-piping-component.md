# RFC-0009: Generic piping component and native type catalog

| Field | Value |
|---|---|
| **Status** | `DRAFT` — reference implementation, schema, catalog, example and tests in this repository |
| **Discussion** | issue opened with the implementing pull request, 30-day period |
| **Target version** | 0.10.0 (additive: one new `@type`, four `componentClass` values, one `catalogType`) |
| **Depends on** | spec 02 §4.3–4.4, spec 05 §AVEVA E3D, `pmef-catalog.schema.json` |
| **Working groups** | WG-Piping, WG-Adapters |
| **Date** | 2026-11-27 |

## Abstract

Authoring tools have more piping element types than PMEF has subtypes (couplings, unions,
in-line instrument bodies, closures, general components). The E3D adapter mapped every
unknown keyword to `pmef:Pipe` and every valve without a known SKEY prefix to `VALVE_GATE`,
so a test with an offshore E3D line lost or mislabelled components. This RFC adds
`pmef:GenericPipingComponent`, four component classes, and a catalog type that holds the
tool-specific type mapping as data.

## Motivation

- **No component may be dropped or relabelled.** A coupling exported as a pipe changes
  lengths, weights and the bill of materials. A valve of unknown type exported as a gate valve
  is wrong in every downstream document.
- **Mappings are data.** Each project and tool version adds element types. Changing a
  catalog file is cheaper and reviewable; it also lets other adapters share the mechanism.

## Detailed Design

Normative text: spec 02 §4.4; schemas `pmef-piping-component.schema.json`,
`pmef-property-sets.schema.json`, `pmef-catalog.schema.json`.

1. `pmef:GenericPipingComponent` is a `PipingComponent` with the optional field
   `nativeType` = `<tool>:<element type>`, e.g. `E3D:PCOM`. It carries ports and
   `componentSpec` like any other subtype. Consumers that do not know the class treat it as
   an in-line component between its ports.
2. New `componentClass` values: `VALVE_UNSPECIFIED`, `INLINE_INSTRUMENT`, `CLOSURE`.
   `COUPLING` existed already.
3. New `catalogType` `NATIVE_TYPE_MAPPING` with `sourceSystem` and entries
   `{catalogId, nativeType, skeyPrefix?, skeyContains?, pmefType, componentClass?, note?}`.
   The first entry whose conditions hold wins, so SKEY-specific entries precede the generic
   entry of the same element type. `check-catalogs.py` reports unknown types and classes,
   duplicate ids and entries that an earlier unconditional entry hides.
4. `catalogs/native-types-e3d.json` maps the E3D/PDMS element types. An element type that is
   not listed becomes `pmef:GenericPipingComponent` / `SPECIAL` with a warning.

## Compatibility

Additive. The E3D adapter output changes for valves without a known SKEY prefix
(`VALVE_GATE` → `VALVE_UNSPECIFIED`) and for unknown element types (`pmef:Pipe` →
`pmef:GenericPipingComponent`); both were wrong before.

## Reference Implementation

- `pmef-core::GenericPipingComponent` (serde round trip via schema drift test).
- `pmef-catalogs::{NativeTypeMapping, native_types, native_type}`.
- `pmef-adapter-e3d`: `e3d_element_to_pmef` reads the catalog; the PML parser accepts every
  catalog keyword as a component; generic components carry `nativeType`.
- Example `heat-exchanger-station.ndjson` (flow element `CW-301-FE-001`); conformance tests
  `PIPE-U-017/018`; broken catalog `tests/catalogs/broken/native-types-broken.json`.

## Open Questions

- Should other adapters (Plant 3D, PCF) move their type tables into the same catalog type?
- Should `nativeType` also be allowed on the specific subtypes, to keep the source keyword?
