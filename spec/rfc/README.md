# PMEF RFCs

Normative changes to the specification and schemas go through an RFC
([CONTRIBUTING.md](../../CONTRIBUTING.md#specification-change-process-rfc),
[GOVERNANCE.md](../../GOVERNANCE.md)): RFC issue → at least 30 days of discussion → working-group
approval → TSC sign-off → merge with the RFC number in `CHANGELOG.md`.

Status values (issue template): `DRAFT` → `REVIEW` → `APPROVED` / `REJECTED` / `SUPERSEDED`.

| RFC | Title | Target | Status | Decisions needed |
|---|---|---|---|---|
| [0001](0001-provenance-asbuilt.md) | Recognition provenance, review status and as-built data | 0.10.0 | `REVIEW` | `$id` path policy (TSC); `review.comment`/`issueRef` deferred? (WGs) |
| [0002](0002-catalog-snapping.md) | Deterministic catalog snapping `pmef-snap/1` | 0.10.0 | `REVIEW` | `pmef-snap/2` as follow-up; fixed ambiguity margin (WG-Catalogs) |
| [0003](0003-as-built-correspondence.md) | Persisted as-built ↔ design correspondence | 0.10.0 | `REVIEW` | `AS_BUILT` is not a content revision in delta packages (TSC) |
| [0004](0004-polytropic-efficiency-camelcase.md) | `polytropicEfficiency` camelCase spelling | 0.10.0 / 1.0.0 | `APPROVED` | — |

RFCs 0001–0003 are fully implemented in this repository (schemas, spec text, Rust reference
implementation, examples, tests in CI); each RFC's *Review Status* section lists the evidence.
Ready-to-file issue texts: [`docs/rfc-review-2026-11.md`](../../docs/rfc-review-2026-11.md).
