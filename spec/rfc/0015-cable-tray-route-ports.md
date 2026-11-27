# RFC-0015: Cable tray route, ports and source data

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust model, geometry check, example and tests in this repository |
| **Discussion** | issue opened with the implementing pull request, 30-day period |
| **Target version** | 0.10.0 (additive: one type, optional fields on two types, `width`/`height` no longer assumed present) |
| **Depends on** | spec 02 §2.6, §2.10, §6.6–§6.8; RFC-0001 (provenance), RFC-0006 (tray fittings), RFC-0012 (placeholders) |
| **Working groups** | WG-E&I, WG-Adapters |
| **Date** | 2026-11-27 |

## Abstract

`CableTrayRun` has no route: no start or end point, no path and no ports. In the Huldra test
case (4,405 runs, about 2,800 fittings from E3D) the route survives only inside a
`ParametricGeometry` box, so `connectedTrayIds` cannot be checked geometrically and cable
routing (spec 02 §6.7) has no lengths. Source attributes (E3D spec, source name) have no place,
and `height`, which the facet geometry shows for only 465 of 4,405 runs, cannot be marked as a
stand-in. This RFC adds a centreline, ports, `nativeType`, `customAttributes` and
`placeholders` to runs and fittings, and documents how `trayType` is derived from project naming
(issue [#56](https://github.com/MatzUp2022/PMEF/issues/56)).

## Motivation

- **Topology from data, not from meshes.** A route that lives only in a geometry primitive is
  lost when a consumer reads another LOD or a mesh; joins cannot be checked.
- **Cable lengths.** Cable routing over trays needs run lengths.
- **No silent loss.** Spec names and source names are needed for round trips and for review.
- **No guessing.** A default height or a type derived from a zone name must be recognisable.

## Detailed Design

Normative text: spec 02 §6.8 (and §2.10, §6.7); schema `pmef-ei.schema.json`.

1. New `$defs/TrayPort`: `portId`, `coordinate` (required), `direction`, `width`, `height`,
   `connectedTo` (PmefId or null). Coordinates are the centre of the tray bottom.
2. `CableTrayRun`: optional `startPoint`, `endPoint`, `path` (≥ 2 points), `ports`,
   `nativeType`, `customAttributes`, `placeholders`. Rules: `startPoint`/`endPoint` equal the
   first/last `path` point; ports lie at a centreline end and point away from the run.
3. `CableTrayFitting`: optional `ports`, `nativeType`, `customAttributes`, `placeholders`.
   Rules: port `connectedTo` targets = `connectedTrayIds`; bend angle from port directions
   agrees with `angle` (1°).
4. `trayType` convention: the zone or spec name is recorded in `customAttributes`; a type
   derived from a naming rule is listed in `placeholders` (`TO_BE_CONFIRMED`); without a rule
   `trayType` is omitted.

```json
{"@type":"pmef:CableTrayRun","@id":"urn:pmef:obj:demo-tray:CT-302","trayMark":"CT-302","width":400,"height":100,
 "path":[[6500,500,3000],[6500,4000,3000],[6500,7500,2800]],
 "ports":[{"portId":"P1","coordinate":[6500,500,3000],"direction":[0,-1,0],"connectedTo":"urn:pmef:obj:demo-tray:CT-301-B01"}, …],
 "nativeType":"E3D:PIPE","customAttributes":{"e3dSpec":"/OE100SS","zone":"/LV-LADDER-U300"},
 "placeholders":[{"path":"/height","reason":"SOURCE_MISSING"}], …}
```

## Alternatives considered

- **Route only in geometry (status quo).** Ties topology to one representation; see Motivation.
- **Reuse `DuctPort`.** Its `crossSection` carries round and flat-oval shapes that trays never
  have; a flat width × height is simpler and matches `CableTrayFitting.width`/`height`.
- **Separate `CableTrayNetwork` with nodes.** Larger change; runs and fittings with shared port
  coordinates already form the network, as in piping and HVAC.

## Compatibility

Additive: all new fields are optional and existing files stay valid. The Rust model makes
`trayType`, `width` and `height` of `CableTrayRun` optional, matching the schema (they were never
required there).

## Review Status

| Item | Evidence |
|---|---|
| Schema | `schemas/pmef-ei.schema.json` (`TrayPort`, `CableTrayRun`, `CableTrayFitting`) |
| Spec | spec 02 §6.8 (new), §6.7, §2.10 |
| Rust | `pmef-core/src/ei.rs` (`TrayPort`, `CableTrayRun::centreline`/`length_mm`, fitting fields); `CableTrayRun` leaves the frozen list in `schema_drift.rs` |
| Checks | `scripts/check-geometry.py` (`check_trays`); negative cases `CT-*` in `tests/geometry/equipment-steel-negative.ndjson` |
| Example | `examples/cable-tray-route.ndjson` |
| Conformance | `EI-U-011` … `EI-U-014` |
