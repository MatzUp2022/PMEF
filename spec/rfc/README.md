# PMEF RFCs

Normative changes to the specification and schemas go through an RFC
([CONTRIBUTING.md](../../CONTRIBUTING.md#specification-change-process-rfc),
[GOVERNANCE.md](../../GOVERNANCE.md)): RFC issue → at least 30 days of discussion → working-group
approval → TSC sign-off → merge with the RFC number in `CHANGELOG.md`.

Status values (issue template): `DRAFT` → `REVIEW` → `APPROVED` / `REJECTED` / `SUPERSEDED`.

| RFC | Title | Target | Status | Issue | Decisions needed |
|---|---|---|---|---|---|
| [0001](0001-provenance-asbuilt.md) | Recognition provenance, review status and as-built data | 0.10.0 | `APPROVED` | [#6](https://github.com/MatzUp2022/PMEF/issues/6) | — (follow-up: 0010) |
| [0002](0002-catalog-snapping.md) | Deterministic catalog snapping `pmef-snap/1` | 0.10.0 | `APPROVED` | [#7](https://github.com/MatzUp2022/PMEF/issues/7) | — (follow-up: 0011) |
| [0003](0003-as-built-correspondence.md) | Persisted as-built ↔ design correspondence | 0.10.0 | `APPROVED` | [#8](https://github.com/MatzUp2022/PMEF/issues/8) | — |
| [0004](0004-polytropic-efficiency-camelcase.md) | `polytropicEfficiency` camelCase spelling | 0.10.0 / 1.0.0 | `APPROVED` | — | — |
| [0005](0005-pmefx-object-index.md) | PMEFX 1.0: object index, mesh LODs, exact B-rep, octree | 0.10.0 | `DRAFT` | [#15](https://github.com/MatzUp2022/PMEF/issues/15) | LOD error metric; ZIP vs. HDF5 criteria (WG-Serialisation) |
| [0006](0006-hvac-ducts-tray-fittings-catalogs.md) | HVAC duct domain, cable tray fittings, support and cable catalogs | 0.10.0 | `DRAFT` | — | Air terminals/AHUs, duct hangers, tray supports (follow-ups) |
| [0007](0007-pyramid-rect-torus-spherical-cap.md) | Primitives PYRAMID, RECT_TORUS, spherical cap and snout shear | 0.10.0 | `DRAFT` | — | RECT_TORUS for duct bends at LOD3 (WG-Geometry) |
| [0008](0008-mesh-ref-gltf-node-fragment.md) | `mesh_ref` glTF node fragments and mesh frame | 0.10.0 | `DRAFT` | — | FileHeader default `meshFrame` (WG-Serialisation) |
| [0009](0009-generic-piping-component.md) | Generic piping component and native type catalog | 0.10.0 | `DRAFT` | — | Type catalogs for Plant 3D and PCF (WG-Adapters) |
| [0010](0010-review-comment-issue-ref.md) | Review comment and issue reference (follow-up of 0001) | 0.10.0 | `DRAFT` | [#44](https://github.com/MatzUp2022/PMEF/issues/44) | `issueRef` single URI vs. array (WG-Piping/WG-Steel) |
| [0011](0011-catalog-snapping-v2.md) | Catalog snapping version 2 `pmef-snap/2` (follow-up of 0002) | 0.10.0 | `DRAFT` | [#45](https://github.com/MatzUp2022/PMEF/issues/45) | Inch-only catalogs; olets/caps/valves (WG-Catalogs, WG-Piping) |
| [0012](0012-placeholder-custom-values.md) | Placeholder and custom values (`UNSPECIFIED`, `CUSTOM`, `placeholders`) | 0.10.0 | `DRAFT` | — | Extend `placeholders` to piping, equipment, steel (WG-Adapters) |
| [0013](0013-steel-plates-project-profiles.md) | Steel plates, project profiles, nested steel systems, reserved volumes | 0.10.0 | `DRAFT` | [#54](https://github.com/MatzUp2022/PMEF/issues/54) | Grating catalogs (WG-Steel) |
| [0014](0014-generic-duct-component.md) | Generic duct component, duct bends without angle, E3D HVAC type catalog | 0.10.0 | `DRAFT` | [#55](https://github.com/MatzUp2022/PMEF/issues/55) | Typed silencer/grille data (WG-HVAC) |
| [0015](0015-cable-tray-route-ports.md) | Cable tray route, ports and source data | 0.10.0 | `DRAFT` | [#56](https://github.com/MatzUp2022/PMEF/issues/56) | Tray supports and network nodes (WG-E&I) |
| [0016](0016-pipe-supports-outside-segments.md) | Pipe supports outside piping segments and support assemblies | 0.10.0 | `DRAFT` | [#53](https://github.com/MatzUp2022/PMEF/issues/53) | Support loads per part vs. per assembly (WG-Piping) |

RFCs 0001–0003 are fully implemented in this repository (schemas, spec text, Rust reference
implementation, examples, tests in CI); each RFC's *Review Status* section lists the evidence.
Review issues were filed on 2026-11-01 (texts: [`docs/rfc-review-2026-11.md`](../../docs/rfc-review-2026-11.md)); the
30-day discussion period would have ended on 2026-12-01; the TSC adopted the proposed resolutions on
2026-11-27 without objections having been raised (section *Resolution* of each RFC).
