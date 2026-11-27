# RFC-0014: Generic duct component and duct bends without angle

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust model, native type catalog, example and tests in this repository |
| **Discussion** | [#70](https://github.com/MatzUp2022/PMEF/issues/70), 30-day period 2026-11-27 → 2026-12-27 |
| **Target version** | 0.10.0 (additive: one object type, one relaxed requirement, one catalog) |
| **Depends on** | spec 02 §11; [RFC-0006](0006-hvac-ducts-tray-fittings-catalogs.md), [RFC-0009](0009-generic-piping-component.md), [RFC-0012](0012-placeholder-custom-values.md) |
| **Working groups** | WG-HVAC, WG-Adapters |
| **Date** | 2026-11-27 |

## Abstract

The Huldra HVAC model (33 duct networks, 1,022 components) has 65 components without a PMEF type
(end caps, splitters, silencers, grilles, offsets, cowls) and 39 bends without a known angle.
This RFC adds `pmef:GenericDuctComponent`, the HVAC counterpart of
`pmef:GenericPipingComponent` (RFC-0009), makes `DuctBend.angle` optional when both port
directions are given, and adds the E3D HVAC native type catalog.

## Motivation

- **UC-1 No component is dropped.** An exporter that meets an unknown HVAC part today has to
  drop it or misclassify it as a duct; both break the network topology and the bill of
  materials.
- **UC-2 Bends from geometry.** Segmented and rectangular bends often come without an angle
  attribute. With both port directions the angle is determined, so requiring it duplicates
  data and forces converters to invent a value.
- **UC-3 Mapping as data.** Element keywords such as `CAP` mean different parts in piping and
  HVAC, so HVAC gets its own catalog.

## Detailed Design

1. **`pmef:GenericDuctComponent`** (`pmef-hvac`): a `DuctComponent` with required
   `componentClass` (`END_CAP`, `SILENCER`, `SPLITTER`, `GRILLE`, `COWL`, `OFFSET`,
   `FLEXIBLE_CONNECTOR`, `SPECIAL`) and `ports` (≥ 1), optional `nativeType`
   (`<tool>:<type>`) and `length`. Adapters **MUST** use it instead of dropping an HVAC part.
2. **`DuctBend.angle`**: required unless the bend has exactly two ports that both carry
   `direction`; then angle = 180° − angle between the outward directions. If both are given
   they **MUST** agree within 1° (`check-geometry.py`). Rust: `angle: Option<f64>` and
   `DuctBend::bend_angle()`.
3. **`catalogs/native-types-e3d-hvac.json`** (`NATIVE_TYPE_MAPPING`): E3D `CAP`, `SPLR`,
   `SILE`, `MESH`, `OFST`, `COWL`, `THRE`, `PCOM` → `GenericDuctComponent`; `BRCO` →
   `DuctBranch`. `check-catalogs.py` checks duct classes against `GenericDuctComponent`.
4. **Conventions** (spec 02 §11.1, §11.5): `airType` derived from system names is a project
   convention; non-air systems modelled as HVAC are `OTHER`; a branch port derived from geometry
   **SHOULD** be listed in `placeholders`.

## Backwards Compatibility

Additive for files. Rust API: `DuctBend.angle` changes from `f64` to `Option<f64>`
(pre-1.0); `PmefEntityType` gains `GenericDuctComponent`.

## Alternatives Considered

- **One subtype per part (`DuctSilencer`, `DuctCap`, …)** — rejected for now: the parts carry no
  data beyond class and length in the source models; dedicated types can follow when they do
  (a silencer with insertion loss data, for instance).
- **Keep `angle` required and let converters write 90°** — rejected: a guessed value is worse
  than a derived one; RFC-0012 placeholders exist for values that cannot be derived.
- **HVAC entries in `native-types-e3d.json`** — rejected: `CAP` would map differently per
  discipline, and the first-match rule has no domain condition.

## Open Questions

1. Should `GenericDuctComponent` get typed blocks for silencers (insertion loss) and grilles
   (free area)? *Proposed:* `customAttributes` until a source supplies them.
   *Decision: WG-HVAC.*

## Reference Implementation

- Schema `pmef-hvac` (`GenericDuctComponent`, `DuctBend` `anyOf`); `pmef-catalog`
  componentClass description.
- Rust `pmef_core::hvac::GenericDuctComponent`, `DuctBend::bend_angle`; type registry,
  validator mapping, schema-drift registry; `pmef-catalogs` embeds the new catalog.
- Example `examples/hvac-generic-components.ndjson`; conformance HVAC-U-014 … HVAC-U-017;
  `check-geometry.py` bend rule (`tests/geometry` cases `DB-OK`, `DB-ANGLE`);
  `check-catalogs.py` duct classes (`tests/catalogs`).
