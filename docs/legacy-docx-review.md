# Review of the Legacy `.docx` Files

**Date:** 2026-10-31 · **Scope:** TASK_01 Phase 2 — "move normative content from the .docx files
into `spec/` where it is missing".
**Result:** nothing was moved. Neither file contains normative content that is missing from
`spec/` + `schemas/`. Both files stay in the repository as non-normative legacy artefacts.

## 1. `PMEF_Specification_v1.0-draft_DE.docx`

Formerly `PMEF_Specification_v1.2.docx` (renamed — title page: "Version 1.0 – April 2026",
Document ID `PMEF-SPEC-0001 Rev.1`, status "Release Candidate – Community Review"). German.

**Normative language:** none (0 occurrences of MUST / SHALL / MUSS / SOLL / "normativ").
The document is a market and format landscape analysis with proposals.

| Chapter | Content | Already in `spec/` / `schemas/`? | Action |
|---|---|---|---|
| 1–4, 9 | CAx system landscape, format analysis, compatibility matrix (35+ systems, 42+ formats) | No — informative market analysis | None. Not specification material. |
| 5.1 | Proposed entity classes: PLCObject, InstrumentLoop, CableObject, StressAnalysisObject, SimulationObject, CivilObject, TerrainObject ("Auswahl" — examples) | PLC, loop, cable, cable tray: yes (`pmef-ei.schema.json`). Stress, Simulation, Civil, Terrain: no | Missing ones are **frozen disciplines** → backlog, each needs an RFC |
| 5.2 | Proposed property sets (PLC hardware, instrument safety/SIL, cable electrical, pipe stress, simulation, civil foundation, eCl@ss) | Partly via E&I schema | Backlog → RFC |
| 6, 10, 11 | Adapter roadmap, crate plan, gaps, roadmap to 2028 | Superseded by `ROADMAP.md` | None |
| 7, 8 | NAMUR NOA data flow; `.cii`/`.ntr` pipe-stress interface | No | Frozen scope → backlog |
| 13 | Simulation / virtual commissioning, `pmef:SimulationObject` example, DS-06 | No | Frozen scope → backlog |
| 14 | E&I, structural steel, typed relationship model, DS-04 | Yes (`pmef-ei`, `pmef-steel`, `pmef-relationships` schemas; `examples/ei-loop-fic-10101.ndjson`) | None |
| Annex A | `InstrumentObject` JSON Schema | Yes, normative version in `pmef-ei.schema.json` | None — the docx version conflicts (below) |

**Conflicts with the normative spec** (spec + schemas win):

- `$id` base `https://pmef.org/schemas/1.0/…` vs. normative `https://pmef.net/schemas/0.9/…`.
- `SimulationObject` example `@id` `urn:pmef:sim:eaf-line3-cooling-sim` has only four segments and
  violates the `PmefId` pattern (`urn:pmef:<domain>:<project>:<local-id>`); `sim` is not a
  registered domain (spec 01 §7.3).
- Version "1.0" vs. normative 0.9.0.

## 2. `PMEF_CHANGELOG_v0.9.0-rc.docx`

Same content as the `[0.9.0-rc]` section of `CHANGELOG.md` (no file names or identifiers
appear in the docx that are missing from `CHANGELOG.md`). No normative content.

**Finding:** both the docx and `CHANGELOG.md` list "Conformance Test Suite (`pmef-conformance/`),
82 tests, 80/82 passing". That directory is not in the repository and never was (checked
against the full Git history). The released changelog entry is kept unchanged but annotated.

## 3. Backlog candidates (require an RFC each, frozen scope)

- Stress analysis entities + `.cii`/`.ntr` mapping
- Simulation entities (`SimulationObject`, FMU / CAPE-OPEN / OPC UA references)
- Civil / terrain entities
- Safety property set (SIL, PFD, proof-test interval) for E&I
- eCl@ss classification property set
