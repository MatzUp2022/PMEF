# RFC Review Package — November 2026

Filed on 2026-11-01 as [#6](https://github.com/MatzUp2022/PMEF/issues/6) (RFC-0001),
[#7](https://github.com/MatzUp2022/PMEF/issues/7) (RFC-0002) and
[#8](https://github.com/MatzUp2022/PMEF/issues/8) (RFC-0003); discussion period until 2026-12-01.

Issue texts for opening the review of RFC-0001 to RFC-0003 with the
[RFC template](../.github/ISSUE_TEMPLATE/rfc.md) (labels `RFC`, `needs-triage`). Each issue starts
the 30-day discussion period (GOVERNANCE.md). The RFC documents in `spec/rfc/` are the full text;
the issues summarise and collect the decisions.

All three RFCs are additive for 0.10.0 (new optional properties only) and implemented with tests
in CI; verification date 2026-11-01.

## Issue 1 — `[RFC-0001] Recognition provenance, review status and as-built data`

**Status:** `REVIEW` · **Working groups:** WG-Piping, WG-Steel, WG-Equipment, WG-Geometry,
WG-Catalogs

**Abstract:** Objects created by automated recognition (drawings, isometrics, point clouds) carry
`provenance` (source, recogniser, confidence), `review` (detected / reviewed / approved / rejected,
tied to the ISO 19650 change state) and `asBuilt` (point-cloud fit quality, catalog snap).

**Full text:** [`spec/rfc/0001-provenance-asbuilt.md`](../spec/rfc/0001-provenance-asbuilt.md),
normative text spec 02 §2.9.

**Decisions requested:**

1. `$id` paths: keep `/0.9/` through 0.x and move to `/1/` with 1.0.0 (proposed) — TSC.
2. `review.comment` / `issueRef`: defer to a follow-up RFC (proposed) — WGs.

## Issue 2 — `[RFC-0002] Deterministic catalog snapping (pmef-snap/1)`

**Status:** `REVIEW` · **Working groups:** WG-Catalogs, WG-Piping, WG-Steel, WG-Geometry

**Abstract:** A deterministic, language-independent rule that snaps measured dimensions (pipe OD
and wall, I-section h/b/tw/tf) to a catalog entry, with tolerance bands, a normalised score, a
runner-up and an ambiguity flag. Python and Rust reference implementations agree on shared test
vectors.

**Full text:** [`spec/rfc/0002-catalog-snapping.md`](../spec/rfc/0002-catalog-snapping.md).

**Decisions requested:**

1. Further kinds (fittings, flanges, hollow sections, angles) as `pmef-snap/2` in a follow-up
   RFC (proposed) — WG-Catalogs.
2. Ambiguity margin fixed per algorithm identifier, not per project (proposed) — WG-Catalogs.

## Issue 3 — `[RFC-0003] Persisted as-built ↔ design correspondence`

**Status:** `REVIEW` · **Working groups:** WG-Piping, WG-Steel, WG-Equipment; TSC

**Abstract:** A reviewed match between an as-built object and its design object is stored as
`pmef:IsRevisionOf` with `changeType = AS_BUILT` and a `correspondence` block (method, distance,
ambiguity), so `pmef diff --match` reuses approved matches and never re-proposes rejected ones.

**Full text:** [`spec/rfc/0003-as-built-correspondence.md`](../spec/rfc/0003-as-built-correspondence.md).

**Decision requested:**

1. In delta packages, an `IsRevisionOf` with `changeType = AS_BUILT` is a correspondence, not a
   content revision; one sentence is added to spec 03 §8.4 on approval (proposed) — TSC.

## After approval

Set the RFC status to `APPROVED`, apply the agreed resolutions to the spec text, record the RFC
number in `CHANGELOG.md` and update `spec/rfc/README.md` and the README status table.
