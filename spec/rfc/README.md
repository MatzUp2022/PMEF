# PMEF RFCs

Normative changes to the specification and schemas go through an RFC
([CONTRIBUTING.md](../../CONTRIBUTING.md#specification-change-process-rfc),
[GOVERNANCE.md](../../GOVERNANCE.md)): RFC issue → at least 30 days of discussion → working-group
approval → TSC sign-off → merge with the RFC number in `CHANGELOG.md`.

Status values (issue template): `DRAFT` → `REVIEW` → `APPROVED` / `REJECTED` / `SUPERSEDED`.

| RFC | Title | Target | Status | Issue | Decisions needed |
|---|---|---|---|---|---|
| [0001](0001-provenance-asbuilt.md) | Recognition provenance, review status and as-built data | 0.10.0 | `REVIEW` | [#6](https://github.com/MatzUp2022/PMEF/issues/6) | `$id` path policy (TSC); `review.comment`/`issueRef` deferred? (WGs) |
| [0002](0002-catalog-snapping.md) | Deterministic catalog snapping `pmef-snap/1` | 0.10.0 | `REVIEW` | [#7](https://github.com/MatzUp2022/PMEF/issues/7) | `pmef-snap/2` as follow-up; fixed ambiguity margin (WG-Catalogs) |
| [0003](0003-as-built-correspondence.md) | Persisted as-built ↔ design correspondence | 0.10.0 | `REVIEW` | [#8](https://github.com/MatzUp2022/PMEF/issues/8) | `AS_BUILT` is not a content revision in delta packages (TSC) |
| [0004](0004-polytropic-efficiency-camelcase.md) | `polytropicEfficiency` camelCase spelling | 0.10.0 / 1.0.0 | `APPROVED` | — | — |
| [0005](0005-pmefx-object-index.md) | PMEFX 1.0: object index, mesh LODs, exact B-rep, octree | 0.10.0 | `DRAFT` | [#15](https://github.com/MatzUp2022/PMEF/issues/15) | LOD error metric; ZIP vs. HDF5 criteria (WG-Serialisation) |
| [0006](0006-hvac-ducts-tray-fittings-catalogs.md) | HVAC duct domain, cable tray fittings, support and cable catalogs | 0.10.0 | `DRAFT` | — | Air terminals/AHUs, duct hangers, tray supports (follow-ups) |

RFCs 0001–0003 are fully implemented in this repository (schemas, spec text, Rust reference
implementation, examples, tests in CI); each RFC's *Review Status* section lists the evidence.
Review issues were filed on 2026-11-01 (texts: [`docs/rfc-review-2026-11.md`](../../docs/rfc-review-2026-11.md)); the
30-day discussion period ends on 2026-12-01.
