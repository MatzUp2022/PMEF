# RFC-0004: `polytropicEfficiency` — camelCase Spelling for `CompressorSpec`

| Field | Value |
|---|---|
| **Status** | `APPROVED` — maintainer decision 2026-11-01 (docs/AUDIT.md P3.10) |
| **Target version** | 0.10.0 (deprecation, non-breaking); 1.0.0 (removal of the old spelling, breaking) |
| **Depends on** | — |
| **Working groups** | WG-Equipment, TSC |
| **Date** | 2026-11-01 |

## Abstract

`CompressorSpec.polytropic_efficiency` is the only snake_case property in the PMEF schemas; every
other property is camelCase (spec 03 naming). This RFC introduces `polytropicEfficiency`,
deprecates the old spelling in 0.10.0 and removes it in 1.0.0.

## Motivation

Mixed naming breaks generated bindings and mappings: code generators produce a lone
`polytropic_efficiency` member, mapping tables to DEXPI / CFIHOS need a special case, and the Rust
reference model needed an explicit rename. Recognition pipelines that emit PMEF from templates
are especially prone to writing the "wrong" spelling.

## Detailed Design

### 0.10.0 — deprecation (non-breaking)

- `CompressorSpec` gains the optional property `polytropicEfficiency` (number, [%]).
- `polytropic_efficiency` stays valid and is marked `"deprecated": true` in the schema.
- An object **MUST NOT** carry both spellings (schema: `not: { required: [both] }`).
- Readers **MUST** accept both spellings; writers **MUST** write `polytropicEfficiency`.
  The Rust reference model reads both and writes camelCase.

### 1.0.0 — removal (breaking)

`polytropic_efficiency` is removed from the schema. Files written by 0.10 writers are already
compliant; only files that still carry the deprecated spelling must be migrated (rename the key).

## Backwards Compatibility

0.10.0: none — every 0.9 file stays valid. 1.0.0: files with the old spelling become invalid
(`additionalProperties: false`); the migration is a mechanical key rename.

## Alternatives Considered

- **Keep the snake_case spelling** — rejected: permanent special case for every consumer.
- **Rename immediately in 0.10.0** — rejected: breaks valid 0.9 files within a minor version
  (CLAUDE.md versioning rule).

## Reference Implementation

Schema `pmef-property-sets` (`CompressorSpec`), spec 02 §9.10, `examples/compressor-train.ndjson`,
`pmef-core` (`CompressorSpec.polytropic_efficiency` with serde alias), tests in
`pmef-validate/tests/rfc0004.rs` and `pmef-core/tests/rfc0004_roundtrip.rs`.
