# RFC-0018: Steel accessories (railings, stairs, ladders, anchors) and object placement

| Field | Value |
|---|---|
| **Status** | `DRAFT` — schema, spec text, Rust model, Plant 3D adapter mapping, example and tests in this repository |
| **Discussion** | — (review issue to be filed; 30-day period from filing) |
| **Target version** | 0.10.0 (additive: one object type, optional fields) |
| **Depends on** | spec 02 §7; spec 04 §2.1; spec 05 §8.2; `pmef-base`, `pmef-steel` schemas; RFC-0013 |
| **Working groups** | WG-Steel, WG-Geometry, WG-Adapters |
| **Date** | 2026-11-28 |

## Abstract

The AutoCAD Plant 3D demo project showed that most of a structure's secondary steel has no PMEF
type: railings, stair treads, stairs, ladders and base-plate anchors were dropped, and plates and
gratings could not be mapped. It also showed that PMEF has no place for an object's installation
frame (insertion point and rotation) or its centre of gravity, which every CAD source carries.
This RFC adds `pmef:SteelAccessory`, `SteelSystem.accessoryIds`, and two optional fields on
`GeometryReference`: `placement` (as IFC `IfcAxis2Placement3D`) and `centerOfGravity`. Plates and
gratings use `pmef:SteelPlate` from RFC-0013.

## Motivation

The demo project's `Piping.dcf` (2,498 objects after this RFC) holds:

- **UC-1 Access steel.** 36 railings, 35 stair treads, 2 caged ladders and 30 anchors. Before this
  RFC they were left out of the conversion. Access routes, escape-route checks, material take-off
  and clash checks against walkways need them.
- **UC-2 Plates and gratings.** 45 gratings and 4 plates. They have no outline in the database,
  only a size text and a centre of gravity, so they need a mapping onto `SteelPlate`.
- **UC-3 Placement.** Every Plant 3D object stores a 4×4 insertion matrix; E3D stores
  `POS`/`ORI`; IFC has `ObjectPlacement`. Today the frame survives only as a vendor-specific
  `customAttribute`, so a writer cannot re-insert the object in the authoring tool and a viewer
  cannot orient a catalog symbol.
- **UC-4 Centre of gravity.** 1,673 objects in the demo carry a COG. Lifting studies, load
  take-off and the pipe-stress interface (eng-calc) need it.

## Detailed Design

### 1. `pmef:SteelAccessory` (spec 02 §7.6)

Secondary steel that is neither a member nor a plate. Required: `@type`, `@id`, `pmefVersion`,
`isPartOf`, `accessoryType`.

| Field | Meaning |
|---|---|
| `accessoryType` | `RAILING`, `STAIR`, `STAIR_TREAD`, `LADDER`, `ANCHOR`, `GENERIC` |
| `designation` | Size or type text from the source, verbatim |
| `path` | System line, ≥ 2 points (mm): top rail of a railing; axis of a stair or ladder from bottom to top |
| `length`, `width`, `height` | Run length, clear width, railing/ladder height or tread thickness (mm) |
| `bottomElevation`, `topElevation` | Lowest and highest point (mm); top **MUST NOT** be below bottom |
| `stepCount`, `hasCage` | Treads or rungs; ladder safety cage |
| `material`, `weight`, `finish` | As on `SteelMember` |

A `SteelSystem` lists its accessories in `accessoryIds`. IFC 4.3: `RAILING` → `IfcRailing`,
`STAIR` → `IfcStair`, `STAIR_TREAD` → `IfcStairFlight`/`IfcMember` (tread), `LADDER` →
`IfcStair` with `PredefinedType` `LADDER`, `ANCHOR` → `IfcMechanicalFastener`
(`ANCHORBOLT`).

### 2. Object placement and centre of gravity (spec 04 §2.4)

`GeometryReference` gains two optional fields:

- `placement` — `{origin, axisZ?, axisX?}`, the object's local frame in project coordinates, as
  `IfcAxis2Placement3D` (Location, Axis, RefDirection). Absent axes are the project axes.
  `axisZ` and `axisX` **MUST** be unit vectors and perpendicular. The placement records how the
  object was inserted; geometry coordinates (ports, outlines, bounding box) stay in project
  coordinates and do not depend on it.
- `centerOfGravity` — a `Coordinate3D` in project coordinates (mm).

Both are valid on every object that has `geometry`, so equipment, piping components, steel and
HVAC objects all use the same fields.

### 3. Plant 3D adapter (spec 05 §8.2)

- `StructureRailing` → `RAILING` (`Length`), `StructureStep` → `STAIR_TREAD`, `StructureStair` →
  `STAIR` (`Length`, `From Height`, `To Height`), `StructureLadder` → `LADDER` (`Height`,
  `Cage`), `StructureAnchor` → `ANCHOR`; `PartSizeLongDesc` → `designation`, `TOS`/`BOS` →
  elevations, `Weight` → `weight`.
- `StructurePlate` → `SteelPlate` `PLATE`, `StructureGrating` → `GRATING`; outline from the size
  text (`L x W x T`), centred on the COG, in the object's placement frame. If the object has no
  rigid placement the outline is laid in the project XY plane and `p3d:outlineFrame: "assumed"`
  is written.
- Every object's insertion matrix → `geometry.placement` when the matrix is rigid (orthonormal);
  otherwise it is kept as `p3d:placement`. `COG X/Y/Z` → `geometry.centerOfGravity`.

## Backwards Compatibility

Additive within 0.10.0. Existing files stay valid; readers that do not know `placement` or
`centerOfGravity` can ignore them.

## Alternatives Considered

- **Accessories as `SteelMember`** — rejected: a railing or ladder is an assembly with a system
  line and type-specific data (height, cage, step count), not a profile swept along an axis.
- **One type per accessory (`pmef:Railing`, `pmef:Ladder`, …)** — rejected: the fields overlap
  almost fully; one type with `accessoryType` keeps queries and adapters simple, as `SteelPlate`
  does with `plateType`.
- **Placement as a 4×4 matrix** — rejected: allows shear and scale, which a placement must not
  have, and differs from IFC; origin plus two axes is unambiguous.
- **Placement on the object instead of on `GeometryReference`** — rejected: it describes where the
  geometry sits; keeping it next to `boundingBox` keeps the object schemas unchanged.

## Open Questions

1. Should stair stringers and landings be separate objects (`IfcStairFlight`, `IfcSlab`) or stay
   inside the `STAIR` accessory? *Proposed:* inside until a source provides them separately.
   *Decision: WG-Steel.*
2. Should `placement` become required for objects that reference a catalog symbol (equipment,
   valves)? *Proposed:* no, recommended (`SHOULD`) in a later version. *Decision: WG-Geometry.*

## Reference Implementation

- Schemas: `pmef-steel` (`SteelAccessory`, `SteelSystem.accessoryIds`), `pmef-base`
  (`Placement`, `GeometryReference.placement`/`centerOfGravity`).
- Rust: `pmef_core::steel::SteelAccessory` (`rise_mm`), `pmef_core::geometry::Placement`
  (`from_matrix`, `to_project`); `pmef-adapter-plant3d` module `pds`.
- Checks: `scripts/check-geometry.py` (placement axes; accessory elevations and path direction),
  negative cases `ACC-*` in `tests/geometry/`.
- Example: `examples/steel-access-placement.ndjson`.
- Measured on the Plant 3D demo project: 103 accessories, 49 plates/gratings, placement on all
  objects with a rigid insertion matrix, COG on 1,673 objects; all objects valid with `--strict`.
