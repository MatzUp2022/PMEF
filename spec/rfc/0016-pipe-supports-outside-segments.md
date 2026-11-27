# RFC-0016: Pipe supports outside piping segments and support assemblies

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust model, catalog, example and tests in this repository |
| **Discussion** | [#72](https://github.com/MatzUp2022/PMEF/issues/72), 30-day period 2026-11-27 → 2026-12-27 |
| **Target version** | 0.10.0 (additive: one optional field, wider meaning of `isPartOf`, four catalog entries) |
| **Depends on** | spec 02 §4.4, §8.3, §9.11; RFC-0009 (native type catalog) |
| **Working groups** | WG-Piping, WG-Adapters |
| **Date** | 2026-11-27 |

## Abstract

`pmef:PipeSupport` inherits from `PipingComponent` and is placed under a `PipingSegment`. In
the Huldra E3D model 520 support assemblies (`REST`, with 716 hangers `HANG` and 1,059 pipe
clamps `PCLA`) live in their own support zones, not under a pipe branch, so an exporter cannot
place them (issue [#53](https://github.com/MatzUp2022/PMEF/issues/53), point 4). This RFC lets
a support's `isPartOf` name the unit/area of the support zone or the assembly it belongs to, and
adds `supportedIds` for the carried pipes. E3D `REST`, `HANG`, `PCLA` and `SHU` map to
`PipeSupport`.

## Motivation

- **No object may be dropped.** Supports carry the loads for stress analysis and the MTO.
- **The carried pipe must be explicit.** Without a segment parent the link to the pipe is lost.
- **Assemblies are common.** Hangers and clamps are parts of one support.

## Detailed Design

Normative text: spec 02 §9.11 (subsection *Supports outside piping segments*); schema
`pmef-piping-component.schema.json` (`PipeSupport`).

1. `PipeSupport.isPartOf`: `PipingSegment`/`PipingNetworkSystem` (unchanged), otherwise the
   `Unit`/`Area` of the support zone, or the assembly `PipeSupport` for hangers, clamps, shoes.
2. `PipeSupport.supportedIds` (optional, ≥ 1, unique): carried pipes, segments or lines.
   **SHOULD** be present when `isPartOf` is not a piping segment or system. Equivalent to
   `pmef:Supports` relationships with the support as `sourceId`; both **MUST** agree.
3. `componentSpec` stays required (`{"componentClass":"PIPE_SUPPORT"}` as minimum).
4. `catalogs/native-types-e3d.json` 2026.2: `REST`, `HANG`, `PCLA`, `SHU` → `pmef:PipeSupport`.

## Alternatives considered

- **Make `componentSpec` optional for supports (as proposed in #53).** Splits the
  `PipingComponent` base contract for one subtype; the minimal value costs one line.
- **Only `pmef:Supports` relationships.** Valid and kept as equivalent, but a field on the object
  is easier for exporters and keeps the support self-describing, like `structuralAttachmentId`.
- **New `SupportAssembly` type.** An assembly is itself a support with a mark and design data;
  nesting `PipeSupport` is enough.

## Compatibility

Additive. The Rust `SupportType` enum is aligned with the schema enumeration (`HANGER`,
`CLAMP`, `STRAP`, `SADDLE`, `DUMMY_LEG`, `SLIDE_PLATE`, `DIRECTIONAL`); its former extra
variants were never schema-valid.

## Review Status

| Item | Evidence |
|---|---|
| Schema | `schemas/pmef-piping-component.schema.json` (`PipeSupport.isPartOf`, `supportedIds`) |
| Spec | spec 02 §4.4 table, §8.3 `pmef:Supports` row, §9.11 |
| Rust | `pmef-core/src/piping.rs` (`supported_ids`, `SupportType`) |
| Catalog | `catalogs/native-types-e3d.json` 2026.2; `pmef-catalogs` test |
| Example | `examples/pipe-support-assembly.ndjson` |
| Conformance | `PIPE-U-019` … `PIPE-U-021` |
