# RFC-0003: Persisted As-Built ↔ Design Correspondence

| Field | Value |
|---|---|
| **Status** | `DRAFT` — implemented on a feature branch for review |
| **Target version** | 0.10.0 (one new optional property; no breaking change) |
| **Depends on** | [RFC-0001](0001-provenance-asbuilt.md) (`review` block), spec 03 §9.5 (matching) |
| **Working groups** | WG-Piping, WG-Steel, WG-Equipment, TSC |
| **Date** | 2026-10-31 |

## Abstract

`pmef diff --match` (spec 03 §9.5) finds which as-built object corresponds to which design
object even when their `@id`s differ. This RFC makes such a correspondence storable PMEF data, so
that a reviewer's decision — *this* scanned beam is design beam B-101, *that* geometric match is
wrong — is recorded once and reused by every later comparison.

## Motivation

Matching by geometry is a proposal, not a fact. Without persistence, every comparison run
re-derives the same proposals, a wrong match comes back after it has been rejected, and the
review work is lost. Use cases:

- **UC-1 Review once:** proposed matches are reviewed (approve / reject) and stored with the
  as-built model.
- **UC-2 Stable comparison:** the next as-built-versus-design run uses the confirmed matches as
  given and never proposes a rejected pair again.
- **UC-3 Traceability:** the record shows how a match was found (tag, tool id, geometry with
  distance, reference) and who confirmed it.

## Detailed Design

### Relationship: reuse `pmef:IsRevisionOf` with `changeType = AS_BUILT`

A correspondence is a `pmef:IsRevisionOf` (spec 02 §8.3) whose `sourceId` is the as-built
object, whose `targetId` is the design object and whose `changeType` is `AS_BUILT`. The type
and the `AS_BUILT` value already exist: the as-built state *is* the realised revision of the
designed object. `targetId` usually refers to an object of another package (the design model);
this is allowed for full `urn:pmef:` URIs (spec 03 §8.4).

### New optional property `correspondence`

| Field | Req. | Type | Description |
|---|---|---|---|
| `method` | **MUST** | enum | `tag`, `equivalent`, `geometry`, `reference` (spec 03 §9.5 passes) or `manual` |
| `distance_mm` | SHOULD for `geometry` | number ≥ 0 | mean anchor distance |
| `ambiguous` | MAY | boolean | a competing candidate was nearly as close |

If `correspondence` is present, `changeType` **MUST** be `AS_BUILT` (schema-enforced).

### Confirmation: the RFC-0001 `review` block

| `review.status` | Meaning for matching |
|---|---|
| absent, `reviewed`, `approved` | **binding** — the pair is matched before any evidence pass |
| `detected` | proposal only — ignored; matching recomputes it |
| `rejected` | **forbidden** — the pair is never matched by any pass |

Writers of proposals **MUST** set `review.status = detected` (with `revision.changeState = WIP`,
RFC-0001 rule). A correspondence written by a person without review block counts as binding.

### Effect on matching (spec 03 §9.5)

1. Correspondence relationships in either package are evidence; they are removed from the sets
   of objects to match and are not compared or reported as added/removed.
2. A new pass **1b `declared`** follows pass 1 (`id`): binding correspondences whose endpoints
   exist on the two sides are matched, in (base `@id`, head `@id`) order; a declaration whose
   objects are already matched is skipped.
3. Forbidden pairs are excluded from all later passes.

### Proposals: `pmef diff --match --emit-matches FILE`

Writes one correspondence per match found by evidence (not for `id` or `declared`):
`@id` = `urn:pmef:rel:<project of the as-built id>:<local id>-asbuilt`, `derivedBy = RULE_BASED`,
`review.status = detected`, `revision.changeState = WIP`, objects with sorted keys. The file can
be reviewed, edited and appended to the as-built package.

### Example

```jsonc
{"@type":"pmef:IsRevisionOf","@id":"urn:pmef:rel:demo-asb:B-101-asbuilt","relationType":"IS_REVISION_OF",
 "sourceId":"urn:pmef:obj:demo-asb:B-101","targetId":"urn:pmef:obj:demo-dsn:B-101","changeType":"AS_BUILT",
 "correspondence":{"method":"geometry","distance_mm":12.5},"derivedBy":"RULE_BASED",
 "review":{"status":"approved","reviewedBy":"reviewer-01","reviewedAt":"2026-10-30T16:05:00Z"}, …}
```

See `examples/asbuilt-recognition.ndjson` (one approved, one detected-and-ambiguous).

## Backward Compatibility

Additive. Existing `IsRevisionOf` objects are unaffected; existing `AS_BUILT` revisions without
`correspondence` become binding correspondences when used with `pmef diff --match` — which is
their meaning anyway.

## Alternatives Considered

- **New relationship type `pmef:IsAsBuiltOf`** — clearer name, but duplicates the existing
  `IsRevisionOf`/`AS_BUILT` semantics and adds a type every reader must learn.
- **`pmef:HasEquivalentIn` with `targetSystem = "PMEF"`** — rejected: that type records native
  tool identities for adapter round trips; mixing PMEF-to-PMEF links into it blurs pass 3.
- **Store matches only in diff reports** — rejected: not part of the model, lost on the next run.

## Open Questions

1. Delta packages (spec 03 §8.4) also use `IsRevisionOf`; should a delta writer distinguish
   `AS_BUILT` correspondences from content revisions in its own processing? (Proposed: yes — only
   `changeType` values other than `AS_BUILT` mark content replacement.)
2. Should `pmef merge` (future) apply confirmed correspondences to rename as-built ids to design ids?

## Implementation Notes

- Schema: `pmef-relationships.schema.json` `IsRevisionOf.correspondence` → `$defs/AsBuiltCorrespondence`
  plus the `AS_BUILT` rule; Rust `relationships::AsBuiltCorrespondence`, `CorrespondenceMethod`.
- Matching: `pmef_core::matching` (pass 1b, forbidden pairs, `proposals`), `pmef diff --emit-matches`.
- Tests: `pmef-validate/tests/correspondence.rs` (schema rules), unit tests in `matching.rs`,
  CLI workflow test (propose → approve/reject → rerun) in `pmef-cli/tests/diff_cli.rs`.
