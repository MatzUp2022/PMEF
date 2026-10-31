#!/usr/bin/env python3
"""Self-test of the conformance suite: it must pass a correct implementation and
detect deliberately broken ones. Run in CI.

    python tests/conformance/tools/selftest.py
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
RUN = [sys.executable, "-W", "ignore", str(ROOT / "tests" / "run_tests.py")]
BROKEN = f"{sys.executable} {HERE / 'broken_sut.py'}"

# mode -> test IDs that MUST fail
EXPECT = {
    "drop": {"RT-001", "RT-018"},
    "pressure": {"RT-008"},
    "custom": {"RT-016", "RT-017"},
    "crash-unknown": {"RT-016", "RT-017"},
    "revision": {"RT-005", "RT-018"},
    "coords": {"RT-009"},
    "header-last": {"INTG-001"},
}
NEGATIVE = {"PIPE-U-003", "PIPE-U-015", "EQUIP-U-011", "EQUIP-U-012", "EI-U-003", "STEEL-U-005", "REL-U-008",
            "GEOM-U-008"}


def run(*args: str) -> tuple[int, dict]:
    with tempfile.NamedTemporaryFile(suffix=".json") as f:
        code = subprocess.run([*RUN, "--report", "json", "--output", f.name, *args]).returncode
        return code, json.loads(Path(f.name).read_text())


def failed(report: dict) -> set[str]:
    return {t["id"] for t in report["tests"] if t["status"] == "FAIL"}


def main() -> int:
    problems = []
    code, rep = run("--level", "3")
    if code != 0 or rep["conformanceLevel"] != 3 or failed(rep):
        problems.append(f"identity SUT must reach level 3 (got {rep['conformanceLevel']}, failed {failed(rep)})")
    for mode, must_fail in EXPECT.items():
        code, rep = run("--level", "3", "--sut", f"{BROKEN} {mode} {{input}} {{output}}")
        missing = must_fail - failed(rep)
        if code == 0 or missing:
            problems.append(f"broken SUT '{mode}' not detected (exit {code}, missing failures {sorted(missing)})")
        else:
            print(f"✓ broken SUT '{mode}' detected: {sorted(failed(rep))}")
    code, rep = run("--kind", "validator", "--validator", "true")
    if code == 0 or failed(rep) != NEGATIVE:
        problems.append(f"accept-all validator must fail exactly the negative tests, got {sorted(failed(rep))}")
    else:
        print("✓ accept-all validator fails exactly the 8 negative tests")
    for p in problems:
        print(f"✗ {p}")
    print("self-test", "FAILED" if problems else "passed")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
