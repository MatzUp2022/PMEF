# RFC-0020: Optional equipment property sets and placeholders on equipment

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust model, adapter, example and tests in this repository |
| **Discussion** | [#104](https://github.com/MatzUp2022/PMEF/issues/104), 30-day period 2026-11-28 → 2026-12-28 |
| **Target version** | 0.10.0 (relaxation: four `required` removed; tightening: empty sets rejected) |
| **Depends on** | spec 02 §2.10, §5.3; RFC-0012 (placeholders) |
| **Working groups** | WG-Equipment, WG-Adapters |
| **Date** | 2026-11-28 |

## Abstract

`pmef:Pump` requires `pumpSpec`, `Vessel`/`Column`/`Reactor` require `vesselDesign`,
`HeatExchanger` requires `hxSpec`, `Compressor` requires `compressorSpec`. Exporters without
process data (3D models, recognition) satisfy the schema with empty objects — the Plant 3D demo
"Bereich 51" produced eight of them (issue [#92](https://github.com/MatzUp2022/PMEF/issues/92)).
An empty set is formally valid and semantically void: a consumer cannot tell "unknown" from
"not applicable". This RFC makes the four sets optional, rejects empty sets, and defines
`placeholders` on all equipment objects (answers RFC-0012 open question 1 for equipment).

## Motivation

- **One spelling for "unknown".** Absent means no data known. `{}` is no longer valid.
- **Recognition and 3D sources rarely have design data.** Requiring a set forces filler.
- **Partial data must be markable.** A design flow from a preliminary datasheet is written and
  listed as `TO_BE_CONFIRMED`; the confirmed value arrives as a new revision of the same `@id`.

## Detailed Design

Normative text: spec 02 §5.3 (*Key property sets are optional*) and §2.10; schema
`pmef-equipment.schema.json`.

1. `pumpSpec`, `vesselDesign`, `hxSpec`, `compressorSpec`: no longer `required`; when present,
   `minProperties: 1`.
2. `EquipmentObject.placeholders` (optional, ≥ 1 entry, `pmef-base` `Placeholder`).
3. Consumers **MUST NOT** read an absent set as "not applicable".
4. Rust `pmef_core::equipment`: the four fields are `Option<…>` (skipped when `None`); all
   equipment structs carry `placeholders`.
5. Plant 3D adapter (`pmef-adapter-plant3d`): omits the set instead of writing `{}`.

## Compatibility

Data that contained a filled set stays valid. Data with an empty set (`{}`) becomes invalid;
fix: drop the key. Only prototype converters are known to write empty sets. Data without the
set was invalid in 0.9.0 and is valid in 0.10.0.

## Alternatives

- *Keep required, require placeholders for every unknown field.* Rejected: forces dozens of
  entries per pump without information gain.
- *Allow `{}` with a SHOULD NOT.* Rejected: keeps two spellings for "unknown".

## Open questions

1. Should `pmef check` report equipment without any key property set (information level)?
2. Should P&ID/datasheet enrichment via `isDerivedFrom` be specified as a merge rule?
