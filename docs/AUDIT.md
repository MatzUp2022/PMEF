# PMEF Repository Audit — Phase 1

**Date:** 2026-10-31 · **Commit audited:** `90b6ea5` (main, after PR #4)
**Scope:** TASK_01 Phase 1 — read-only audit. No code, schema, spec or example was changed.
**Toolchain used:** rustc/cargo 1.97.0, Python 3 with `jsonschema` 4.26.0.

Status legend: **real** = verifiable and true · **placeholder** = points at something that
does not exist (yet) · **missing** = referenced file or feature not in the tree ·
**false** = contradicted by the tree.

---

## 1. README vs. actual tree

| README says | Actual tree | Status |
|---|---|---|
| Repo path `pmef/specification` (structure block, badge URL, `Cargo.toml` `repository`, `CONTRIBUTING.md`, `docs/getting-started.md`, `spec/01-introduction.md`, issue templates) | Repo is `MatzUp2022/PMEF`; no `pmef` GitHub org verified | false |
| `adapters/` with `plant3d/`, `cadmatic/`, `e3d/`, `openplant/` | No `adapters/` folder. Adapters live in `pmef-rs/pmef-adapter-*` (10 crates) and root folders `pmef-*-dotnet`, `pmef-inventor-addin`, `pmef-creo-toolkit`, `pmef-dotnet-adapters`. No OpenPlant adapter anywhere | false |
| `catalog/` with `profiles-en.json`, `profiles-aisc.json`, `piping-class-a1a2.json` | Folder is `catalogs/` with 17 files | false (path) |
| `schemas/` with 5 files | 10 files (adds catalog, ei, hierarchy, relationships, steel) | outdated |
| `examples/` with `pump-skid-complete`, `heat-exchanger-station`, `eaf-segment.ndjson` (DS-05) | 5 files: `pump-skid-complete` (DS-01), `heat-exchanger-station` (DS-02), `compressor-train` (DS-03), `ei-loop-fic-10101` (DS-04), `stress-model-cw201`. **`eaf-segment.ndjson` (DS-05) does not exist** | partly false |
| `docs/dexpi-alignment.md` | missing (docs has `adapter-overview.html`, `design-decisions.md`, `getting-started.md`, `iso15926-mapping.md`) | missing |
| `docs/assets/pmef-logo.svg` (header image) | missing → broken image | missing |
| `.github/PULL_REQUEST_TEMPLATE/` (folder) | single file `.github/pull_request_template.md` | false (path) |
| `pmef-rs/` (Rust core) | exists but **not mentioned** in the structure block | omitted |
| Root `.docx` files | exist, not mentioned | omitted |
| Architecture box: adapters for E3D, Smart 3D, Plant 3D, CADMATIC, OpenPlant, Tekla, Revit, EPLAN P8, COMOS, INOSIM, Creo, CATIA, TIA Portal, SAP EAM, Emulate3D | Code (any state) exists only for: E3D, Plant 3D, CADMATIC, COMOS, Creo, Inventor, Navisworks, Revit, Tekla, Advanced Steel. **No code** for Smart 3D, OpenPlant, EPLAN, INOSIM, CATIA, TIA Portal, SAP EAM, Emulate3D | over-claim |
| "PMEF Package (.pmefx)" | Container is specified in `spec/03-serialisation.md` §3 (archive layout, manifest, split files) — but **no reader/writer** in `pmef-rs` | spec only |
| "15 primitive types" | consistent with `pmef-geometry.schema.json` / `pmef-geom` (not re-counted in detail) | real (not deeply verified) |

Correction to the earlier informal review: PMEFX **is** defined in the spec (§3 of
`03-serialisation.md`); what is missing is an implementation.

## 2. Version conflicts

| Location | Version | Note |
|---|---|---|
| README badge | `v0.9-rc` | |
| `CHANGELOG.md` | `[0.9.0-rc] — 2026-03-31` + `[Unreleased]` + two more `[Unreleased]` headings ("v0.9.1 Planned", "v1.0 Planned") | three `Unreleased` sections break Keep-a-Changelog and `extract-changelog.py` lookups |
| Schemas `$id` | `https://pmef.net/schemas/0.9/…` | consistent across all 10 |
| Schemas `pmefVersion` | pattern `^\d+\.\d+\.\d+$` | **rejects `0.9.0-rc`**, so the version string in the README can never appear in data |
| Examples `pmefVersion` | `0.9.0` (all 78 occurrences) | |
| Spec Markdown | "v0.9", footer "PMEF Specification v0.9.0-rc" (`06-conformance.md`) | |
| `pmef-rs` workspace + CLI `--version` | `0.9.0` | |
| `PMEF_CHANGELOG_v0.9.0-rc.docx` | 0.9.0-rc | consistent |
| `PMEF_Specification_v1.2.docx` | **file name says v1.2, title page says "Version 1.0 – April 2026", Document ID `PMEF-SPEC-0001 Rev.1`, predecessor "v0.9, März 2026"**; German; claims "35+ CAx systems, 42+ formats analysed" | conflicts internally and with everything else |
| `ROADMAP.md` | v0.9 = current; v1.0/v1.1/v1.2 = planned 2026 Q3 – 2027 Q1 | the "v1.2" docx name collides with the roadmap's future v1.2 (MVP-3) |

**Recommendation:** the normative version is **0.9.0** (spec + schemas + examples + Rust agree);
`-rc` is a release label, not a data version. The docx is a non-normative v1.0 *draft*
mislabelled as v1.2. Decision needed from maintainer (see §8, Q1).

## 3. Placeholder / unverifiable claims

| Claim | Status | Evidence |
|---|---|---|
| Discord `discord.gg/pmef` | **placeholder / unverified** | CI's link checker explicitly excludes `https://discord.gg`; no server referenced anywhere else |
| OpenSSF Best Practices badge | **placeholder** | static shields.io badge linking to the generic site, no project ID |
| Schema CI badge | **false** | points at `pmef/specification` workflow; the workflow exists here but the badge URL does not resolve to this repo |
| `ghcr.io/pmef/pmef-cli:latest` Docker image | **missing** | no Dockerfile, no image build in `release.yml` |
| `cargo install pmef-cli` (crates.io) | **missing** | README itself says "once published"; no publish step anywhere |
| "Bidirectional adapters for 14+ engineering tools across 8 disciplines" | **false** | 10 Rust adapter crates exist, **none compiles** (§4); none implements import back into the tool; `pmef convert` bails with "not yet implemented" |
| "Canonical information model grounded in ISO 15926-14/IDO, DEXPI 2.0, CFIHOS V2.0" | **prose only** | `docs/iso15926-mapping.md` + `catalogs/rdl-uri-map.json`; no machine-checked crosswalks or conversion tests |
| `pmef.net` homepage / schema host | **unverified** | used as `$id` base and `homepage`; link check excludes it |
| Release PDF via `scripts/build-spec-pdf.py` | **missing** | script does not exist (step is `continue-on-error`, so silently skipped) |
| Conformance test suite in `tests/`, `python run_tests.py --level N` (`spec/06-conformance.md` §5.1) | **missing** | no `tests/`, no `run_tests.py` |
| Benchmark DS-05 "EAF Cooling Circuit Segment" (`06-conformance.md` §4.5) | **missing** | no such example |

## 4. Build and test status

### 4.1 Python validation (what CI runs)

All run locally with exit code 0:

| Script | Result |
|---|---|
| `scripts/validate-schemas.py` | 10/10 schemas valid (625 non-strict warnings, visible with `--verbose`) |
| `scripts/check-schema-ids.py` | 10/10 `$id` match file paths |
| `scripts/validate-examples.py` | 5/5 files "valid" — **but see gap below** |
| `scripts/check-ndjson-format.py` | 5/5 OK |
| `scripts/check-topology.py` | 5/5 consistent |
| `scripts/validate-catalogs.py` | 17/17 valid |

**Gap — not all examples are actually validated.** `validate-examples.py` silently returns
"no errors" for any `@type` missing from its `TYPE_TO_SCHEMA` map. Not mapped:
`pmef:SteelMember` (2), `pmef:InstrumentObject` (12), `pmef:InstrumentLoop` (3),
`pmef:PLCObject` (2), `pmef:CableObject` (1), `pmef:CableTrayRun` (1) — **21 of 117 objects
(18 %)**, including Structural Steel, an active-scope discipline. I validated them ad hoc
against `pmef-steel` / `pmef-ei` schemas: they pass today, so this is a CI hole, not a data
defect. Unknown types should fail (or at least be counted), not pass.

Minor: the script uses the deprecated `jsonschema.RefResolver` (removal announced upstream).

### 4.2 Rust (`pmef-rs`) — not covered by any CI workflow

`cargo build --workspace` **fails**. Per crate (`cargo test -p <crate>`):

| Crate | Compiles | Tests |
|---|---|---|
| `pmef-core` | ✅ | 21 pass |
| `pmef-geom` | ✅ | 14 pass |
| `pmef-io` | ✅ | 5 pass |
| `pmef-validate` | ✅ | 15 pass |
| `pmef-cli` | ✅ | 0 tests |
| all 10 `pmef-adapter-*` | ❌ (4–15 errors each) | cannot run (~196 `#[test]`s declared) |

Error categories (representative):

- API drift against `pmef-io`: `serde_json::Error: From<PmefIoError>` not satisfied (e3d, tekla, cadmatic, …).
- Missing dependencies in crate manifests: `tracing` (plant3d), `phf` (tekla).
- Lifetime error in `pmef-adapter-tekla/src/profile_map.rs`.

`pmef validate` on all 5 examples: 0 errors, 0 skipped, "Conformance L1 PASS" (the Rust
validator *does* map steel and E&I types, unlike the Python script).

### 4.3 CLI reality check

| Command | State |
|---|---|
| `validate` | working (schema validation, L1 pass-rate) |
| `stats`, `index`, `check-refs`, `topology` | present (not deeply tested; `pmef-cli` has 0 tests) |
| `diff` | **shallow**: reports added/removed/changed `@id`s only — no field-level diff; iterates a `HashMap`, so **output order is non-deterministic** (conflicts with the "stable diffs" goal) |
| `conformance` | **partial**: 6 structural checks; `--level` and `--dataset` are only echoed into the report, not used; "duplicate `@id`" check cannot fire because the index is a `HashMap` that silently overwrites duplicates |
| `convert` | **stub**: always bails "not yet implemented" |

## 5. Adapter maturity matrix

Scale: **none** (nothing) · **stub** (skeleton / docs only) · **partial** (substantial code, not
usable end-to-end) · **working** (builds, tested, usable). Active scope per CLAUDE.md marked ★.

| Tool | Rust crate | Host-side plug-in | Maturity | Notes |
|---|---|---|---|---|
| ★ AVEVA E3D | `pmef-adapter-e3d` (~2.0 k LOC, 31 tests) | none (PML scripts referenced, not in repo) | **partial, does not compile** | ⚠ contains `rvm.rs`, a parser for the binary **RVM** format — see §6 |
| ★ AutoCAD Plant 3D | `pmef-adapter-plant3d` (~1.6 k LOC, 22 tests: PCF/IDF parsing) | `pmef-plant3d-dotnet` (C#, 442 LOC, Plant SDK) | **partial, does not compile** | PCF is a public text format; the .NET part could not be built here (no .NET SDK / AutoCAD SDK) |
| CADMATIC | `pmef-adapter-cadmatic` (~2.0 k) | — | partial, does not compile | frozen |
| COMOS | `pmef-adapter-comos` (~1.8 k) | `pmef-comos-dotnet` (640 LOC) | partial, does not compile | frozen |
| Creo | `pmef-adapter-creo` (~1.7 k) | `pmef-creo-toolkit` (C, 624 LOC, no build files) | partial, does not compile | frozen |
| Inventor | `pmef-adapter-inventor` (~1.5 k) | `pmef-inventor-addin` (603 LOC) | partial, does not compile | frozen |
| Tekla | `pmef-adapter-tekla` (~1.6 k) | `pmef-tekla-dotnet` (940 LOC) | partial, does not compile | frozen |
| Revit | `pmef-adapter-revit` (~1.2 k) | in `pmef-dotnet-adapters/src/PmefDotNetAdapters.cs` (combined file, "csproj stubs below") | partial, does not compile | frozen |
| Advanced Steel | `pmef-adapter-advanced-steel` (~0.6 k) | same combined file | stub/partial, does not compile | frozen |
| Navisworks | `pmef-adapter-navisworks` (~0.7 k) | same combined file | stub/partial, does not compile | frozen |
| Smart 3D, OpenPlant, EPLAN, INOSIM, CATIA, TIA Portal, SAP EAM, Emulate3D | — | — | **none** | listed in README architecture box |

No adapter implements the import direction (PMEF → tool), so "bidirectional" is not met by any.

## 6. Hard-rule / policy findings (need maintainer decision — nothing changed)

1. **RVM reverse engineering.** `pmef-adapter-e3d/src/rvm.rs` parses the binary RVM format.
   CLAUDE.md forbids reverse engineering RVM without explicit instruction. Provenance of
   that format knowledge is undocumented. Options: (a) remove/feature-gate it, (b) document
   the public source and get explicit sign-off, (c) replace with an E3D-side export via the
   official PML/.NET API.
2. **`@id` pattern conflict — resolved 2026-10-31.** CLAUDE.md said
   `urn:pmef:obj:<project>:<tag>`. Spec 02 §2.1 + spec 01 §7.3 (normative), the `PmefId`
   schema pattern, `pmef-core::PmefId` and all adapters use `urn:pmef:<domain>:<project>:<local-id>`;
   all 117 example `@id`s match the schema pattern. The rule was wrong, not the data:
   CLAUDE.md now states the `<domain>` form. No `@id` was changed.
3. **Example realism.** Examples are labelled synthetic ("PMEF Reference Generator 0.9"),
   but use a real location (`"Duisburg, Germany"`) and real Siemens S7-1500 article
   numbers. Public catalogue data is fine; please confirm none of this mirrors a real
   customer/employer project.
4. **THIRD-PARTY-NOTICES.md** covers standards-derived catalog data but no Rust crate
   dependencies (serde, tokio, jsonschema, quick-xml, reqwest, …).
5. **Scope drift.** Active scope is Piping/Steel/Equipment + E3D/Plant 3D, yet the README,
   spec ch. 01 and the docx advertise 8 disciplines and 14–35+ tools.

## 7. Prioritised fix list

### P1 — credibility and correctness (do first; Phase 2 material)

- **P1.1** Make `cargo build --workspace` green **or** exclude non-scope adapters from the
  workspace (`members`/`exclude`) so core + E3D + Plant 3D build; fix `tracing`/`phf`
  deps and the `PmefIoError` drift.
- **P1.2** Add a Rust CI job (`cargo build`, `cargo test`, `cargo clippy`) — today Rust is not in CI at all.
- **P1.3** Close the Python validation hole: map all steel/E&I types; make unknown `@type` an error
  (or a counted, reported skip that fails CI above zero).
- **P1.4** README: correct repo path, folder names (`catalogs/`, adapter locations), file lists,
  remove/label Discord, OpenSSF, ghcr, crates.io, "14+ adapters", "bidirectional"; add `pmef-rs/`.
- **P1.5** One version: `0.9.0` everywhere (badge, CHANGELOG headings, spec footer); collapse the three
  `[Unreleased]` sections; rename/label the docx as legacy draft (ask before renaming).
- **P1.6** Decide on `rvm.rs` (§6 item 1).

### P2 — make claims true

- **P2.1** Real conformance runner: honour `--level`, detect duplicate `@id`s (reader must not use a
  last-wins map for this check), add `tests/` fixtures or update spec §5.1 to the real command.
- **P2.2** Deterministic `pmef diff` (sorted output) + field-level diff — prerequisite for the
  as-built-vs-design use case.
- **P2.3** Either create DS-05 (`eaf-segment.ndjson`) or remove it from spec/README.
- **P2.4** Fix CHANGELOG so `extract-changelog.py` and `release.yml` can work; drop or implement
   `build-spec-pdf.py`.
- **P2.5** Fix the 625 non-strict schema warnings (at least for steel/E&I `$defs`).
- **P2.6** ~~Correct the `@id` convention in CLAUDE.md~~ — done (§6 item 2).

### P3 — maturity

- **P3.1** Unit tests for `pmef-cli`.
- **P3.2** PMEFX reader/writer in `pmef-io`.
- **P3.3** Migrate `validate-examples.py` from `RefResolver` to `referencing`.
- **P3.4** Rust dependency section in THIRD-PARTY-NOTICES.md; SBOM in releases.
- **P3.5** Machine-readable standards crosswalks with round-trip tests (DEXPI, IFC, CAEX).

## 8. Open questions for the maintainer

- **Q1** Which version should be canonical for Phase 2: `0.9.0` (my recommendation) or jump to
  `1.0.0`? And what is `PMEF_Specification_v1.2.docx` — a v1.0 draft?
- **Q2** Out-of-scope adapters: exclude from the Cargo workspace (keep code, frozen) or fix
  all ten? Recommendation: exclude, fix E3D + Plant 3D only.
- **Q3** `rvm.rs`: remove, feature-gate, or keep with documented source?
- **Q4** ~~`@id` convention~~ — answered: CLAUDE.md follows the spec (§6 item 2).
- **Q5** Is `github.com/MatzUp2022/PMEF` the canonical URL for now, or is a `pmef` org planned?
