# RFC-0007: Primitives PYRAMID, RECT_TORUS, spherical cap and snout shear

| Field | Value |
|---|---|
| **Status** | `DRAFT` — reference implementation, schema, example and tests in this repository |
| **Discussion** | issue opened with the implementing pull request, 30-day period |
| **Target version** | 0.10.0 (additive: two new primitive types, one new enum value, new optional fields) |
| **Depends on** | spec 04 §4.2 (primitive library), DD-011 (E3D geometry via PML) |
| **Working groups** | WG-Geometry |
| **Date** | 2026-11-27 |

## Abstract

A test with an offshore plant model (piping of one module, exported from AVEVA E3D) mapped its
design primitives to PMEF. About 95 % mapped directly. Three shapes did not: pyramids (valve
actuators, steel base plates), rectangular tori, and spherical caps, which had to be written as
`TORISPHERICAL` heads with knuckle radius 0. This RFC adds `PYRAMID`, `RECT_TORUS`,
`DISH.dishType = SPHERICAL_CAP`, and optional shear angles on `SNOUT`, and it defines the local
frame that SNOUT and PYRAMID offsets refer to.

## Motivation

- **No object may lose its shape.** A primitive without a PMEF counterpart is either dropped or
  replaced by a bounding box; both break clash checks and as-built comparison.
- **Semantics matter.** A torispherical head with knuckle 0 is mathematically a spherical cap,
  but a consumer that tessellates or checks heads by type gets it wrong.
- **The plant design tools have these shapes.** E3D and PDMS offer `PYRA`, `RTOR`, sheared
  `SNOU`; their parameters map one to one.
- **"Local X" was undefined.** `SNOUT.offsetX/offsetY` referred to a local frame that the
  specification never defined.

## Detailed Design

Normative text: spec 04 §4.2 (`PYRAMID`, `RECT_TORUS`, `DISH`, `SNOUT`) and §4.3; schema
`pmef-geometry.schema.json`.

1. **`PYRAMID`** — rectangular frustum: `center` (bottom face), `axis` (bottom → top),
   optional `refDirection`, `bottomX/Y`, `topX/Y` (≥ 0), `offsetX/Y` (top face centre),
   `height` (> 0). At least one face has an area.
2. **`RECT_TORUS`** — same placement as `CIRC_TORUS` (`center` on the axis of revolution,
   `axis` with the right-hand rule, `startDir`), cross-section `innerRadius` < `outerRadius`
   and `height` centred on `center`, `angleDeg` ∈ (0, 360].
3. **`DISH.dishType = SPHERICAL_CAP`** — `shellRadius` = base circle radius, `depth` required
   and ≤ `shellRadius`. A cap must not be written as torispherical with knuckle 0.
4. **`SNOUT` shear** — `bottomShearX/Y`, `topShearX/Y` in degrees ∈ (−90, 90): a point at local
   `(x, y)` on an end face moves along `axis` by `x·tan(shearX) + y·tan(shearY)`.
5. **Local frame (§4.3)** — local Z = `axis`; local X = `refDirection` (new, optional on SNOUT
   and PYRAMID) projected perpendicular to `axis`, else the projection of project X, else
   project Y when `axis` ∥ X.

## Compatibility

Additive. Existing files stay valid; the default local frame only fixes what SNOUT offsets
already meant in the reference implementation. A reader of 0.9 that meets an unknown primitive
type fails schema validation for that object only.

## Reference Implementation

- `pmef-adapter-e3d`: PML elements `PYRA` and `RTOR` (with `XDIR`) map to `PYRAMID` and
  `RECT_TORUS`; degenerate primitives are reported, not dropped silently.
- `pmef-route`: conservative obstacle boxes for both new types.
- `scripts/check-geometry.py` rule 12; negative cases in `tests/geometry/`.
- Example `compressor-train.ndjson` (PYRAMID governor housing); conformance tests
  `GEOM-U-009…013`.

## Open Questions

- Should `RECT_TORUS` also cover rectangular duct bends (`pmef:DuctBend`) at LOD3, replacing
  `COMPOSITE` approximations? (WG-Geometry with WG-Information-Model.)
- Shear on `CYLINDER` (sloped cut pipe ends) is not part of this RFC.
