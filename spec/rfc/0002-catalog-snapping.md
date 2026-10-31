# RFC-0002: Deterministic Catalog Snapping (`pmef-snap/1`)

| Field | Value |
|---|---|
| **Status** | `DRAFT` — reference implementations and test vectors on a feature branch for review |
| **Target version** | 0.10.0 (new optional `catalogSnap` fields; no breaking change) |
| **Depends on** | [RFC-0001](0001-provenance-asbuilt.md) (`asBuilt.catalogSnap`) |
| **Working groups** | WG-Catalogs, WG-Piping, WG-Steel, WG-Geometry |
| **Date** | 2026-10-31 |

## Abstract

PMEF's exactness comes from catalog snapping: a recognised pipe becomes "DN 150 of class
EN-P1A", a recognised beam becomes "HEB 300", and the catalog — not the noisy measurement —
supplies the exact dimensions. RFC-0001 standardised how a snap *result* is recorded. This RFC
standardises the *algorithm*, so that two independent implementations given the same
measurement and the same catalog version choose the same entry, compute the same score and
flag the same ambiguities.

## Motivation

Without a normative algorithm, two recognisers can snap the same scan to different profiles,
and an as-built-versus-design comparison then reports differences that come from the tools,
not from the plant. A reproducible algorithm also lets reviewers trust the result: a recorded
score, runner-up and ambiguity flag show *how sure* the snap is, and tell a review queue which
objects need a human (UC-1 of RFC-0001).

## Detailed Design

### Scope of `pmef-snap/1`

| Kind | Catalog | Candidate entries | Parameters (order) | Required |
|---|---|---|---|---|
| `pipe` | `catalogType = PIPING_CLASS` | `componentClass = PIPE` | `outsideDiameter`, `wallThickness` | `outsideDiameter` |
| `i-section` | `catalogType = STEEL_PROFILES` | `family ∈ {HEA, HEB, HEM, IPE, W}`, optionally restricted to one family | `h`, `b`, `tw`, `tf` | `h`, `b` |

The first parameter of each kind is its **primary parameter**. Catalog field mapping:
`outsideDiameter ← outsideDiameter_mm`, `wallThickness ← wallThickness_mm`,
`h ← h_mm | d_mm`, `b ← b_mm | bf_mm`, `tw ← tw_mm`, `tf ← tf_mm` (first present wins; this covers
the EN and AISC catalog conventions). All values are millimetres.

Fittings, flanges, hollow sections, channels and angles are out of scope for version 1 and
will be added as new kinds (`pmef-snap/2` or later) without changing version 1 results.

### Input

- the catalog document (with `catalogId` and `version`; unversioned catalogs are rejected),
- the kind, the measured parameters (any subset of the kind's parameters that includes the
  required ones; unknown parameters are an error),
- optionally a family restriction (`i-section`) and non-default tolerances.

Parameters that cannot be measured (e.g. pipe wall thickness from an outside scan) are simply
omitted; they are then not used for scoring.

### Tolerance band

For measured parameter *p* and candidate catalog value *c*:

| Parameter | Default tolerance *t(p, c)* [mm] |
|---|---|
| `outsideDiameter` | max(1.0, 0.01 · c) |
| `wallThickness` | max(0.5, 0.15 · c) |
| `h`, `b` | max(2.0, 0.02 · c) |
| `tw`, `tf` | max(1.0, 0.15 · c) |

The defaults are deliberately wider than typical mill tolerances to absorb measurement noise
from scans and drawings. A caller **MAY** pass other tolerances; they **MUST** then be recorded
in `catalogSnap.tolerances` (only the parameters actually used).

### Algorithm

1. Candidates are the catalog entries of the kind (and family, if given) that provide a value
   for every required parameter and every measured parameter.
2. For each candidate compute the normalised residuals
   *n_p = (measured_p − c_p) / t(p, c_p)* for all measured parameters *p*.
3. A candidate is **admissible** iff |*n_p*| ≤ 1 for every measured parameter.
4. Score = √( Σ *n_p*² / *k* ), *k* = number of measured parameters, rounded to 6 decimals
   (round half away from zero). Admissible candidates have scores in [0, 1].
