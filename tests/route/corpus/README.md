# Auto-routing against plant-test-corpus

Not run in CI (needs the corpus IFC files and IfcOpenShell). Reproduces the Duplex and Clinic
results in `docs/auto-routing.md`.

```bash
# corpus files: plant-test-corpus `python3 fetch.py --set ifc --yes --dest $PLANT_TEST_CORPUS`
pip install ifcopenshell trimesh numpy scipy
C=$PLANT_TEST_CORPUS/ifc
python ifc2obstacles.py /tmp/duplex $C/Duplex_A_20110907.ifc $C/Duplex_Plumbing_20121113.ifc
cp duplex-problem.json /tmp/duplex/problem.json
(cd ../../../pmef-rs && cargo build --release -p pmef-cli)
../../../pmef-rs/target/release/pmef route /tmp/duplex/problem.json -o /tmp/duplex/routed.ndjson --report /tmp/duplex/report.json
python ifc2obstacles.py /tmp/clinic $C/Clinic_Structural.ifc $C/Clinic_Plumbing.ifc   # ~4 min
```

Screenshots: `python route2glb.py /tmp/duplex /tmp/duplex/report.json duplex-route.glb --soft`, then
render with plant-test-corpus `render/shot.js` using `view-route.html` (soft obstacles transparent).
