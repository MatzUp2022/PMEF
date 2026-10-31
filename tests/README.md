# PMEF Conformance Test Suite

Executable form of [spec 06 — Conformance](../spec/06-conformance.md). It tests an
implementation against the 82 normative test IDs (§5.4) and produces the conformance
report of §7.

```bash
pip install -r requirements.txt

python tests/run_tests.py                          # all tests (level 3), text summary
python tests/run_tests.py --level 1                # level 1 tests only
python tests/run_tests.py --report json --output conformance-report.json
```

Without options the suite tests the built-in **identity** round trip (a pass-through
reference) and the **reference validator** (the repository schemas). That is how CI
self-tests the suite.

## Testing your implementation

| Option | Meaning |
|---|---|
| `--kind validator` | Your tool validates PMEF files. Runs the 58 unit/negative tests only. Result: a validator verdict, not a level (levels require reading **and** writing, CL1-09). |
| `--kind converter` (default) | Your tool reads and writes PMEF. All tests except INTG-006. |
| `--kind adapter` | Your tool exports from an engineering tool. All tests, including INTG-006 (`HasEquivalentIn` for every physical object, CL2-05). |
| `--validator "cmd {file}"` | Validation command; exit code 0 = valid, anything else = invalid. Default: `reference`. |
| `--sut "cmd {input} {output}"` | Round trip PMEF → your tool → PMEF. Must write `{output}`. Default: `identity`. |
| `--name`, `--version`, `--domains` | Written into the report. |

Example — the Rust CLI as a validator (run in CI):

```bash
cargo build -p pmef-cli --manifest-path pmef-rs/Cargo.toml
python tests/run_tests.py --kind validator --name pmef-cli \
  --validator "pmef-rs/target/debug/pmef validate {file} --schemas schemas >/dev/null 2>&1"
```

The exit code is 0 when the requested level (or, for validators, all applicable tests) passes.
The JSON report is checked against [`conformance/report.schema.json`](conformance/report.schema.json).

## What is tested

| Category | IDs | Input | Pass criterion |
|---|---|---|---|
| `unit` | PIPE-U, EQUIP-U, EI-U, STEEL-U, REL-U, GEOM-U | `conformance/fixtures/unit/<ID>.ndjson` | validator accepts the fixture |
| `negative` | 8 of the above (e.g. PIPE-U-003) | fixture with a defect | validator rejects it |
| `integration` | INTG-001 … 006 | SUT output of PMEF-DS-01 | structural rules (header, `@id`, references, revision) |
| `roundtrip` | RT-001 … 018 | SUT output vs. input | attribute fidelity per spec 06 §6 (tolerances §6.2, identity fields §6.3) |

The full catalogue — name, level, requirement (CL*/R-GEN-*), fixture, expectation — is
[`conformance/manifest.json`](conformance/manifest.json). Level 3 also round-trips DS-02, DS-03
and DS-04 and requires one of them to reach 98 % (CL3-03). Performance tests (PERF-*) are not
implemented yet.

## Maintaining the suite

- Fixtures and the manifest are generated:
  `python tests/conformance/tools/generate_fixtures.py` (CI runs it with `--check`).
  Valid fixtures reuse objects from `examples/`; all data is synthetic (CC0).
- `python tests/conformance/tools/selftest.py` proves the suite detects defects: seven
  deliberately broken round-trip implementations (`tools/broken_sut.py`) and an accept-all
  validator must fail the expected tests.
