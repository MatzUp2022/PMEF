# RFC-0011: Catalog Snapping Version 2 (`pmef-snap/2`)

| Field | Value |
|---|---|
| **Status** | `DRAFT` — reference implementations and test vectors verified 2026-11-27; awaiting WG review and TSC sign-off |
| **Discussion** | [#45](https://github.com/MatzUp2022/PMEF/issues/45) — opened 2026-11-27, 30-day period until 2026-12-27 |
| **Target version** | 0.10.0 (new algorithm identifier; no schema change, no breaking change) |
| **Depends on** | [RFC-0002](0002-catalog-snapping.md) (`pmef-snap/1`; open question 2) |
| **Working groups** | WG-Catalogs, WG-Piping, WG-Steel, WG-Geometry |
| **Date** | 2026-11-27 |

## Abstract

`pmef-snap/1` snaps straight pipes and I-sections. Recognised plant models also contain fittings,
flanges, hollow sections, channels and angles; today they keep raw geometry or get a
tool-specific snap that cannot be reproduced. `pmef-snap/2` extends the same deterministic rule —
tolerance bands, normalised score, total order, runner-up, ambiguity margin 0.1 — to eight more
kinds, adds two selectors, and leaves every `pmef-snap/1` result unchanged.

## Motivation

- **UC-1 Fittings and flanges dominate piping item counts.** A spool from a scan has more elbows,
  tees, reducers and flanges than pipes. Without snapping they cannot get a catalog entry, so
  material take-off and as-built-versus-design comparison stop at the pipes.
- **UC-2 Secondary steel.** Bracing, platforms and pipe-rack secondary members are hollow
  sections, channels and angles. Their profile is what the structural model needs.
- **UC-3 Reproducibility.** The reason for RFC-0002 applies unchanged: two tools given the same
  measurement and catalog version must choose the same entry.

## Detailed Design

### Relation to `pmef-snap/1`

- The input, the tolerance-band rule, steps 1–7 of the algorithm, rounding, the ordering key,
  the ambiguity margin (0.1, part of the algorithm identity — RFC-0002 resolution 2) and the
  output fields are those of RFC-0002.
- For `pipe` and `i-section`, `pmef-snap/2` returns the same `catalogRef`, `score`, `residuals`,
  `runnerUp` and `ambiguous` as `pmef-snap/1`; only `algorithm` differs. `snappedSize` stays
  informative (version 2 also reads `DN` for the pipe designation).
- `pmef-snap/1` stays valid and unchanged; a `pmef-snap/1` implementation rejects the new kinds.

### New kinds

| Kind | Catalog / candidates | Parameters (order; first = primary) | Required |
|---|---|---|---|
| `channel` | `STEEL_PROFILES`, family UPE, UPN, C, MC | `h`, `b`, `tw`, `tf` | `h`, `b` |
| `rect-hollow` | `STEEL_PROFILES`, family RHS, SHS, HSS_RECT | `d1`, `d2`, `t` | `d1`, `d2` |
| `round-hollow` | `STEEL_PROFILES`, family CHS, HSS_ROUND | `outsideDiameter`, `wallThickness` | `outsideDiameter` |
| `angle` | `STEEL_PROFILES`, family EQUAL_ANGLE, UNEQUAL_ANGLE, L | `leg1`, `leg2`, `t` | `leg1`, `leg2` |
| `elbow` | `PIPING_CLASS`, `componentClass` ELBOW | `outsideDiameter`, `centreToFace` | `outsideDiameter` |
| `tee` | `PIPING_CLASS`, TEE | `outsideDiameter`, `branchOutsideDiameter`, `centreToEndRun`, `centreToEndBranch` | the two ODs |
| `reducer` | `PIPING_CLASS`, REDUCER, REDUCER_CONCENTRIC, REDUCER_ECCENTRIC | `outsideDiameter`, `reducedOutsideDiameter`, `length` | the two ODs |
| `flange` | `FLANGE_DIMENSIONS` (all entries) or `PIPING_CLASS` FLANGE, BLIND_FLANGE | `flangeOD`, `flangeThickness`, `boltCircle`, `boltHoleDiameter` | `flangeOD` |

**Orientation-free pairs.** A scan does not know which side of a hollow section is "h", nor which
leg of an angle is the first. `d1`/`d2` and `leg1`/`leg2` are the larger and the smaller outside
dimension; the measured values **MUST** satisfy `d1 ≥ d2` and `leg1 ≥ leg2` (otherwise the input
is invalid), and the catalog values are sorted the same way.

**Catalog field mapping** (first present wins; only `_mm` fields are read — an entry with inch
fields only has no candidates; `profiles-aisc-extended` carries `_mm` fields since 2024.3):

| Parameter | Catalog fields |
|---|---|
| channel `h`, `b`, `tw`, `tf` | as `i-section` (`h_mm`/`d_mm`, `b_mm`/`bf_mm`, `tw_mm`, `tf_mm`) |
| `d1`, `d2` | max / min of `h_mm`, `b_mm`; `t` ← `t_mm` |
| `leg1`, `leg2` | max / min of (`b1_mm` or `h_mm`), (`b2_mm` or `b_mm`); `t` ← `t_mm` |
| round-hollow | `outsideDiameter` ← `OD_mm`, `outsideDiameter_mm`; `wallThickness` ← `t_mm`, `wallThickness_mm` |
| fitting `outsideDiameter` | `outsideDiameter_mm`, else *class pipe OD* of the entry's DN |
| `branchOutsideDiameter` | *class pipe OD* of `branchDiameter_mm` |
| `reducedOutsideDiameter` | `reducedOD_mm`, else *class pipe OD* of `reducedDiameter_mm` / `reducingDiameter_mm` |
| `centreToFace` | `centreToFace_mm` |
| `centreToEndRun` / `centreToEndBranch` | `centreToEnd_run_mm` / `centreToEnd_branch_mm`, else `centreToEnd_mm` |
| `length` | `length_mm` |
| `flangeOD` | `flangeOD_mm`, else `outsideDiameter_mm` |
| `flangeThickness`, `boltCircle`, `boltHoleDiameter` | `flangeThickness_mm`; `boltCircle_mm`, `boltCircleDiameter_mm`; `boltHoleDiameter_mm` |

*Class pipe OD* of a DN is the `outsideDiameter_mm` of the first `PIPE` entry of the same catalog
(catalog order) whose `nominalDiameter_mm` (or `DN`) equals it. Fittings of a piping class have
the run ODs of the class's pipes; ASME classes in this repository store fitting dimensions
without ODs, EN classes store them on the entry. The two flange layouts of the piping classes
(docs/AUDIT.md P3.12, maintainer decision 2) are both covered by the `flangeOD` mapping.
`boltDiameter_mm` is not used: its meaning differs between catalogs.

### Selectors

Selectors filter candidates before scoring; they are not scored and not written as residuals
(their unit is not millimetres).

| Selector | Kind | Candidate passes if |
|---|---|---|
| `angle` [°] | `elbow` | \|`angle_deg` − angle\| ≤ 2.0 |
| `boltCount` | `flange` | `boltHoles` (or `numberOfBolts`) = boltCount |

An unknown selector is invalid input. Without `angle`, elbows of equal OD but different angle
tie and the snap is `ambiguous` — the correct outcome when the angle was not measured.

### Default tolerances [mm]

| Parameter | *t(p, c)* |
|---|---|
| `outsideDiameter`, `branchOutsideDiameter`, `reducedOutsideDiameter` | max(1.0, 0.01 · c) |
| `wallThickness`, rect-hollow `t` | max(0.5, 0.15 · c) |
| `h`, `b`, `d1`, `d2`, `leg1`, `leg2`, `centreToFace`, `centreToEnd*`, `length` | max(2.0, 0.02 · c) |
| `tw`, `tf`, angle `t` | max(1.0, 0.15 · c) |
| `flangeOD` | max(2.0, 0.01 · c) |
| `flangeThickness` | max(1.0, 0.10 · c) |
| `boltCircle` | max(1.5, 0.01 · c) |
| `boltHoleDiameter` | max(1.0, 0.05 · c) |

### Restriction

The optional restriction of RFC-0002 (`family`) compares with the entry's `family` for profile
kinds and with its `componentClass` for piping-class kinds (e.g. `FLANGE` to exclude blind
flanges). Dimension tables (`FLANGE_DIMENSIONS`) have no component class and drop out when a
restriction is given. `pipe` ignores the restriction, as in version 1.

### Output

`asBuilt.catalogSnap` as in RFC-0002 with `algorithm = "pmef-snap/2"`. No schema change: the
`algorithm` pattern, `residuals[].parameter` (free string) and `tolerances` (free keys) already
admit the new values. Informative `snappedSize`: profiles their `designation`, else the entry id
after `:` (channels: `<family> <size>`); elbows `DN <dn> <angle>° <rating>`; tees and reducers
`DN <dn>x<dn2> <rating>`; flanges `DN <dn> <rating>` (`Class <n>` for numeric ASME classes).

### Test vectors and reference implementations

`tests/snapping/vectors-v2.json` (CC0): 27 vectors — `pipe`/`i-section` equivalence with
version 1, every new kind, rotated hollow sections, ties broken by `catalogId` (elbows without
angle, EN 1092-1 PN10/PN16 with identical dimensions), class-pipe OD lookup for ASME fittings,
both flange layouts, selectors, a tolerance override, no-snap cases and invalid input.
Both implementations pass all vectors of both versions in CI:

- Python: `tests/snapping/reference_snap.py` (one table per kind, readable next to this RFC);
- Rust: `pmef_core::snapping::{snap_v2, snap_with}` with `pmef-core/tests/snapping_vectors.rs`.

## Backwards Compatibility

None. `pmef-snap/1` results, vectors and examples are unchanged. Rust API: `SnapKind` gains
variants (pre-1.0, additive in practice for callers that construct kinds), `snap` keeps its
signature; `SnapError` gains variants.

## Alternatives Considered

- **Snap fittings by DN only** — rejected: DN is not measurable; ODs are.
- **Angle as a scored parameter** — rejected: degrees cannot be mixed into a millimetre residual
  without an arbitrary scale; a selector with a fixed ±2° window is explainable.
- **Per-class flange field layouts in the algorithm** — rejected: a first-present field list
  covers both layouts without naming catalogs.
- **Version 2 as a separate identifier per kind** (`pmef-snap-flange/1`) — rejected: one
  identifier, one vector file, one conformance claim.

## Open Questions

1. **Inch-only catalogs** (AISC C, MC, WT): convert `_in` fields (× 25.4) in a version 3, or add
   `_mm` fields to the catalogs? *Proposed resolution:* add `_mm` fields to the catalogs (data
   change, no algorithm change) — done in `profiles-aisc-extended` 2024.3 (checked by
   `check-catalogs.py`, vector SNAP2-027); WT sections still have no snapping kind.
   *Decision: WG-Catalogs.*
2. **Olets, caps, crosses, valves** — out of scope; valves need face-to-face tables first.
   *Decision: WG-Piping.*

## References

- [RFC-0002](0002-catalog-snapping.md); spec 02 §2.9.4
- `catalogs/` (piping classes, flange tables, EN and AISC profiles)
