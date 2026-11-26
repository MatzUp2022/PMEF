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

`ifc2obstacles.py` merges all hard elements into one mesh (`duplex-hard`, `clinic-hard`), so the
terminals name it as `owner`: boiler outlets and fixtures are part of that mesh. Only the terminal
stub may enter its clearance zone; earlier lines still block the stub, which is why outlets on the
same heater are spaced and ordered so that no line passes over another line's outlet.

Screenshots: `python route2glb.py /tmp/duplex /tmp/duplex/report.json duplex-route.glb --soft`, then
render with plant-test-corpus `render/shot.js` using `view-route.html` (soft obstacles transparent).

Coarse-to-fine (stage 1): add `"coarseStep": 500` (Duplex) or `600` (Clinic) to `grid` in the
problem file. The route report notes which graph found each route; results are in
`docs/auto-routing.md`.

## Reference runs: routes against the modelled pipes

`reference_runs.py` takes pipe runs that the corpus models already contain and routes them again
between the same end points, with the run itself removed from the obstacles:

```bash
pip install ifcopenshell trimesh numpy scipy rtree matplotlib
python ifc2obstacles.py /tmp/ref/duplex-arch $C/Duplex_A_20110907.ifc   # building only (hard + soft)
python reference_runs.py extract /tmp/ref/duplex $C/Duplex_Plumbing_20121113.ifc /tmp/ref/duplex-arch --max 12
python reference_runs.py compare /tmp/ref/duplex ../../../pmef-rs/target/release/pmef
```

`extract` follows `IfcRelConnectsPorts` from fitting to fitting and keeps runs made of
axis-parallel segments (pipe axes from the segment geometry, corners where consecutive axes meet).
Per run it writes `R##/problem.json` (DN from the `Dimensions.Size` property, class A1A2, the
other plumbing as hard obstacles, walls and slabs as soft) and `reference.json` (polyline,
length, bends). `compare` routes each run twice, `default` (walls and slabs soft) and `chases`
(no soft obstacles, as if the pipes were allowed in wall chases), and writes `summary.json`,
`R##/compare.png` (plan and elevation, reference blue) and `overview.png`. Metrics: length ratio
to the reference, bends, clash (length closer to other geometry than the pipe radius, 150 mm at
each end ignored), length inside soft elements, mean and maximum deviation from the reference.

## DEXPI line lists

`dexpi_layout.py` places the equipment of a `dexpi-pmef` package (e.g. the DEXPI reference P&ID
C01, dumped by the DEXPI-Manager test `cargo test -p dexpi-pmef --test mapping` with
`DEXPI_PMEF_DUMP=<dir>`) as boxes on a 6 m grid and writes a problem with `lineLists`:

```bash
python dexpi_layout.py /tmp/ref/dexpi/c01.ndjson /tmp/ref/c01 --class-map 75HB13=A1A2 73HG12=A1A2
../../../pmef-rs/target/release/pmef route /tmp/ref/c01/problem.json -o /tmp/ref/c01/routed.ndjson --report /tmp/ref/c01/report.json
```

The layout is synthetic; the run checks the line-list path (matching, DN, class mapping, skipped
lines), not routing quality.
