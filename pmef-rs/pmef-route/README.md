# pmef-route

Graph and rule kernel for automatic pipe routing. Lines are routed in a rectilinear voxel graph
around obstacles, with piping-class rules from the PMEF catalogs (outside diameter, elbow
centre-to-face, class pressure/temperature envelope), and written as PMEF `Pipe`/`Elbow`
components with `provenance.sourceType: generated`.

Design, algorithm and roadmap: [`docs/auto-routing.md`](../../docs/auto-routing.md).

```bash
cargo run -p pmef-cli -- route ../tests/route/pump-skid-route.json -o /tmp/routed.ndjson --report /tmp/route.json
```

Minimal problem:

```json
{
  "project": "demo", "revisionId": "r2026-11-25-001", "parent": "urn:pmef:unit:demo:U1",
  "grid": { "step": 100, "margin": 2000 },
  "settings": { "bendPenalty": 1000, "clearance": 50 },
  "pmefScene": ["plant.ndjson"],
  "obstacles": [{ "id": "walls", "kind": "soft", "obj": "walls.obj" }],
  "zones": [{ "aabb": { "min": [0, 0, 4000], "max": [20000, 1500, 4600] }, "factor": 0.5 }],
  "routes": [{
    "id": "CW-101", "pipeClass": "A1A2", "dn": 100,
    "from": { "nozzle": "urn:pmef:obj:demo:P-101#N2" },
    "to": { "point": [15000, 800, 4300], "direction": [-1, 0, 0] }
  }]
}
```
