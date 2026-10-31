# CLAUDE.md — PMEF (Plant Model Exchange Format)

## What this repo is

Open, cross-discipline 3D plant model exchange format plus reference implementation.

- `spec/` — normative specification (Markdown). **Single source of truth.**
- `schemas/` — JSON Schema Draft 2020-12. Normative together with `spec/`.
- `examples/` — annotated NDJSON reference instances (CC0).
- `catalogs/` — open piping class and steel profile catalogs.
- `pmef-rs/` — Rust core library + CLI (Apache 2.0).
- `pmef-*-dotnet`, `pmef-*-addin`, `pmef-creo-toolkit` — tool adapters.
- `*.docx` in the repo root are NON-normative legacy artefacts. Never treat them as source of truth.
  If content conflicts, `spec/` + `schemas/` win; flag the conflict.

## Strategic context

PMEF is the backbone data model for a recognition product that builds plant models from
2D drawings, isometrics, DWG/DXF/DGN/PCF/PDF and point clouds/scans, and derives standard-compliant
2D drawings from the model. Implications for every design decision:

- Every object may originate from automated recognition → provenance, confidence and review
  status must be representable.
- As-built vs. design comparison is a core use case → object-level diffs must stay stable
  (stable `@id`s, deterministic NDJSON ordering).
- Exactness comes from catalog snapping (profiles, DN/PN, piping classes), not raw geometry.

## Scope (current focus)

Active: Piping, Structural Steel, Equipment. Adapters: AVEVA E3D and AutoCAD Plant 3D only.
All other disciplines/adapters are frozen: do not extend them unless explicitly asked.

## Conventions

- IDs: `urn:pmef:<domain>:<project>:<local-id>` (spec 02 §2.1, schema `pmef-base` `PmefId`).
  `<domain>` per spec 01 §7.3: `obj` (physical object: equipment, piping component, instrument),
  `line`, `seg`, `spool`, `loop`, `geom`, `rel`, `plant`, `unit`, `pkg`, `doc`, `functional`, `catalog`.
  Never reuse or change an existing `@id`; adding a new domain segment is a spec change.
- Units: SI, lengths in mm, angles in degrees. No implicit units.
- Versioning: SemVer for spec and schemas. Every schema change → CHANGELOG.md entry.
- Every schema change requires: updated example in `examples/`, a passing validation test,
  and matching Rust types in `pmef-rs` (serde round-trip test).
- New optional fields only within a minor version; breaking changes need an RFC (issue template).
- Standards alignment must be kept: ISO 15926-14/IDO, DEXPI 2.0, CFIHOS V2.0, IEC 81346,
  IFC 4.3, AutomationML/CAEX (IEC 62424), AAS (IEC 63278), ISO 19650.

## Commands

Verified 2026-10-31 (see `docs/AUDIT.md`). Do not invent commands.

```bash
# Python checks (exactly what .github/workflows/validate-schemas.yml runs)
pip install -r requirements.txt           # jsonschema, check-jsonschema
python scripts/validate-schemas.py        # schemas are valid Draft 2020-12 (--verbose for warnings)
python scripts/check-schema-ids.py        # $id matches file path
python scripts/validate-examples.py       # examples/*.ndjson against schemas
python scripts/check-ndjson-format.py     # one object per line
python scripts/check-topology.py          # connectivity integrity
python scripts/validate-catalogs.py       # catalogs/*.json against pmef-catalog schema

# Rust reference implementation (exactly what .github/workflows/rust.yml runs)
cd pmef-rs
cargo build --workspace --all-targets
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets    # informational in CI (doc-comment warnings)
cargo run -p pmef-cli -- validate ../examples/pump-skid-complete.ndjson --schemas ../schemas
cargo run -p pmef-cli -- diff ../tests/diff/design.ndjson ../tests/diff/asbuilt.ndjson --ignore revision --ignore-recognition
cargo run -p pmef-cli -- diff ../tests/diff/design.ndjson ../tests/diff/asbuilt-scan.ndjson --ignore revision --ignore-recognition --match
cargo run -p pmef-cli -- conformance ../examples/pump-skid-complete.ndjson   # shallow structural check only
cd ..

# Conformance suite (spec 06; details in tests/README.md)
python tests/run_tests.py --level 3                       # reference self-run, must reach level 3
python tests/conformance/tools/generate_fixtures.py       # regenerate fixtures after schema changes
python tests/conformance/tools/generate_fixtures.py --check
python tests/conformance/tools/selftest.py                # broken implementations must be detected
python tests/snapping/reference_snap.py                   # pmef-snap/1 test vectors (RFC-0002)
python scripts/generate-asbuilt-example.py examples/asbuilt-recognition.ndjson   # after catalog/algorithm changes
```

CI also runs Mermaid validation (`mmdc` via `scripts/extract-mermaid.py`), markdownlint
(`.markdownlint.jsonc`) and lychee link checking. A schema change that affects a tested type
needs regenerated fixtures (`generate_fixtures.py`) — CI fails otherwise.

## Hard rules

- No proprietary customer or employer project data in examples, catalogs or tests.
  Examples must be synthetic.
- No copying of vendor-proprietary format documentation. No reverse engineering of closed
  formats (RVM, Dabacon, VUE, 3DDX) without explicit instruction; adapters use official public APIs.
- Keep THIRD-PARTY-NOTICES.md current when adding dependencies. Licences: spec CC BY 4.0,
  code Apache 2.0, examples CC0.
- Work on a feature branch per task; small, reviewable commits; never push to `main` directly.
- Stop and ask before deleting files, renaming schemas, or changing `@id` patterns.
