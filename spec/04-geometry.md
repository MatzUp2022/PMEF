# PMEF Specification · Chapter 04 · Geometry

**Document number:** PMEF-SPEC-04  
**Version:** 0.9.0-rc  
**Status:** Normative  
**Date:** 2026-03-31

---

## Table of Contents

1. [General](#1-general)
2. [Geometry Layer Architecture](#2-geometry-layer-architecture)
3. [Level of Detail System](#3-level-of-detail-system)
4. [Parametric Primitive Library](#4-parametric-primitive-library)
5. [glTF 2.0 Mesh Layer](#5-gltf-20-mesh-layer)
6. [STEP AP242 B-Rep Layer](#6-step-ap242-b-rep-layer)
7. [OpenUSD Layer](#7-openusd-layer)
8. [Coordinate System](#8-coordinate-system)
9. [Primitive-to-Component Mapping](#9-primitive-to-component-mapping)

---

## 1 General

PMEF geometry is **optional**. A PMEF package that contains only semantic data (no geometry) is valid. The `geometry` field on any PMEF object **MAY** be omitted or set to `{"type": "none"}`.

When geometry is present, PMEF supports three independent geometry layers that can coexist on the same object:

1. **Parametric primitives** — exact analytic shapes, lossless, compact.
2. **glTF 2.0 mesh** — triangulated mesh for web viewers and real-time rendering.
3. **STEP AP242 B-Rep** — precise boundary representation for CAD and FEM.

An optional fourth layer is defined:

4. **OpenUSD** — for simulation, virtual commissioning, and Omniverse integration.

### 1.1 Scope of This Chapter

This chapter defines:

- The `ParametricGeometry` entity type and its 15 primitive subtypes.
- The conventions for referencing glTF 2.0, STEP AP242, and OpenUSD geometry.
- Level of Detail (LOD) definitions.
- The coordinate system and unit conventions.
- Mapping from PMEF entity types to recommended primitives.

---

## 2 Geometry Layer Architecture

Each geometry layer serves different use cases:

| Layer | Format | Primary Use | LOD Range |
|-------|--------|------------|-----------|
| Parametric | PMEF primitives | Clash detection, piping stress, compact exchange | LOD1–LOD4 |
| Mesh (glTF 2.0) | `.glb` / `.gltf` | Web viewer, AR, Navisworks/ACC review | LOD2–LOD3 |
| B-Rep (STEP AP242) | `.stp` / `.step` | CAD round-trip, FEM pre-processing | LOD3–LOD4 |
| Simulation (OpenUSD) | `.usdc` / `.usdz` | Digital twin, Virtual Commissioning, Omniverse | LOD2–LOD3 |

### 2.1 GeometryReference on PMEF Objects

Every PMEF physical object carries a `geometry` field of type `GeometryReference`:

```jsonc
"geometry": {
  "type": "parametric",          // or "mesh_ref", "step_ref", "usd_ref", "none"
  "ref": "urn:pmef:geom:proj:V-201-prim",
  "lod": "LOD2_MEDIUM",
  "boundingBox": {
    "xMin": 11200, "xMax": 11800,
    "yMin": 5100, "yMax": 5700,
    "zMin": 700,  "zMax": 3200
  }
}
```

When `type` is `"parametric"`, `ref` points to the `@id` of a `pmef:ParametricGeometry` object in the same package.

When `type` is `"mesh_ref"`, `"step_ref"`, or `"usd_ref"`, `ref` is a file URI or package-relative path to the external geometry file.

### 2.2 Multiple Geometry Layers per Object

A single PMEF object **MAY** carry geometry in multiple layers. This is expressed by having the primary `geometry` field point to the highest-LOD representation, with additional lower-LOD or alternative-format representations linked via `pmef:ParametricGeometry` objects:

```jsonc
// Primary geometry (LOD3, parametric)
"geometry": {
  "type": "parametric",
  "ref": "urn:pmef:geom:proj:V-201-prim",
  "lod": "LOD3_FINE"
}
// Additionally, the package may contain:
// - mesh_ref: "geometry/model.glb" (LOD2, for web viewer)
// - step_ref: "geometry/vessel.stp" (LOD4, for FEM)
// These are referenced from separate pmef:ParametricGeometry objects.
```

---

## 3 Level of Detail System

PMEF defines five LOD levels:

| Code | Name | Geometry Included | Typical Use |
|------|------|------------------|------------|
| `BBOX_ONLY` | Bounding Box Only | Axis-aligned bounding box | Early design, space reservation |
| `LOD1_COARSE` | Coarse | One primitive per object | Fast clash check |
| `LOD2_MEDIUM` | Medium | Full primitive assembly; major features | Standard plant model, IFC export |
| `LOD3_FINE` | Fine | All flanges, nozzle details, supports | Piping stress, construction model |
| `LOD4_FABRICATION` | Fabrication | Weld preps, bevel angles, all dimensions | Shop drawing, spool fabrication |

**Requirements:**
- The `lod` field on a `GeometryReference` **MUST** accurately represent the detail level of the referenced geometry.
- Implementations **MUST NOT** claim a higher LOD than the actual geometry supports.

---

## 4 Parametric Primitive Library

### 4.1 ParametricGeometry Object

The `pmef:ParametricGeometry` object is a first-class NDJSON object that links a geometry primitive to a PMEF object.

**Required fields:** `@type` (`"pmef:ParametricGeometry"`), `@id`, `ownerRef`, `primitive`

| Field | Description |
|-------|-------------|
| `ownerRef` | `@id` of the PMEF object this geometry belongs to. |
| `lod` | Level of Detail. |
| `primitive` | The geometry primitive (see §4.2). |

### 4.2 Primitive Types

The following 15 primitive types are defined:

#### CYLINDER

Right circular cylinder. Used for pipe runs, vessel shells, nozzle barrels.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"CYLINDER"` |
| `center` | Coordinate3D | Centre of one circular face [mm] |
| `axis` | UnitVector3D | Direction from `center` toward the other face |
| `radius` | number | Outer radius [mm], > 0 |
| `innerRadius` | number \| null | Inner radius [mm] for hollow cylinders (pipe). Null = solid. |
| `length` | number | Length along axis [mm], > 0 |

**Constraint:** When `innerRadius` is provided, `innerRadius` < `radius`.

#### CONE

Truncated cone (frustum). Used for concentric reducers, cone-roof tanks, nozzle necks.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"CONE"` |
| `center` | Coordinate3D | Centre of the `r1` face [mm] |
| `axis` | UnitVector3D | Points from `r1` to `r2` |
| `r1` | number | Radius at start face [mm], ≥ 0 |
| `r2` | number | Radius at end face [mm], ≥ 0 |
| `innerR1` | number \| null | Inner radius at `r1` for hollow |
| `innerR2` | number \| null | Inner radius at `r2` for hollow |
| `length` | number | Axial length [mm], > 0 |

**Constraints:** `r1 ≠ r2` (use CYLINDER for a cylinder); when `innerR1` present, `innerR1 < r1`; when `innerR2` present, `innerR2 < r2`.

#### SPHERE

Full sphere or hemisphere. Used for spherical tanks, ball valve bodies.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"SPHERE"` |
| `center` | Coordinate3D | Centre point [mm] |
| `radius` | number | Radius [mm], > 0 |

#### DISH

Vessel dished head. Used for pressure vessel heads, tank domes.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"DISH"` |
| `center` | Coordinate3D | Centre of the knuckle circle (shell-to-head junction) [mm] |
| `axis` | UnitVector3D | Points outward from the vessel |
| `shellRadius` | number | Shell inside radius [mm], > 0 |
| `dishType` | enum | `HEMISPHERICAL`, `ELLIPTICAL_2:1`, `TORISPHERICAL`, `FLAT`, `CONICAL` |
| `depth` | number | Overall head depth (tangent to crown) [mm] |
| `knuckleRadius` | number \| null | Knuckle radius [mm] for torispherical heads |
| `crownRadius` | number \| null | Crown radius [mm] for torispherical heads |
| `coneAngle` | number \| null | Half-apex angle [°] for conical heads |

For `ELLIPTICAL_2:1` heads, `depth = shellRadius / 2` (implied; may be provided explicitly). For `HEMISPHERICAL`, `depth = shellRadius`.

#### CIRC_TORUS

Circular torus segment. Used for pipe elbows and bends.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"CIRC_TORUS"` |
| `center` | Coordinate3D | Centre of the torus centre circle at the start of the arc [mm] |
| `axis` | UnitVector3D | Normal to the plane of the arc (right-hand rule: thumb points along axis, fingers wrap in arc direction) |
| `startDir` | UnitVector3D | Unit vector from `center` toward the P1 port |
| `torusRadius` | number | Centreline bend radius R [mm], > 0 |
| `tubeRadius` | number | Pipe outside radius r [mm], > 0 |
| `innerTubeRadius` | number \| null | Pipe inside radius for hollow |
| `angleDeg` | number | Bend angle [°], ∈ (0°, 360°] |

**Constraint:** `torusRadius > tubeRadius` (the torus ring does not self-intersect).

#### SNOUT

Eccentric frustum with lateral offset. Used for eccentric reducers, off-centre nozzles.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"SNOUT"` |
| `center` | Coordinate3D | Centre of the `r1` face [mm] |
| `axis` | UnitVector3D | Primary direction from `r1` to `r2` |
| `r1` | number | Radius at start [mm] |
| `r2` | number | Radius at end [mm] |
| `length` | number | Axial length [mm] |
| `offsetX` | number | Lateral offset of r2 centre in local X [mm]. Default 0. |
| `offsetY` | number | Lateral offset of r2 centre in local Y [mm]. Default 0. |

The total lateral offset vector `(offsetX, offsetY)` expresses how the r2 circle centre is shifted relative to the r1 circle centre.

#### BOX

Rectangular cuboid. Used for rectangular tanks, junction boxes, equipment outlines.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"BOX"` |
| `center` | Coordinate3D | Geometric centre of the box [mm] |
| `xLen` | number | Length in X [mm], > 0 |
| `yLen` | number | Length in Y [mm], > 0 |
| `zLen` | number | Length in Z [mm], > 0 |
| `rotationX` | number | Rotation about X axis [°]. Default 0. |
| `rotationY` | number | Rotation about Y axis [°]. Default 0. |
| `rotationZ` | number | Rotation about Z axis [°]. Default 0. |

Rotations are applied in Z → Y → X order (intrinsic Tait-Bryan angles).

#### EXTRUSION

Linear extrusion of a 2D profile. Used for structural steel members.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"EXTRUSION"` |
| `startPt` | Coordinate3D | Extrusion start point [mm] |
| `endPt` | Coordinate3D | Extrusion end point [mm] |
| `profileId` | string | PMEF profile catalog ID, e.g. `"EN:HEA200"` |
| `rotationDeg` | number | Roll rotation of the profile around the extrusion axis [°]. Default 0. |
| `material` | string | Material designation. |

**Constraint:** `startPt ≠ endPt`.

#### REVOLUTION

Surface of revolution. Used for flanges, torispherical heads (alternative), nozzle flanges.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"REVOLUTION"` |
| `axisPoint` | Coordinate3D | A point on the rotation axis [mm] |
| `axis` | UnitVector3D | Rotation axis direction |
| `profile2d` | array of [r, z] | 2D polyline in the (r, z) half-plane. Each item is `[r_mm, z_mm]`. r ≥ 0. |
| `angleDeg` | number | Sweep angle [°]. 360 for full revolution. |

**Constraint:** All `r` values in `profile2d` **MUST** be ≥ 0. `angleDeg` ∈ (0°, 360°].

#### VALVE_BODY

Simplified parametric valve body. Used for valves at LOD2.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"VALVE_BODY"` |
| `valveBodyType` | enum | `GATE`, `GLOBE`, `BALL`, `BUTTERFLY`, `CHECK`, `DIAPHRAGM`, `PLUG`, `CONTROL`, `KNIFE` |
| `center` | Coordinate3D | Centreline midpoint [mm] |
| `direction` | UnitVector3D | Flow axis direction |
| `boreD` | number | Bore diameter [mm], > 0 |
| `faceToFace` | number | Face-to-face length [mm], > 0 |
| `bodyHeight` | number \| null | Overall body + bonnet height [mm] |
| `actuatorHeight` | number \| null | Actuator height above centreline [mm] |
| `actuatorDir` | UnitVector3D | Actuator direction vector |

#### NOZZLE

Parametric nozzle: barrel cylinder + flange disc. Used for equipment nozzles at LOD2/LOD3.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"NOZZLE"` |
| `origin` | Coordinate3D | Face of flange at shell intersection [mm] |
| `axis` | UnitVector3D | Points outward from vessel |
| `nomDN` | number | Nominal bore [mm], > 0 |
| `projection` | number | Distance from shell face to flange face [mm] |
| `flangeOD` | number | Flange outside diameter [mm] |
| `flangeThickness` | number | Flange thickness [mm] |
| `flangeStandard` | string | e.g. `"ANSI_B16.5_150"`, `"EN_1092-1_PN16"` |

#### STEEL_PROFILE

Structural steel member as axis + profile cross-section. Used at LOD2.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"STEEL_PROFILE"` |
| `startPt` | Coordinate3D | Member start node [mm] |
| `endPt` | Coordinate3D | Member end node [mm] |
| `profileId` | string | PMEF profile catalog ID |
| `rollDeg` | number | Roll angle [°]. Default 0. |
| `material` | string | Material designation |
| `grade` | string | Steel grade, e.g. `"S355JR"` |

#### CABLE_TRAY

Cable tray or conduit routing segment. Used for E&I cable routes.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"CABLE_TRAY"` |
| `centerline` | array of Coordinate3D | Ordered list of centreline waypoints [mm]. Minimum 2 points. |
| `width` | number | Tray width [mm], > 0 |
| `height` | number | Tray height [mm], > 0 |
| `trayType` | enum | `LADDER`, `SOLID_BOTTOM`, `PERFORATED`, `WIRE_MESH`, `CONDUIT`, `DUCT`, `UNDERGROUND` |
| `material` | string | Material designation |

#### MESH_REF

Reference to a pre-computed mesh file. Used when no parametric primitive is available or for LOD2 vendor meshes.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"MESH_REF"` |
| `meshUri` | string (URI) | URI to the mesh file |
| `meshFormat` | enum | `GLTF`, `USD`, `STEP_AP242`, `OBJ`, `STL` |
| `featureId` | integer \| null | glTF `EXT_mesh_features` feature ID for semantic annotation. |

#### COMPOSITE

A named collection of primitives forming a compound shape.

| Field | Type | Description |
|-------|------|-------------|
| `type` | const | `"COMPOSITE"` |
| `children` | array | Array of `{partName: string, primitive: AnyPrimitive}` objects. Minimum 1 child. |

---

## 5 glTF 2.0 Mesh Layer

### 5.1 Format Requirements

When a PMEF package includes a glTF geometry file, the following requirements apply:

- The file **MUST** conform to the glTF 2.0 specification (Khronos Group, 2017).
- The binary GLB container format (`.glb`) **SHOULD** be used in preference to the JSON `.gltf` format.
- All geometry **MUST** use the project coordinate system (Z-up; see §8). The glTF Y-up convention requires the application of a +90° rotation about the X axis on load.

### 5.2 Semantic Annotation

PMEF uses the `EXT_mesh_features` glTF extension for semantic annotation. Each mesh feature corresponds to one PMEF object:

- The `featureId` in a `MESH_REF` primitive **MUST** match the feature index in the glTF `EXT_mesh_features` extension.
- The feature table **MUST** include a `pmefId` property containing the `@id` of the corresponding PMEF object.

Example glTF feature table:

```json
{
  "EXT_mesh_features": {
    "featureIds": [
      {"featureCount": 1250, "attribute": 0}
    ],
    "schema": {
      "classes": {
        "pmefObject": {
          "properties": {
            "pmefId": {"type": "STRING"},
            "pmefType": {"type": "STRING"}
          }
        }
      }
    }
  }
}
```

### 5.3 Coordinate Conversion

PMEF (Z-up, mm) → glTF (Y-up, m) conversion:
- Divide all coordinates by 1000 (mm → m).
- Apply rotation matrix `Rx(+90°)`: swap Y and Z, negate new Z.
- `pmef(x, y, z)_mm → gltf(x/1000, z/1000, -y/1000)_m`

---

## 6 STEP AP242 B-Rep Layer

### 6.1 Format Requirements

When a PMEF package includes STEP AP242 geometry:

- The file **MUST** conform to ISO 10303-242:2022 (AP242 Managed Model-Based 3D Engineering).
- The application protocol subset **SHOULD** be `AP242 AIC 219` (Mechanical Design) for piping and equipment geometry.
- All dimensions **MUST** be in millimetres. The STEP file header **MUST** declare `LENGTH_UNIT(.,MILLI.,0.001)`.

### 6.2 Object Identity in STEP

The PMEF `@id` **MUST** be recorded in the STEP file as a `PRODUCT_DEFINITION_CONTEXT` descriptor or equivalent user-defined attribute, enabling round-trip identification of STEP entities from PMEF objects.

### 6.3 Recommended STEP Use Cases

- Flanged joint assemblies (flange + gasket + bolt set) at LOD4.
- Pressure vessel shells and heads at LOD4 (for FEM pre-processing).
- Equipment vendor geometry when the vendor provides STEP B-Rep.
- Pipe spools for fabrication drawing generation.

---

## 7 OpenUSD Layer

### 7.1 Overview

OpenUSD (Universal Scene Description, Alliance for OpenUSD) is supported as a fourth geometry layer for simulation and Digital Twin applications. The OpenUSD layer enables integration with:

- NVIDIA Omniverse (industrial Digital Twin platform).
- Emulate3D Factory Test (virtual commissioning with OpenUSD).
- Siemens Plant Simulation X (planned Omniverse integration).

### 7.2 PMEF Custom USD Schema

PMEF defines a custom USD schema namespace `pmef:` for annotating USD prims with PMEF semantics:

```
// PMEF USD Schema (pmef-schema.usda)
class PMEFObject "PMEFObject" {
    string pmef:id = ""           // @id URI
    string pmef:type = ""         // @type string
    string pmef:tagNumber = ""    // equipment/instrument tag
    string pmef:changeState = "WIP"
    string pmef:revisionId = ""
}
```

Every PMEF USD prim **MUST** have a `PMEFObject` applied schema with the `pmef:id` and `pmef:type` attributes populated.

### 7.3 Scene Structure in USD

The USD scene hierarchy mirrors the PMEF plant hierarchy:

```
/Plant
  /Unit_U100 {pmef:type = "pmef:Unit", pmef:id = "urn:pmef:unit:..."}
    /Equipment
      /P_201A {pmef:type = "pmef:Pump", pmef:id = "urn:pmef:obj:..."}
        /Body
        /Suction_Nozzle
        /Discharge_Nozzle
    /Piping
      /CW_201 {pmef:type = "pmef:PipingNetworkSystem", ...}
```

### 7.4 Physics Schema

For simulation applications, PMEF USD prims **SHOULD** include USD Physics schema attributes:

- `physicsCollisionEnabled = true` for equipment and structures.
- `physicsMassAPI` with `mass` attribute for equipment objects.
- `physicsRigidBodyAPI` for moving parts (valve actuators, conveyor belts).

---

## 8 Coordinate System

### 8.1 PMEF Project Coordinate System

All PMEF 3D coordinates are expressed in the project coordinate system:

```
Z (Up / Elevation, positive upward)
│
│    Y (North, or project North)
│   /
│  /
│ /
└──────────────── X (East, or project East)
```

- **Handedness:** Right-handed.
- **Unit:** Millimetres (mm) for all lengths.
- **Angle unit:** Degrees (°) for all angles.
- **Origin:** Project datum point (defined per project).

### 8.2 Plant North

Plant North is typically aligned with the Y-axis. When Plant North differs from geographic North, a rotation angle **SHOULD** be declared in the `pmef:Plant` object via `customAttributes.plantNorthAngleDeg`.

### 8.3 Georeferencing

When PMEF coordinates are referenced to a geographic coordinate system:

- The EPSG code of the projected coordinate system **SHOULD** be declared in `pmef:Plant.epsgCode`.
- The origin of the PMEF coordinate system in the projected CRS **SHOULD** be declared in `pmef:Plant.geoOrigin` as `{easting_m, northing_m, elevation_m}`.

---

## 9 Primitive-to-Component Mapping

The following table defines the **RECOMMENDED** primitives for each PMEF component type and LOD level:

| Component | LOD1 | LOD2 | LOD3 | LOD4 |
|-----------|------|------|------|------|
| `pmef:Pipe` | CYLINDER (bbox) | CYLINDER (hollow) | CYLINDER (hollow) | CYLINDER + REVOLUTION (bevels) |
| `pmef:Elbow` | BOX | CIRC_TORUS | CIRC_TORUS | CIRC_TORUS + REVOLUTION (ends) |
| `pmef:Tee` | BOX | COMPOSITE (3×CYLINDER) | COMPOSITE | COMPOSITE |
| `pmef:Reducer` (concentric) | BOX | CONE (hollow) | CONE (hollow) | CONE + REVOLUTION |
| `pmef:Reducer` (eccentric) | BOX | SNOUT | SNOUT | SNOUT + REVOLUTION |
| `pmef:Flange` | CYLINDER | REVOLUTION | REVOLUTION | REVOLUTION (all rings) |
| `pmef:Valve` | BOX | VALVE_BODY | VALVE_BODY | MESH_REF (vendor mesh) |
| `pmef:Olet` | CYLINDER | COMPOSITE | COMPOSITE | COMPOSITE |
| `pmef:Gasket` | (none) | CYLINDER (thin disk) | REVOLUTION | REVOLUTION |
| `pmef:PipeSupport` (resting) | BOX | BOX | COMPOSITE | COMPOSITE |
| `pmef:PipeSupport` (spring hanger) | BOX | CYLINDER (rod) | COMPOSITE | COMPOSITE |
| `pmef:Vessel` (shell) | CYLINDER | COMPOSITE (cyl+dishes) | COMPOSITE | COMPOSITE |
| `pmef:Tank` (vertical) | CYLINDER | COMPOSITE (cyl+roof) | COMPOSITE | COMPOSITE |
| `pmef:Pump` | BOX | COMPOSITE+MESH_REF | MESH_REF (vendor) | MESH_REF |
| `pmef:HeatExchanger` | BOX | COMPOSITE (shell+channel) | COMPOSITE | COMPOSITE |
| `pmef:Column` | CYLINDER | COMPOSITE | COMPOSITE | COMPOSITE |
| `Nozzle` | (none) | NOZZLE | NOZZLE | NOZZLE + REVOLUTION (flange) |
| `pmef:SteelMember` | CYLINDER | STEEL_PROFILE | EXTRUSION | EXTRUSION + details |
| `pmef:CableTrayRun` | BOX | CABLE_TRAY | CABLE_TRAY | CABLE_TRAY |

### 9.1 Composite Assembly Convention

For COMPOSITE primitives, the `partName` field in each child **SHOULD** follow this naming convention:

| Part | `partName` |
|------|-----------|
| Main shell / body | `"shell"` or `"body"` |
| Top head / dome | `"top_head"` |
| Bottom head | `"bottom_head"` |
| Nozzle (by ID) | `"nozzle_<nozzleId>"` |
| Support leg (by number) | `"leg_<N>"` |
| Skirt | `"skirt"` |
| Actuator | `"actuator"` |
| Handwheel | `"handwheel"` |
| Insulation (outer) | `"insulation"` |

---

*End of Chapter 04.*

**[← Chapter 03](03-serialisation.md)** · **[Chapter 05 — Adapters →](05-adapters.md)**