5. Candidates are ordered by (score, |measured − catalog| of the primary parameter, `catalogId`
   by Unicode code point). The order is total, so the result is deterministic.
6. If at least one candidate is admissible, the first one is the snap. The second one (if any)
   is the **runner-up**. The snap is **ambiguous** iff a runner-up exists and
   round6(runner-up score − score) < 0.1.
7. If no candidate is admissible, there is **no snap**. Implementations SHOULD report the first
   candidate of the same ordering over *all* candidates as `nearest` (for review tooling); it is
   not written to PMEF data.

Residuals are written as `residual_mm = round6(measured − catalog)` in parameter order.

### Output (`asBuilt.catalogSnap`)

RFC-0001 fields plus the optional fields introduced here:

| Field | Value for `pmef-snap/1` |
|---|---|
| `catalogRef` | `catalogId`, `catalogVersion` (= catalog `version`), `entryId` (= entry `catalogId`) |
| `snappedSize` | informative designation: pipes `DN <nominalDiameter_mm> <catalog rating>`; I-sections the entry `designation` if present, else `<family> <size>` |
| `residuals` | one entry per measured parameter |
| `algorithm` | `pmef-snap/1` (**MUST** be set by implementations of this RFC) |
| `score` | as above |
| `runnerUp` | `{entryId, score}` of the second admissible candidate |
| `ambiguous` | boolean |
| `tolerances` | only when non-default tolerances were used |

### Rules for recognisers

- A snap with `ambiguous = true` **MUST NOT** be created with `review.status = approved`;
  it SHOULD be created as `detected`.
- When there is no snap, the object keeps its measured geometry, has no `catalogSnap`, and
  SHOULD be created as `detected`.
- Conformance is defined on `catalogRef.entryId`, `score`, `runnerUp`, `ambiguous` and
  `residuals`; `snappedSize` is informative and not compared.

### Test vectors and reference implementations

`tests/snapping/vectors.json` (CC0) contains 16 language-neutral vectors: exact hits, scan
noise, missing wall thickness, no-snap cases, family filters, an ambiguous AISC case, a tie
decided by `catalogId`, tolerance overrides and invalid input. Two independent
implementations pass all vectors and run in CI:

- Python reference: `tests/snapping/reference_snap.py` (no dependencies, readable next to this RFC);
- Rust: `pmef_core::snapping::snap` with `pmef-rs/pmef-core/tests/snapping_vectors.rs`.

Expected results are pinned to catalog versions; correcting catalog data changes
`catalogVersion` and, deliberately, the expected results.

### Schema changes

`pmef-base.schema.json` `$defs/CatalogSnap` gains the optional properties `algorithm`
(pattern `^[a-z0-9-]+/\d+$`), `score` (0..1), `runnerUp` (`entryId`, `score`), `ambiguous`
(boolean) and `tolerances` (parameter → mm > 0). `pmef-core::CatalogSnap` gains the same fields.

## Backward Compatibility

Additive only. Snaps without `algorithm` (manual or tool-specific) stay valid.

## Alternatives Considered

- **Nearest by Euclidean distance in mm** — rejected: mixes parameters of very different
  size (h ≈ 300 mm vs. tf ≈ 19 mm) and has no notion of "too far".
- **Accept the best candidate even outside the band** — rejected: silently turns a
  non-catalog item into a catalog item, the opposite of what review needs.
- **Probabilistic scores** — rejected for version 1: harder to make bit-identical across
  languages; the normalised residual is simple and explainable.

## Open Questions

1. ~~Catalog data quality~~ — resolved: `profiles-en` 2024.2 corrects the HEA series (heights had
   been the nominal size, so HEA and HEB of one size could not be told apart). Vector SNAP-017
   now covers that case. Other catalogs still need the same check (`docs/AUDIT.md` P1.8).
2. Kinds for fittings (elbow radius/angle), flanges, hollow sections and angles (`pmef-snap/2`).
3. Should the ambiguity margin be configurable per project (then recorded like tolerances)?

## References

- [RFC-0001](0001-provenance-asbuilt.md); spec 02 §2.9.3–§2.9.4
- `catalogs/` (piping classes, EN and AISC profiles)
