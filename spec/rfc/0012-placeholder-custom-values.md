# RFC-0012: Placeholder and custom values (stand-ins replaced in a later revision)

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust model, `pmef check` report, example and tests in this repository |
| **Discussion** | issue opened with the implementing pull request, 30-day period |
| **Target version** | 0.10.0 (additive: one base type, one optional array, two enum values, one field) |
| **Depends on** | spec 02 §2.10 and §11.5, `pmef-base.schema.json`, `pmef-hvac.schema.json` |
| **Working groups** | WG-HVAC, WG-Adapters |
| **Date** | 2026-11-27 |

## Abstract

`DuctDamper.damperType` is required. The Huldra E3D model (36 dampers) carries damper tags
only, so an exporter must either drop the dampers or guess the type. This RFC adds a stand-in
value `UNSPECIFIED`, a project value `CUSTOM` with `damperTypeCustom`, and a general
`placeholders` list that records which fields are stand-ins and why, so a later revision of the
same `@id` can replace them with real data.

## Motivation

- **No object may be dropped, no value guessed.** A fire damper exported as a volume-control
  damper is a safety error in every downstream document.
- **Placeholders must be findable.** Stand-ins are valid data, but someone has to replace them.
  A machine-readable list per object lets tools and `pmef check` report what is still open.
- **Replacement is a normal revision.** The `@id` stays; no relationship or new object type.

## Detailed Design

Normative text: spec 02 §2.10; schemas `pmef-base.schema.json` (`$defs/Placeholder`),
`pmef-hvac.schema.json` (`DuctComponent.placeholders`, `DuctDamper`).

1. `Placeholder` = `{path, reason, note?, expectedFrom?}`; `path` is a JSON Pointer relative to
   the object, `reason` ∈ `SOURCE_MISSING`, `NOT_IN_ENUM`, `TO_BE_CONFIRMED`.
2. `DuctComponent.placeholders`: optional array of `Placeholder` (min. 1 item).
3. `damperType` gains `UNSPECIFIED` and `CUSTOM`. Rules: `UNSPECIFIED` ⇒ `placeholders` contains
   `/damperType`; `CUSTOM` ⇔ `damperTypeCustom` present (`<namespace>:<name>`).
4. `pmef check` lists open placeholders as information; they never fail a check.

```json
{"@type":"pmef:DuctDamper","@id":"urn:pmef:obj:huldra:16815-1234","damperType":"UNSPECIFIED",
 "placeholders":[{"path":"/damperType","reason":"SOURCE_MISSING","expectedFrom":"HVAC damper schedule"}], …}
```

Replacement in revision 2 of the same `@id`: `"damperType":"FIRE","fireResistance":"EI 90 S"`, no `placeholders`.

### Amendment 2026-11-27: piping components

`placeholders` is also defined on `PipingComponent`. A valve whose type is not known keeps the
existing stand-in `componentClass: VALVE_UNSPECIFIED` (RFC-0009) and **SHOULD** list
`/componentSpec/componentClass` in `placeholders`. This is a SHOULD rather than a MUST, so files
written before the amendment stay valid. No new enumeration value is needed: a valve type outside
the list is `VALVE_UNSPECIFIED` with reason `NOT_IN_ENUM` and the type in `note`.
Example: `examples/piping-valve-placeholder.ndjson`.

## Alternatives considered

- **Make `damperType` optional.** Loses the distinction between "not known yet" and "forgot";
  nothing tells a consumer that a value is still due.
- **`customAttributes`.** Free-form and not validated; tools cannot find open items reliably.
- **Generic `x-` extension enums.** Violates closed enumerations of spec 02 §10.

## Compatibility

Additive. Readers of 0.9 that validate against closed enums reject `UNSPECIFIED`/`CUSTOM`;
writers targeting 0.9 must not emit them.

## Review Status

| Item | Evidence |
|---|---|
| Schema | `schemas/pmef-base.schema.json` `$defs/Placeholder`; `schemas/pmef-hvac.schema.json`; `schemas/pmef-piping-component.schema.json` (amendment) |
| Spec | spec 02 §2.10, §11.5 table |
| Rust | `pmef-core/src/placeholder.rs`, `hvac.rs` (`placeholders`, `damper_type_custom`), `piping.rs` (`placeholders`), `pmef check` |
| Example | `examples/hvac-damper-placeholder.ndjson`, `examples/piping-valve-placeholder.ndjson` |
| Tests | `tests/placeholders/run.py` (11 negative cases), `placeholder::tests::round_trip`, `valve_placeholder_round_trip`, schema drift |
| Use case | Huldra (Equinor), 36 dampers in HA/HB/HC HVAC; 1,509 valves without type (no SKEY in the export) |
