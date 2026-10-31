#!/bin/sh
# Round-trip SUT for the conformance suite: PMEF -> PCF (Plant 3D import) -> PMEF (update of the
# input model from the PCF). Usage via tests/run_tests.py:
#   python tests/run_tests.py --kind converter --domains piping --name pmef-adapter-plant3d \
#     --sut "sh tests/conformance/suts/plant3d-pcf.sh {input} {output}"
# Set PMEF to the pmef binary (default: pmef-rs/target/debug/pmef). To test *through* AutoCAD
# Plant 3D, import the PCF files written to $PCF_DIR with PCFIMPORT, re-export them as PCF into
# the same directory, and run the second command manually.
set -e
PMEF="${PMEF:-pmef-rs/target/debug/pmef}"
PCF_DIR="$(mktemp -d)"
"$PMEF" convert "$1" --to pcf -o "$PCF_DIR" 2>/dev/null
"$PMEF" convert "$PCF_DIR" --to pmef --base "$1" -o "$2" 2>/dev/null
rm -rf "$PCF_DIR"
