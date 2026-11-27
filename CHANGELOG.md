# CHANGELOG

All notable changes to the PMEF Specification are documented here.
Adheres to [Semantic Versioning](https://semver.org/) and [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

**Version identifiers.** The data/spec version is plain SemVer (`pmefVersion: "0.9.0"`, schema
`$id` path `/0.9/`). Pre-release labels such as `-rc` mark *releases* (Git tags, e.g.
`v0.9.0-rc`) and never appear in data. The next schema extension is `0.10.0`.

---

## [Unreleased]

### Added — pipe supports outside piping segments (RFC-0016, #53)

- **`PipeSupport`**: `isPartOf` may name the unit/area of a support zone or the assembly
  `PipeSupport` (hangers, clamps, shoes); new optional `supportedIds` for the carried pipes,
  segments or lines (equivalent to `pmef:Supports`). Spec 02 §9.11, §4.4, §8.3.
- **`native-types-e3d` 2026.2**: E3D `REST` (assembly), `HANG`, `PCLA`, `SHU` → `pmef:PipeSupport`.
- Rust `PipeSupport.supported_ids`; **fixed** `SupportType` enum to match the schema
  (`HANGER`, `CLAMP`, … were rejected before).
- Example `examples/pipe-support-assembly.ndjson`; conformance `PIPE-U-019` … `PIPE-U-021`.

### Added — steel plates, project profiles, nested steel systems (RFC-0013, #54)

- **`pmef:SteelPlate`**: plate, grating, chequered plate or panel as an outline in a local plane
  (`origin`, `normal`, `refDirection`, `outline`, `holes`) with `thickness` and `plateType`
  (spec 02 §7.4). `SteelSystem.plateIds`; Rust `SteelPlate` with area and volume.
- **Nested steel systems**: `SteelSystem.isPartOf` may name a parent `SteelSystem`; new
  `systemLevel` `STRUCTURE`/`FRAMEWORK`/`SUBFRAMEWORK` (spec 02 §7.5).
- **Project profiles**: catalog field `scope` (`SHARED`/`PROJECT`); `<PROJECT>:<designation>`
  resolved in the package's project catalogs first (spec 02 §7.1, spec 05 §7.2 replaces
  `CUSTOM:`). `check-geometry.py` reads project catalogs from `catalogs/` next to the file;
  `validate-catalogs.py` also checks `examples/catalogs/`.
- **`profiles-en` 2024.4**: round bars `EN:RB10`–`EN:RB60` (EN 10060); snapping vectors and
  `asbuilt-recognition` example pin the new version.
- **Reserved volumes**: `equipmentClass` `OBSTRUCTION_VOLUME` and `ACCESS_ROUTE` for
  `GenericEquipment` (spec 02 §5.3).
- **E3D adapter**: STRU/FRMW/SBFR → nested `SteelSystem`, SCTN (`POSS`/`POSE`/`BANG`/`SPRE`) →
  `SteelMember`, PANE (PLOO/PAVE) → `SteelPlate` (spec 05 §8.1).
  Steel `@id`s come from the element name; a name already issued to another DB address
  (same last name under different parents) falls back to the full address.
- **`pmef-route`**: `SteelPlate`s are structural obstacles (box over the extruded outline).
- Rust `SteelMaterial`: `fy`/`fu` optional, `density` added, as in the schema.
- Example `examples/steel-platform-plates.ndjson` with project catalog
  `examples/catalogs/profiles-demo-project.json`; plate rules in `check-geometry.py`
  (`tests/geometry` cases `PL-*`).

### Added — cable tray route, ports and source data (RFC-0015, #56)

- **`CableTrayRun`**: optional centreline (`startPoint`/`endPoint` or `path` polyline, tray-bottom
  centre), `ports` (new `TrayPort`: coordinate, direction, width, height, `connectedTo`),
  `nativeType`, `customAttributes`, `placeholders` (RFC-0012). The E&I tray run is no longer frozen.
- **`CableTrayFitting`**: optional `ports`, `nativeType`, `customAttributes`, `placeholders`.
- Spec 02 §6.8: route and port rules, convention for `trayType` derived from zone/spec names.
- `scripts/check-geometry.py`: path vs start/end, ports at centreline ends and pointing outward,
  port width, fitting ports vs `connectedTrayIds`, bend angle vs port directions.
- Rust `pmef-core`: `TrayPort`, `CableTrayRun::centreline()`/`length_mm()`; `trayType`, `width`,
  `height` optional as in the schema.
- Example `examples/cable-tray-route.ndjson`; conformance `EI-U-011` … `EI-U-014`.

### Fixed — steel bounds for rolled and skewed members

- **`pmef_geom::member_aabb`** with **`SectionSize`**: bounds of a straight member that enclose
  the section for any roll. Along a world axis the rolled rectangle's larger perpendicular extent
  (`max(h, b)` at 0° and 90°, unchanged), otherwise the half-diagonal; round sections their radius.
  Before, `STEEL_PROFILE` primitives (`rollDeg`) and routing obstacles of steel members
  (`rollAngle`) always used half the larger dimension, so a member rolled by 45° or a skewed
  brace could leave flange corners outside the obstacle (Codex review on #62).
- `schema_primitive_aabb` takes a `SectionSize` callback instead of a half dimension; `pmef-route`
  supplies it from the profile catalogs (flat bars: `b × t`). Routing examples are byte-identical
  (all members there are axis-aligned with roll 0).

### Changed — routing obstacles from `pmef-geom`

- **`pmef_geom::schema_primitive_aabb`**: conservative bounds of a primitive given as schema JSON
  (all spec 04 types, `STEEL_PROFILE` with a profile-size callback, `VALVE_BODY`, `CABLE_TRAY`,
  `COMPOSITE`; `MESH_REF` → none). Moved from `pmef-route`, which now delegates to it, so the
  routing kernel and the geometry crate share one implementation.
- **`pmef-route`**: `STEEL_PROFILE`, `VALVE_BODY` and `CABLE_TRAY` primitives in
  `ParametricGeometry` now block routes (before they were ignored). `examples/pump-skid-auto-routed.ndjson`
  and the line-list run are byte-identical; the WASM build grows by about 15 kB.
- `pmef-geom` drops its unused `nalgebra`, `serde` and `thiserror` dependencies and uses plain
  `serde_json` (no `preserve_order` forced on consumers of `pmef-route`).

### Added — PCF to PMEF as WASM

- **`pmef-plant3d-wasm`**: one call, `pcf_to_pmef(pcf)`, runs the Plant 3D adapter in the
  browser (about 0.7 MB). Web apps that only need PCF isometrics as PMEF, such as the
  ProDocAimAi material takeoff, no longer load a full converter.

### Changed — E3D attachment points and the dished-head geometry rule (#53)

- **`catalogs/native-types-e3d.json`:** E3D `ATTA` (attachment point on a branch) maps to
  `pmef:PipeSupport` / `PIPE_SUPPORT` instead of falling back to `GenericPipingComponent/SPECIAL`
  with a warning. Found in the Huldra test case: 4,358 attachments, about 75 % tagged as pipe
  supports. 39 entries.
- **`scripts/check-geometry.py`:** the rule "DISH must sit on a cylinder end face" now applies only
  to `pmef:Vessel`, `Tank`, `Column`, `Reactor` and `HeatExchanger`. Catalogue bodies of piping
  components use dishes differently (E3D valve bodies: two dishes back to back across the bonnet),
  which produced false errors. The negative fixture gains such a valve (`VALV-DISH`), which must
  stay clean.

### Added — placeholders on piping components (RFC-0012 amendment)

- **`PipingComponent.placeholders`** (optional): the RFC-0012 stand-in list is now available on
  all piping components. A valve of unknown type stays `componentClass: VALVE_UNSPECIFIED`
  (RFC-0009) and **SHOULD** list `/componentSpec/componentClass` (`SOURCE_MISSING`, or
  `NOT_IN_ENUM` with the real type in `note`). Spec 02 §2.10, RFC-0012 amendment; example
  `examples/piping-valve-placeholder.ndjson`; Rust field `placeholders` on every piping
  component; `tests/placeholders/run.py` with three more negative cases.

### Added — CAEX XML serialisation (AutomationML)

- **`pmef-io::caex`** (feature `caex`, on by default): `write_caex` / `read_caex` implement the
  CAEX 3.0 secondary serialisation of spec 03 §4 (AutomationML 2.10). Lossless round trip for all
  `examples/*.ndjson`; foreign AutomationML files are read as far as they map.
- **`pmef convert <file.ndjson> --to aml`** and **`pmef convert <in.aml> --to pmef`** (no `--base`
  needed for AML).
- Spec 03 §4.3 describes the encoding of objects, arrays, `null`, `isPartOf` and relationships;
  §4.4 flags the unit-suffix rule as an open point (names stay unchanged in the reference
  implementation).

### Added — bonding jumpers on flanged joints

- **`Gasket.bondingJumper`** (optional boolean): the flanged joint of the gasket is bridged by a
  bonding jumper (earthing cable) for electrical continuity, e.g. in hazardous areas or with
  insulating gaskets. Isometric generators show the jumper and list it in the bill of materials.
  Example: `examples/pump-skid-complete.ndjson` (gasket `CW-202-GK-001`); Rust field
  `Gasket.bonding_jumper`.

### Added — RFC-0012 placeholder and custom values

- **`placeholders`** on duct components and **`$defs/Placeholder`** (`path`, `reason`, `note`,
  `expectedFrom`): marks stand-in values that a later revision of the same `@id` replaces
  (spec 02 §2.10, [RFC-0012](spec/rfc/0012-placeholder-custom-values.md)).
- **`damperType` `UNSPECIFIED` and `CUSTOM`**, new field **`damperTypeCustom`**; schema rules:
  `UNSPECIFIED` needs a `/damperType` placeholder, `CUSTOM` ⇔ `damperTypeCustom`.
- `pmef check` lists open placeholders (information, never a failure).
- Example `examples/hvac-damper-placeholder.ndjson`, test `tests/placeholders/run.py`.

### Deprecated

- **`CompressorSpec.polytropic_efficiency`** — use `polytropicEfficiency`
  ([RFC-0004](spec/rfc/0004-polytropic-efficiency-camelcase.md)). Both spellings are valid in
  0.10.x, but not together in one object; the old one is removed in 1.0.0. The Rust model reads
  both and writes camelCase; `examples/compressor-train.ndjson` uses the new spelling.

### Removed

- **Benchmark PMEF-DS-05 "EAF Cooling Circuit Segment"** from spec 06 §4.5 and the ROADMAP — the
  file `examples/eaf-segment.ndjson` never existed (docs/AUDIT.md P2.3). CL3-03 now lists DS-02,
  DS-03 or DS-04.

- **Seven entries that are not AISC designations** from `profiles-aisc` (`HSS1.5x0.125`,
  `HSS2.0x0.125`, `HSS4.5x0.25`, `HSS5.5x0.25`, `HSS8.0x0.25`, `HSS9.0x0.375`) and
  `profiles-aisc-extended` (`WT18x150`; nearest is WT18x151). Not referenced anywhere in the
  repository (docs/AUDIT.md P3.12).

- **RVM parser removed from `pmef-adapter-e3d`** (`src/rvm.rs`, `E3DConfig::rvm_export_path`,
  `byteorder` dependency). AVEVA's RVM format is not publicly specified; the reference
  implementation does not reverse-engineer it. See DD-011 in `docs/design-decisions.md`.
  RVM-only models are planned via an external RVM→glTF conversion + `mesh_ref` (ROADMAP).

### Fixed

- **Plant 3D PCF bridge keeps what a material takeoff snaps against.** `write_pcf` writes
  `PIPING-SPEC` (line `pipeClass`) and `ITEM-CODE` (`catalogRef.catalogId`); `export_pcf_to_pmef`
  reads them back and keeps a PMEF object id from `UNIQUE-COMPONENT-IDENTIFIER`, so
  PMEF → PCF → PMEF keeps pipe class, catalog codes and stable `@id`s. Port topology no longer
  drops the link in one direction where two components meet (one coordinate index entry per point
  kept only the last component).
- **Plant 3D PCF import: generated ids no longer collide with kept PMEF ids.** Kept ids from
  `UNIQUE-COMPONENT-IDENTIFIER` are reserved first; a component without one (e.g. added in
  Plant 3D) gets the next free `<line>-<class>-NNN`.
- **Plant 3D PCF import: imperial units.** `UNITS-BORE INCH` (also `IN`, `INCHES`) is recognised,
  `UNITS-CO-ORDS` is read on its own (bore in inches with co-ordinates in mm works), and a
  standard NPS bore becomes its DN (8 → 200) instead of 203.2 mm, so catalog snapping matches.
- **`boltDiameter_mm` in the piping classes is now the bolt, not the bolt hole.** All four classes
  (A1A2, B3C1, C5D1, EN-P1A) carried the hole diameter or a mix (88 of 89 flange entries, e.g.
  A1A2 DN200 CL150: 22 → 19.1 mm = 3/4"). The values now come from `flange-dimensions*.json`, and
  `scripts/check-catalogs.py` checks them against the flange catalogs. Bolting BOMs (stud length,
  thread) derive from this field.
- **0.9.0-rc known issues closed or restated** (ROADMAP backlog). EQUIP-U-005 and PIPE-U-003 are
  obsolete (`vesselSubtype` has `KO_DRUM`; both IDs now name other, passing conformance tests);
  the Rust `jsonschema` integration is done; DS-05 was removed; the `pmef-geom` AABB item now names
  the primitive types that still have no bounds. The 0.9.0-rc table below is left as released.

- **Elbow `CW-201-ELB-001` in `examples/pump-skid-complete.ndjson` swept the wrong way.** Its
  CIRC_TORUS had `axis: [0,1,0]`; by the right-hand rule of spec 04 the arc then ends at
  x = 10890.4 instead of at port P2 (x = 11500). The spec is right, the example was wrong:
  `axis` is now `[0,-1,0]`. Fixture GEOM-U-003 and `examples/pmefx/` are regenerated.
  `scripts/check-geometry.py` now checks that an elbow's CIRC_TORUS runs from one port to the
  other (new case `ELB-SENSE` in `tests/geometry/`).

- **`pmef-core` no longer enables serde_json `preserve_order`** — same fix as for `pmef-catalogs`:
  consumers outside this workspace (DEXPI-Manager's DEXPI→PMEF mapping) keep plain JSON maps.
  Inside the workspace `pmef-io`/`pmef-cli` still enable it, so output order is unchanged.
  No API change.

- **`pmef-catalogs` no longer enables serde_json `preserve_order`** — the crate took serde_json
  from the workspace, so Cargo feature unification switched every consumer (e.g. DEXPI-Manager)
  to order-preserving JSON maps. It now depends on plain `serde_json`. No API change.

- **PMEF-DS-04 (`ei-loop-fic-10101`) had no FileHeader** although spec 03 requires one as the
  first object; added (10 objects).

- **Validators agree (docs/AUDIT.md P3.8).** `pmef validate`, `stats`, `index`, `check-refs`,
  `conformance` and `topology` silently dropped lines without a string `@id`/`@type` or with
  invalid JSON, so an invalid file could pass; these lines are now errors. The Rust validator no
  longer asserts `format` (annotation, as in Draft 2020-12 and the Python reference). Unknown
  `@type` is skipped with a warning in both validators (spec 06 CL1-03); `pmef validate --strict`
  and `scripts/validate-examples.py` (default; `--lenient` to relax) report it as an error.

- **Flange catalogs corrected** (docs/AUDIT.md P3.12): `flange-dimensions` 2024.2 against the
  ASME B16.5 inch tables — Class 900/1500/2500 rows held values of other classes (e.g. Class 900
  NPS 1–2 had Class 600 drilling), flange thickness is tf without the raised face (Class 150/300
  values included 1/16 in); `flange-dimensions-en` 2024.2 against the EN 1092-1 type 11 tables —
  PN40 DN200–600 held PN25 values, raised-face diameters were ASME values, thicknesses C1.
  `boltDiameter_mm` now means the bolt diameter in both catalogs; the hole is in the new optional
  field `boltHoleDiameter_mm` (field meanings in each catalog's `fieldDefinitions`). Piping classes
  A1A2/B3C1/C5D1/EN-P1A take the corrected flange dimensions; snapping vectors and
  `asbuilt-recognition` re-recorded (catalog version only).

- **Catalog data corrected** (docs/AUDIT.md P3.12; versions bumped, notes in each catalog):
  `profiles-aisc` 2023.2 and `profiles-aisc-extended` 2024.2 against the AISC Shapes Database
  v16.0 (angle masses ≈ 38 % low, W36x150/W40x167/W44x230 flanges and weak-axis properties,
  HSS weights, WT Ix/Sx); `profiles-en` 2024.3 (UPE series, RHS 100x60); `profiles-en-extended`
  2024.2 (T sections to EN 10055); `pipe-dimensions` 2024.2 (schedules to ASME B36.10M: SCH10,
  NPS 14–24 shifted by one schedule, SCH120/SCH160/XXS swapped for NPS 3–6, NPS 12 SCH40);
  piping classes A1A2/B3C1/C5D1 2024.2 take pipe walls and flange dimensions from the dimension
  catalogs. Snapping vectors re-recorded (catalog versions); SNAP-010/011 use h = 252 mm so the
  W10 ambiguity case stays ambiguous with the corrected W10x33 depth. Conformance fixture and
  `asbuilt-recognition` example regenerated (catalog version only).

- **Benchmark equipment consistent with the piping (docs/AUDIT.md P3.11)** — `@id`s and object
  counts unchanged:
  - PMEF-DS-01 (`pump-skid-complete`): P-201A moved so its suction meets the free end of CW-201
    (CW-201 unchanged, it is shared with `stress-model-cw201`); discharge line CW-202, its support
    beam STEEL-BM-C02 and V-201 shifted by −1700 mm in x; V-201 now sits above the pump with its
    bottom inlet on CW-202's flange end, top head on the shell end (spec 04).
    `connectedPortId` now names real port IDs (`P1`/`P2`) instead of `P-first`/`P-last`.
  - PMEF-DS-02 (`heat-exchanger-station`): CW-301 ran 1.1 m into the E-301A shell; re-laid as a
    vertical control-valve station onto a radial shell nozzle (same components and lengths);
    channels flush with the shell; E-301A/B nozzles on the body surfaces; E-301B's LOD1 box is
    E-301A's envelope.
  - `compressor-train`: K-101 nozzles were inside its bounding box.
  Conformance fixtures regenerated; PCF round-trip test updated (CW-202 start x 10200 → 8500).

- **`compressor-train` equipment geometry**: V-101 shell `center` was the shell mid-point instead
  of the bottom face centre (spec 04), so the heads floated inside the shell; nozzles of V-101 and
  E-101 lay inside the bodies (outlet and vent both on the top centreline). Nozzles moved onto the
  surfaces, E-101 shell nozzles made radial, channels flush with the shell. Conformance fixtures
  regenerated.

- **Rust model matches the schemas (docs/AUDIT.md P3.9).** `pmef-core` types for the active scope
  (hierarchy, piping, equipment, steel, relationships) now accept every schema-valid object and
  round-trip it without loss: fields required in Rust but optional in the schema became `Option`
  (e.g. `PipeSupport.supportsMark`, `Pipe.pipeLength`, the `PumpSpec`/`VesselDesign`/`hxSpec`
  members), missing schema properties were added, and wrong JSON names were fixed —
  `Iec81346Designation` (`functionalAspect` etc. were silently dropped), `GeometryReference.ref`,
  `SteelMember.teklaGUID`, `CatalogReference.eclassIRDI`, `CompressorSpec.polytropic_efficiency`.
  New test `pmef-cli/tests/schema_drift.rs` guards this against every reference instance.

- **E3D adapter output is schema-valid (docs/AUDIT.md P1.7).** `pmef-adapter-e3d` no longer
  writes `null` for absent PML attributes, puts equipment design data into the required
  type-specific blocks (`vesselDesign`, `pumpSpec`, `compressorSpec`, `hxSpec`, tank fields)
  and always sets `nozzleMark`. In-line piping components (flanges, valves, supports, tees,
  reducers, …) were all exported as `pmef:Pipe` (enum name passed instead of the PML keyword);
  they now get their proper `@type`. New integration test `tests/output_validates.rs` validates
  every exported object against `schemas/`.

- **Benchmark geometry made consistent** (new CI check `scripts/check-geometry.py`: bend centre
  points, port directions per spec 02 §2.6, pipe lengths). PMEF-DS-01 / stress model: elbow
  CW-201-ELB-001 had both ports on the same vertical with no intersecting axes — it is now a real
  90° LR DN200 bend (centre-to-end 304.8 mm around the original corner at x = 11500, z = 850):
  CW-201-PIPE-001 ends at x = 11195.2 (length 2195.2 mm, weight 75.0 kg), CW-201-PIPE-002 starts at
  z = 1154.8 (795.2 mm, 27.8 kg), welds W01/W02 moved accordingly, elbow torus geometry, spool
  length/weight updated. Inward-pointing port directions fixed on CW-201-PIPE-002, -FL-001,
  -GK-001, -FL-002 and on PMEF-DS-02 CW-301-FV-001, -PIPE-002; weld directions `[0,0,0]` replaced
  by the pipe axis. No `@id` changed. Spec 06 §4.1 object count of DS-01 corrected (35, not 16).
- **`catalogs/profiles-en.json` HEA series corrected (catalog version 2024.1 → 2024.2).** All 24
  HEA entries listed the nominal size as height (HEA 300: 300 instead of 290 mm), HEA 900/1000
  had wrong flange thicknesses (28 instead of 30/31 mm), HEA 100 was listed as `EN:HEA96`, and
  `Iy`, `Iz`, `Wel_y`, `Wel_z` were wrong by a factor of up to 15. Fixed: h and tf per the EN 10365
  dimension tables (the corrected h also follows independently from the catalog's own,
  correct cross-section areas); `EN:HEA96` → `EN:HEA100` (entry id change); section properties
  recomputed from the dimensions with `scripts/i-section-properties.py`, which reproduces all
  HEB and IPE entries within 0.31 % and the published HEA properties within 1 %. CI now checks
  every I/H section's properties against its dimensions (±1 %). Snapping test vectors updated
  (only `catalogVersion` changed) plus SNAP-017: HEA 300 and HEB 300 are now distinguishable by
  h and b.
- **Conformance fixtures no longer pick up 0.10.0 feature examples by accident.** The
  generator took the first object of a type from all examples, so after RFC-0001 several
  Level-1 fixtures (PIPE-U-002/004/005/016, STEEL-U-002/005) silently used objects from
  `asbuilt-recognition.ndjson`. They now come from the 0.9.0 benchmark datasets only.
- **`validate-examples.py` silently skipped 21 of 117 example objects** (all structural-steel
  and E&I types) because its hand-written type list was incomplete, and unknown `@type`s passed.
  The `@type` → schema map is now derived from the schemas themselves; unknown types are errors.
  A Rust test (`pmef-validate/tests/type_map_matches_schemas.rs`) fails if `schema_for_type`
  drifts from the schemas.
- **The whole Rust workspace builds again** (`cargo build --workspace`): all ten
  `pmef-adapter-*` crates failed to compile. `pmef-io` now implements
  `From<PmefIoError> for AdapterError` (used by all adapters); missing `tracing` dependency
  (Plant 3D); Tekla profile override table no longer needs the undeclared `phf` crate;
  lifetime and `&mut self` fixes (Plant 3D, Tekla, CADMATIC); missing test imports
  (Creo, Inventor). All 259 workspace tests pass.
- **Creo STEP parser read digits inside entity names as coordinates**
  (`CARTESIAN_POINT('NOZZLE_N1',(200,…))` yielded `1.0, 200, 400`). Quoted names are now skipped.
- **Plant 3D PCF: blind/slip-on/socket-weld flanges were never detected.** The flange type was
  read from SKEY characters 1–2 (always `FL`) instead of 3–4 (`FLBL`, `FLSO`, …).
- **CADMATIC test expected a wrong value** for 15 barg → Pa(abs): correct is 1 601 325 Pa
  (the implementation was right).
- **E3D PML parser swallowed attributes after reference values.** Any attribute whose value
  starts with `/` (e.g. `SPEC /SPEC-A1A2`, `CREF /PIPE-1`) was treated as a new element,
  so all following attributes were attached to it. Only structural and component keywords
  now start elements.
- **`pmef validate` crashed on every real dataset.** `PmefValidator` compiled
  each `$defs` sub-schema in isolation with no document store, so cross-file
  `$ref`s (`pmef-base.schema.json`, `pmef-property-sets.schema.json`,
  `#/$defs/EquipmentObject`) could not resolve; `jsonschema` then fell back to
  fetching schemas over HTTP, which panicked inside the async runtime. The
  validator now loads every `*.schema.json` in the directory, indexes them by
  `$id`, and validates via an absolute `$ref` root so all references resolve
  locally. `pmef validate examples/*.ndjson` now runs to completion.
- **`FileHeader` schema drifted from the spec.** `pmef-hierarchy.schema.json`
  required `createdAt` (a PMEFX container-manifest field, not a FileHeader
  field per spec §3.1) and defined `projectId` instead of `projectCode`, so
  every benchmark `FileHeader` failed validation — hidden because
  `validate-examples.py` skipped hierarchy objects. Aligned the schema to
  spec §3.1: `required` is now `@type`/`@id`/`pmefVersion`/`plantId`, and the
  field is `projectCode`. Added a top-level `oneOf` so the hierarchy schema
  validates its objects like the piping/equipment schemas do.
- **`validate-examples.py` now validates `FileHeader`/`Plant`/`Unit`/`Area`**
  against `pmef-hierarchy.schema.json` instead of skipping them, so CI catches
  hierarchy schema/data divergence.
- **Relationship objects were never validated** (both tools). `schema_for_type`
  (Rust) and `TYPE_TO_SCHEMA` (Python) listed relationship type names that do
  not exist in `pmef-relationships.schema.json` (`pmef:ConnectedTo`,
  `pmef:HasPort`, `pmef:FlowsTo`, …) and omitted the real ones, so every
  relationship object was silently skipped. Both maps now use the schema's
  actual relationship types (`IsConnectedTo`, `IsDerivedFrom`, `Supports`,
  `ControlledBy`, `HasEquivalentIn`, `IsRevisionOf`, …).
- **`pmef validate` conformance pass rate** now excludes skipped objects.
  A type the validator has no schema for is a tool gap, not a data defect, and
  no longer drags the pass rate below the conformance threshold. With the
  relationship fix, all five benchmark datasets now report 0 errors, 0 skipped,
  and Conformance L1 PASS.

### Changed

- **RFC-0001 to RFC-0003 `APPROVED`** (TSC decision 2026-11-27, before the end of the discussion
  period of #6–#8 on 2026-12-01; no objections) with the proposed resolutions: schema `$id`s keep
  `/0.9/` for all 0.x releases; `review.comment` / `issueRef` and `pmef-snap/2` move to follow-up
  RFCs (0010, 0011); the `pmef-snap/1` ambiguity margin is part of the algorithm identity. spec 03
  §8.4 gains one sentence: an `IsRevisionOf` with `changeType` `AS_BUILT` is a correspondence, not a
  content revision of a delta package. No schema, code or example change.

- **RFC-0001 to RFC-0003 moved to `REVIEW`** — verified against the implementation; open questions
  answered or given a proposed resolution; each RFC has a *Review Status* section. New RFC index
  `spec/rfc/README.md` and ready-to-file issue texts `docs/rfc-review-2026-11.md`. RFC-0004 status
  spelled `APPROVED` (template vocabulary).

- **`pmef check` replaces `pmef conformance`** (docs/AUDIT.md P2.1). The old command claimed to
  run the conformance suite but was a shallow package check that ignored `--level`, `--json` and
  `--dataset` and could not detect duplicate `@id`s. `pmef check` checks a package's structure
  (one FileHeader first, every line readable, `@type`/`@id` present, unique `@id`, `connectedTo`
  resolves unless `urn:pmef:external:`; layer references such as `isPartOf` → unit are reported
  only) as text or `--json`; `pmef conformance` remains as a deprecated alias with JSON output.
  Spec 06 §7.1: the conformance report comes from the suite (`tests/run_tests.py --report json`).

- **`pmef-core` API (Rust):** helper methods on fields that are optional in the schema now return
  `Option<f64>`: `PipingDesignConditions::design_temperature_celsius`/`design_pressure_barg`,
  `PipingSpecification::bore_mm`/`effective_wall_mm`, `HeatExchangerSpec::lmtd_counter_current`.
  The corresponding struct fields are `Option<_>`. No workspace caller was affected.

- **`pmef convert` implemented** (was a stub): `--to pcf|pmef`, `--base`, `-o`; the unused
  `--from` and `--schemas` options were removed.
- **Conformance suite: INTG-006 checks only objects of the claimed `--domains`** (spec 06 CL3-05:
  "every *exported* object"); before, an adapter exporting only piping failed for equipment and steel.
- **`pmef diff` CLI options changed:** `--changed-only` removed (unchanged objects are never
  listed); new `--exact`, `--ignore`, `--ignore-recognition`. The command now exits with 1 when
  the files differ and 2 on errors.
- **README and contributor docs match the repository.** Repo URL `MatzUp2022/PMEF` instead of
  the placeholder `pmef/specification` (README, spec 01, Cargo.toml, CONTRIBUTING, getting
  started, issue templates); real folder names and file lists; placeholder badges and links
  removed (Discord, OpenSSF, ghcr.io image, crates.io install); "14+ bidirectional adapters"
  replaced by an implementation-status table; CLI install from source.
- **`PMEF_Specification_v1.2.docx` renamed to `PMEF_Specification_v1.0-draft_DE.docx`** — its
  title page says v1.0 draft; it is a non-normative legacy artefact.
- **Rust CI** (`.github/workflows/rust.yml`): builds and tests the whole `pmef-rs` workspace
  and runs `pmef validate` on every example; clippy runs informationally. The release workflow
  now requires it.
- **`pmef-adapter-e3d`: equipment geometry from PML design primitives.** `BOX`, `CYLI`,
  `CONE`, `SNOU` and `DISH` children of an `EQUI` are mapped to `pmef:ParametricGeometry`
  (single primitive or `COMPOSITE`) and referenced from the equipment via
  `geometry: {type: "parametric", ref}`. Invalid primitives are reported via
  `E3DAdapter::warnings()` instead of being dropped silently. Spec 05 §8.1 updated.
- **`pmef-adapter-e3d` compiles again** (writer errors mapped to `AdapterError::Other`).

- **Catalog schema (`schemas/pmef-catalog.schema.json`)**: tightened from a
  loose envelope into a per-`catalogType` contract. Each `entries` item is now
  validated against a type-specific `$def` (steel profile, piping class, pipe
  dimension, flange dimension, instrument, material grade, RDL mapping), with
  required identifier/core fields enforced per type. Added `RDL_MAPPING` and
  `PIPE_STRESS_MAPPING` to the `catalogType` enum; `pressureClass` now accepts
  numeric ASME classes and EN `PN` strings.
- Reclassified `catalogs/rdl-uri-map.json` from `INSTRUMENT_CATALOG` to
  `RDL_MAPPING` to match its actual content.

### Added

- **`pmef-geom` bounds for the schema compound types**: `PrimitiveType::SteelProfile`
  (startPt→endPt inflated by the section half-diagonal, any roll), `ValveBody` (face-to-face
  cylinder with flange-OD envelope, else `VALVE_ENVELOPE_FACTOR` × boreD, plus bonnet/actuator)
  and `CableTray` (polyline capsule); `aabb_union` for `COMPOSITE`. `MESH_REF` stays without
  bounds (no extent in the schema). New `PrimitiveParams` fields: `end_point`, `points`,
  `actuator_axis`, `actuator_height`.

- **`profiles-aisc-extended` 2024.3: millimetre dimensions** for C, MC and WT (`d_mm`, `bf_mm`,
  `tw_mm`, `tf_mm` = inch value × 25.4, half-up to 0.1 mm; inch values unchanged and
  authoritative). AISC channels can now be snapped with `pmef-snap/2` (RFC-0011 open question 1,
  vector SNAP2-027). `check-catalogs.py` checks `_mm` against `_in` (new case in `tests/catalogs/`).

- **Catalog snapping `pmef-snap/2`** ([RFC-0011](spec/rfc/0011-catalog-snapping-v2.md), spec 02
  §2.9.4). New kinds `channel`, `rect-hollow`, `round-hollow`, `angle`, `elbow`, `tee`, `reducer`,
  `flange`; selectors `angle` and `boltCount`; fitting ODs from the class's pipes where a class
  stores none. `pipe`/`i-section` results equal `pmef-snap/1`. Python reference and Rust
  `pmef_core::snapping::{snap_v2, snap_with, Algorithm}` agree on 27 new vectors
  (`tests/snapping/vectors-v2.json`); `pmef-snap/1` vectors and examples unchanged. No schema change.

- **Review comment and issue reference** ([RFC-0010](spec/rfc/0010-review-comment-issue-ref.md),
  spec 02 §2.9.2). `review` gains optional `comment` (1–2000 characters; SHOULD for `rejected`)
  and `issueRef` (absolute URI, e.g. the BCF API URL of a topic). Rust `ReviewStatus.comment` /
  `issue_ref`; `examples/asbuilt-recognition.ndjson` regenerated (two objects carry the new
  fields); negative cases in `pmef-validate/tests/provenance.rs`.

- **E3D adapter: ports, connectivity and implicit tubes** (spec 05 §8.1). Arrive/leave/branch
  p-points of the PML export (`APOS`/`LPOS`/`P3POS`, directions, bores, connection types)
  become ports `P1`/`P2`/`P3` with `connectedTo` along the branch; gaps become generated
  `pmef:Pipe` tubes (TUBI) with `pipeLength`; branch `HREF`/`TREF` link nozzles
  (`connectedLineId`/`connectedPortId`) and tees of other branches. Components without
  p-points fall back to one port `P0` without `connectedTo`. SITE/ZONE are now traversed.
  Fixture `pmef-adapter-e3d/tests/fixtures/branch-ports.pml`.

- **Generic piping component and native type catalog**
  ([RFC-0009](spec/rfc/0009-generic-piping-component.md), spec 02 §4.4).
  New `pmef:GenericPipingComponent` with `nativeType` (e.g. `E3D:PCOM`); new `componentClass`
  values `VALVE_UNSPECIFIED`, `INLINE_INSTRUMENT`, `CLOSURE`; new catalog type
  `NATIVE_TYPE_MAPPING` and `catalogs/native-types-e3d.json`. The E3D adapter maps element
  types through the catalog instead of hard-coded rules: unknown types are kept as generic
  components (before: `pmef:Pipe`), valves without a known SKEY prefix become
  `VALVE_UNSPECIFIED` (before: `VALVE_GATE`). `pmef_catalogs::native_type`, check-catalogs
  rules, example `heat-exchanger-station.ndjson` (CW-301-FE-001), tests `PIPE-U-017/018`.
- **`mesh_ref` glTF node fragments and `meshFrame`**
  ([RFC-0008](spec/rfc/0008-mesh-ref-gltf-node-fragment.md), spec 04 §2.3).
  `ref = <file>#/nodes/<i>;node=<name>;extras.<key>=<value>` selects one glTF node; stable
  selectors win over a stale index. `GeometryReference.meshFrame` gives unit, up axis and origin
  of the mesh file. `GeometryReference.ref` format is now `uri-reference`. New
  `pmef_io::mesh_ref` (parse/resolve, `.gltf`/`.glb`), `pmef_core::geometry::MeshFrame`, and
  check `meshRefsResolve` in `pmef check`. Example `compressor-train.ndjson` (K-101), tests
  `EQUIP-U-013/014`. Found with glTF files converted from an E3D model (DD-011 addendum).
- **Primitives `PYRAMID` and `RECT_TORUS`, `DISH.dishType = SPHERICAL_CAP`, `SNOUT` shear**
  ([RFC-0007](spec/rfc/0007-pyramid-rect-torus-spherical-cap.md), spec 04 §4.2–§4.3). New
  optional `refDirection` on SNOUT and PYRAMID; spec 04 §4.3 now defines the local frame that
  SNOUT offsets always referred to. `pmef-adapter-e3d` maps PML `PYRA`/`RTOR`; `pmef-route`
  bounds both types; `check-geometry.py` rule 12 (face area, radii, cap depth, knuckle > 0).
  Example `compressor-train.ndjson` (governor housing), conformance tests `GEOM-U-009…013`.
  Found by mapping an E3D offshore model (actuators, caps written as knuckle-0 torispheres).
- **HVAC duct domain** `pmef-hvac.schema.json` ([RFC-0006](spec/rfc/0006-hvac-ducts-tray-fittings-catalogs.md),
  spec 02 §11): `DuctNetworkSystem`, `DuctSegment`, `Duct`, `DuctBend`, `DuctTransition`,
  `DuctBranch`, `DuctDamper`, `DuctAccessDoor`, `DuctTestPort`. Cross-sections ROUND, SQUARE,
  RECTANGULAR, FLAT_OVAL per port; connection type (flanged, welded, slip joint…) per port;
  insulation per segment or component. Rust: `pmef-core::hvac` (area, hydraulic diameter, velocity).
- **`pmef:CableTrayFitting`** (E&I, spec 02 §6.6) and optional **`CableObject.catalogRef`**.
- **Catalog types** `DUCT_DIMENSIONS`, `PIPE_SUPPORT_TYPES`, `CABLE_TYPES` with the catalogs
  `duct-dimensions-en`, `pipe-supports-generic`, `cable-types-en`, consistency rules in
  `check-catalogs.py` and typed accessors in `pmef-catalogs`.
- Example `examples/utilities-hvac-cable.ndjson`; conformance tests `HVAC-U-001…013`, `EI-U-009/010`.
- **`pmef route`: re-route a single line-list line or cable.** The report of a problem with line
  lists or cables carries `lineListRoutes` (the routes generated from the line lists); a route in
  `routes` whose `line` is a line-list line now replaces that line instead of routing it twice.
  `cables[].trays` limits a cable to the named trays, and such cables are routed first, so a
  viewer can fix the other cables on the trays they used and re-route one cable alone.
- **`pmef-route-wasm` `RouteSession`: parse a building model once.** `addFile` /
  `removeFile` / `hasFile` / `route(problem, plant)`; the first run parses each file (OBJ mesh
  with its search tree, PMEF package) and drops its text, later runs reuse it. Clinic (140 MB
  OBJ) in Node 22: one-shot `route` 9.6–10.7 s per call, session 2.9 s for the first run and then
  only the search (0.25–0.61 s per line with `coarseStep` 600). In `pmef-route`:
  `SceneCache`, `Scene::load_cached`, `run_cached`; `Shape::Mesh` now holds an `Arc<Mesh>`.

- **`pmef route` reference tests**: `tests/route/corpus/reference_runs.py` re-routes pipe runs
  modelled in the corpus (Duplex, Clinic) between their own end points and compares length,
  bends, clashes and deviation, with comparison images; `dexpi_layout.py` runs the DEXPI
  reference P&ID C01 (`dexpi-pmef` output) on a synthetic layout. Results in
  `docs/auto-routing.md` (Reference tests). Fix found by them: soft mesh obstacles now block
  their interior, so routes no longer run inside thick walls. `lineLists[].classMap` maps P&ID
  piping class codes to catalog classes.
- **`pmef route` stage 6 (integration)**: `lineLists` routes P&ID lines mapped to PMEF
  (DEXPI-Manager `dexpi-pmef`: `PipingNetworkSystem` + `IsConnectedTo` with the nozzle mark)
  between the nozzles of the 3D scene, matching equipment by `@id`, `isDerivedFrom` or tag
  number; segments attach to the P&ID line, unroutable lines are reported (`skippedLines`).
  New crate `pmef-route-wasm` (wasm-bindgen) exports `route(problem, files, plant)` for web
  viewers; `pmef_route::run` reads files through a reader (no file system), CI builds the
  crate for `wasm32-unknown-unknown`. Example: `tests/route/pump-skid-linelist-route.json`.
- **`pmef route` stage 5 (cables)**: `trays` (tray runs as polylines with width, height,
  cable classes, bend radius, existing fill) and `cables` (class, diameter, minimum bend radius,
  ends) route cables over the tray network: drops to the nearest allowed tray (`cableSettings`:
  `maxDrop`, `dropFactor`), segregation by class, bend radius checked at every direction change,
  fill limit (`maxFill`, default 40 %) with the largest cables first. Output: one
  `pmef:CableObject` per cable (`routeLength`, `cableTrayId` of the main tray, ends); the report
  adds the fill per tray. Example: `tests/route/cable-trays-route.json`. No E&I schema change.
- **`pmef route` stage 4 (line rules)**: per-terminal straight length (`straight`), heat-tracing
  allowance (`tracing`) and pipe supports (`supports`: on structural steel within reach, i.e.
  PMEF members or obstacles with `structural: true`; gaps at most the ASME B31.1 Table 121.5
  span, also around tees) written as `pmef:PipeSupport`. The search keeps the straight length
  before the last elbow exactly (state key), also when it exceeds twice the elbow
  centre-to-face.
- **`pmef route` stage 3 (branches)**: `routes[].branches` route further consumers of a line
  onto its tree with equal tees from the class catalog (ASME B16.9 centre-to-end if the catalog
  has none), nearest consumer first; output adds one `PipingSegment` per branch and
  `pmef:Tee` components. Example problem `tests/route/header-branches-route.json`.
- **`pmef route` stage 2 (multi-line negotiation)**: rip-up and reroute after the sequential
  pass (`settings.ripUpRounds`, default 5). A line blocked by earlier lines is routed first and
  the blocking lines are re-routed around it; the report notes the re-routed lines.
- **`pmef route` stage 1 (plant-scale graphs)** (`docs/auto-routing.md`): pipe racks
  (`racks`: levels as preferred edges at top of steel + shoe + outer radius), coarse-to-fine
  search (`grid.coarseStep`, `grid.corridor`) with fallback to the full graph, a Morton-chunked
  triangle index for large OBJ models, and reserved terminal stubs (an earlier line no longer
  blocks a later line's nozzle). On the corpus models, coarse-to-fine cuts the routing time
  by a factor of 5 (Duplex) to 20 (Clinic).
- **Catalog `piping-class-c5d1` 2024.3**: 90° long-radius elbows for DN 15, 20, 25, 32, 40, 65 and
  250, so every pipe size in the class has one (48 → 55 components). Every elbow now carries
  `centreToFace_mm` per ASME B16.9. `pmef route` can now route C5D1 lines in DN 25/40, which it
  rejected before for lack of an elbow.
- **Auto-routing kernel `pmef-route`** (new crate, CLI `pmef route`): routes pipes in a voxel graph
  around PMEF equipment, steel, existing piping and OBJ meshes, with elbow-fit, clearance and
  piping-class rules from the PMEF catalogs, and writes PMEF `Pipe`/`Elbow` components. Design and
  later increments: `docs/auto-routing.md`. Ownership: `docs/ECOSYSTEM.md` decision 6.
- **`provenance.sourceType` value `generated`** (spec 02 §2.9.1, `pmef-base` `$defs/Provenance`,
  RFC-0001 table): object produced by a design algorithm such as auto-routing. Such objects SHOULD
  carry `review.status: detected` until checked. New example `examples/pump-skid-auto-routed.ndjson`
  (0.10.0 feature example, not used for the 0.9.0 conformance fixtures).
- **PMEFX 1.0 container** ([RFC-0005](spec/rfc/0005-pmefx-object-index.md), spec 03 §3): object
  index `index/objects.ndjson` linking each `@id` to its NDJSON record, bounding box, glTF mesh
  levels of detail (coarse → fine, `triangles`, `vertexCount`, `errorMm`) and STEP AP242
  B-rep; loose octree `index/octree.json`; manifest `pmefxVersion`, `meshOrigin`, per-file
  `sha256`; archive rules (safe paths, stored geometry entries for range reads). New schema
  `schemas/pmefx-manifest.schema.json`, example `examples/pmefx/pump-skid-complete/`.
  Reference implementation `pmef_io::pmefx` (feature `pmefx`) and CLI `pmef pack`,
  `pmef unpack`, `pmef verify` (closes docs/AUDIT.md P3.2). New Rust dependencies `zip`
  (MIT) and `sha2` (MIT/Apache-2.0).
  Mesh levels of detail: `pmefx::glb::mesh_glb` writes a triangle mesh (project mm, Z-up →
  glTF m, Y-up relative to `meshOrigin`, 32-bit indices, `extras.pmefId`) and
  `pmefx::glb::read_mesh_glb` reads any LOD glb back into project coordinates (node transforms
  applied), so producers (PlantView) and viewers (cad-engine) share one implementation of the
  spec 03 §3.6 mapping.
  `read_mesh_glb` transforms normals with the inverse transpose, reverses the winding under
  mirroring transforms and rejects overflowing accessor offsets instead of panicking; its
  singularity test is scale-independent. `pmefx::glb::mesh_glb_quantized` writes compact LODs
  with the glTF extension `KHR_mesh_quantization` (16-bit positions within a given error
  bound, else 32-bit floats; 8-bit normals; welded vertices; 16-bit indices where they fit),
  about half the size; `read_mesh_glb` reads them and rejects other required extensions.
  Spec 03 §3.6 allows `KHR_mesh_quantization` (readers MUST support it).

- **`pmef-catalogs` crate** (`pmef-rs/pmef-catalogs`) — embeds every catalog in `catalogs/` so
  other repositories read PMEF catalogs instead of keeping their own tables (ecosystem roadmap
  stage 2, `docs/ECOSYSTEM.md`). Raw/parsed access by `catalogId`, plus typed pipe sizes
  (`pipe_sizes`, `nps_to_dn`, `dn_to_nps`, `parse_nps`). The JSON files stay normative; a test
  fails when a catalog file is added without being embedded. No schema change.

- **Catalog schema: flange entry fields and `fieldDefinitions`** — `flangeDimensionEntry` now
  defines the optional fields `NPS`, `DN`, `flangeThickness_mm`, `boltCircle_mm`, `boltHoles`
  (multiple of 4), `boltDiameter_mm`, `boltHoleDiameter_mm`, `raisedFace_mm` and `facingType`
  (RF/FF/RTJ); catalogs may document their field meanings in `fieldDefinitions`. Negative cases
  in `tests/catalogs/run.py`. Catalogs are read as JSON values in `pmef-rs`; no Rust types change.

- **`CompressorSpec.polytropicEfficiency`** (optional, [%]) — camelCase replacement for the
  deprecated `polytropic_efficiency` (RFC-0004). Tests: `pmef-validate/tests/rfc0004.rs`,
  `pmef-core/tests/rfc0004_roundtrip.rs`.

- **Differential validator test** `tests/validators/differential.py` (CI): the Rust validator and
  the Python reference must reach the same verdict on all reference instances and systematic
  mutations of them. Spec 06 §5.1 states the validation semantics (format = annotation,
  unreadable lines invalid, unknown `@type` per CL1-03).

- **Catalog consistency check** `scripts/check-catalogs.py` (CI): section areas and masses from
  the dimensions, I/H section properties, lb/ft ↔ kg/m and AISC designation weights,
  designation/catalogId ↔ dimensions, monotony along a series, pipe bore/mass/schedule order,
  flange geometry, and piping classes against the pipe and flange dimension catalogs. Golden
  negative fixture `tests/catalogs/` (CI).

- **Geometry checks for equipment and structural steel** in `scripts/check-geometry.py`:
  nozzle face not inside / not detached from the owner's parametric body, nozzle direction
  outward (spec 02 §5.2), DISH heads on a cylinder end face (spec 04), connected line has a free
  end at the nozzle face; steel member length, `profileId` resolvable in `catalogs/`, nodes at
  member ends, connected members within (h1 + h2) / 2 (profile depths from the catalog).
  Equipment without parametric geometry: nozzle face not inside `geometry.boundingBox`.
  Golden negative fixture `tests/geometry/` (CI).

- **AutoCAD Plant 3D import direction and round trip via PCF** (spec 05 §8.2).
  `pmef convert MODEL --to pcf -o DIR` writes one PCF per line (identity in
  `UNIQUE-COMPONENT-IDENTIFIER`); `pmef convert DIR --to pmef --base MODEL -o OUT` updates the
  model from the PCF exported by Plant 3D (re-identification, create, delete; fields PCF does not
  carry are kept). Library: `pmef_adapter_plant3d::pcf_bridge`. The PCF reader now also reads
  `CO-ORDS` (supports). Round trip of PMEF-DS-01 is lossless; 15 % of its attributes travel
  through PCF. Conformance: level 3 as converter
  (`tests/conformance/suts/plant3d-pcf.sh`, run in CI), not yet as adapter (INTG-006: no native
  Plant 3D id in PCF). Not yet verified with a Plant 3D installation.
- **Three-way `pmef merge`** (spec 03 §10). Merges two edits of a common base object by object and
  field by field with the diff rules (keyed arrays, numeric tolerances); one-sided changes are
  taken, differing changes on both sides are conflicts (`modify-modify`, `modify-delete`,
  `add-add`) reported with base/ours/theirs values; `--prefer ours|theirs` decides the written
  value. Deterministic output (FileHeader first, `@id` order, sorted keys); exit 0/1/2.
  Library `pmef_core::merge`; golden test: design change merged with confirmed as-built deviations.
- **RFC-0003 — persisted as-built ↔ design correspondence** ([RFC](spec/rfc/0003-as-built-correspondence.md)).
  `pmef:IsRevisionOf` with `changeType = AS_BUILT` gains an optional `correspondence` block
  (`method`, `distance_mm`, `ambiguous`; schema requires `AS_BUILT` when present); confirmation via
  the RFC-0001 `review` block. `pmef diff --match` uses confirmed correspondences as a new pass 1b
  (`declared`), never re-proposes rejected pairs, and treats correspondences as evidence rather
  than compared objects. `--emit-matches FILE` writes proposals (`review.status = detected`) for
  review. Rust: `relationships::AsBuiltCorrespondence`. Example and tests (schema rules, matching
  units, CLI propose → approve/reject → rerun workflow) added.
- **`pmef diff --match`: comparison when `@id`s differ** (spec 03 §9.5) — typical for as-built
  models with recogniser ids. Objects are matched one-to-one in fixed passes: same `@id`, same
  engineering identifier (tag, line number, mark — only if unique), same native tool object
  (`HasEquivalentIn`), geometric proximity of anchor points (ports, nozzles, member ends;
  direction-free mean distance ≤ `--match-distance`, default 100 mm, with an ambiguity flag), and
  references (geometry owners, relationship endpoints). References in the head package are
  rewritten to the matched ids before comparing, so differing ids cause no spurious changes.
  Library: `pmef_core::matching`, `pmef_core::diff::diff_matched`. Golden test: an as-built
  model with scanner ids yields the same plant deviations as the identical-id case.
- **Deterministic object-level `pmef diff`** (spec 03 §9). Objects matched by `@id`, field-level
  changes with stable paths (`ports[portId=P2].coordinate[2]`), keyed array matching (ports,
  nozzles, residuals, composite parts), spec 06 §6.2 numeric tolerances (`--exact` to disable),
  `--ignore <path>` and `--ignore-recognition`, text and JSON report, exit codes 0/1/2. Output
  is independent of object order, JSON key order and keyed array order; duplicate `@id`s are an
  error. Library: `pmef_core::diff`. Golden as-built-vs-design tests in `tests/diff/`.
- **RFC-0002 — deterministic catalog snapping `pmef-snap/1`** ([RFC](spec/rfc/0002-catalog-snapping.md),
  spec 02 §2.9.4). Normative algorithm for straight pipes (`PIPING_CLASS`) and HEA/HEB/HEM/IPE/W
  sections (`STEEL_PROFILES`, EN and AISC field conventions): tolerance band per parameter,
  admissibility, normalised RMS score, total ordering (score → primary residual → `catalogId`),
  runner-up and `ambiguous` flag (margin 0.1), recorded non-default tolerances. Schema:
  `CatalogSnap` gains optional `algorithm`, `score`, `runnerUp`, `ambiguous`, `tolerances`.
  Two implementations — Python reference (`tests/snapping/reference_snap.py`) and Rust
  (`pmef_core::snapping`) — pass the same 16 test vectors (`tests/snapping/vectors.json`) in CI.
  `examples/asbuilt-recognition.ndjson` is now generated with the reference implementation
  (`scripts/generate-asbuilt-example.py`).
- **Executable conformance test suite** (`tests/`, spec 06 §5–§7). `python tests/run_tests.py`
  runs the 82 normative test IDs — 50 unit and 8 negative schema tests (fixtures generated from
  one catalogue, `tests/conformance/manifest.json`), INTG-001…006 integration checks and
  RT-001…018 round-trip checks with aggregate and identity-field fidelity per spec 06 §6 — against
  any implementation via command templates (`--validator "cmd {file}"`,
  `--sut "cmd {input} {output}"`, `--kind validator|converter|adapter`). Produces the §7.1 JSON
  report (schema-checked) and the §7.2 text summary. A self-test proves that seven deliberately
  broken implementations and an accept-all validator are detected. CI runs the suite, the
  self-test, and the Rust `pmef` CLI as a second validator (agrees with the Python reference on
  all 58 unit/negative tests). This replaces the `pmef-conformance/` suite claimed for 0.9.0-rc,
  which was never published.
- **RFC-0001 — recognition provenance, review status and as-built data (schema 0.10.0,
  backward compatible).** Every object that carries `revision` may now carry three optional
  blocks ([spec 02 §2.9](spec/02-information-model.md), [RFC](spec/rfc/0001-provenance-asbuilt.md)):
  - `provenance` — `sourceType` (`manual`, `cad-import`, `iso-drawing`, `2d-drawing`, `scan`,
    `photo`), `sourceRef` URI (document page/region or point-cloud segment), `recognizer`
    (name + version), `confidence` 0..1, `createdAt`;
  - `review` — `detected` / `reviewed` / `approved` / `rejected` with `reviewedBy`/`reviewedAt`;
    schema-enforced ISO 19650 rule: `detected` ⇒ `WIP`, `rejected` ⇒ not `PUBLISHED`;
  - `asBuilt` — `pointCloudRef`, `fitMethod`, `deviationRms_mm`, `deviationMax_mm`, and
    `catalogSnap` (catalog id **+ version** + entry, snapped size, per-parameter residuals).
  - Schemas: `pmef-base` `$defs` `Provenance`, `ReviewStatus`, `AsBuilt`, `CatalogSnap`,
    `ReviewChangeStateRule`, referenced from all 20 object definitions. `$id` paths unchanged.
  - Rust: `pmef-core::provenance` types, flattened into all 27 object structs.
  - Example: `examples/asbuilt-recognition.ndjson` — DN 150 pipe run recognised from a scan and
    an isometric, snapped to `piping-class-en-p1a` 2024.1; HEB 300 beam snapped to `profiles-en`
    2024.1; one `detected`, one `reviewed`, one `rejected` object (synthetic, CC0).
  - Tests: Rust positive/negative schema tests (`pmef-validate/tests/provenance.rs`) and a lossless
    serde round trip of every example object (`pmef-core/tests/provenance_roundtrip.rs`).
  - CAEX/AutomationML mapping documented in the RFC.

- `scripts/validate-catalogs.py` — validates every file in `catalogs/` against
  the catalog schema and flags duplicate `catalogId` values within a catalog.
- CI job `validate-catalogs` (and `catalogs/**` path trigger) in the
  schema-validation workflow, so malformed catalogs fail the build.
- `scripts/check-topology.py` — dataset connectivity integrity checks for
  NDJSON examples: unique `@id`, `connectedTo` resolution, and port
  reciprocity (with `--allow-external` for federated/cross-dataset boundaries).
  Wired into the `validate-examples` CI job.
- **`pmef-validate` topology module** (`pmef-rs/pmef-validate/src/topology.rs`):
  Rust reference implementation of the same connectivity checks (unique `@id`,
  `connectedTo` resolution, port reciprocity), exposed as `check_topology()`
  with a `TopologyReport`, at parity with `check-topology.py`.
- **`pmef topology` CLI subcommand** — runs the topology checks over an NDJSON
  package, with `--allow-external` for cross-dataset boundaries.

---

## [0.9.0-rc] — 2026-03-31 · Release Candidate

### Summary

First public release candidate. Covers the complete five-layer architecture
for Piping, Equipment, E&I, Structural Steel, and Pipe Stress. Includes
the full Rust reference implementation workspace (`pmef-rs`), 7 open JSON
catalogs, 5 validated benchmark datasets (117 objects, 0 schema errors),
6 normative specification chapters (18,128 words), and a conformance test
suite with 82 tests (80 passing, 97.6%).

---

### Added — Specification (spec/)

Six normative Markdown chapters totalling 3,087 lines / 18,128 words:

- **Ch. 01 Introduction**: 30+ normative references, 25 term definitions,
  45 abbreviations, problem statement, design goals G-01..G-09, five-layer
  architecture, P&ID→3D→EAM linking model, standards alignment
- **Ch. 02 Information Model**: entity type registry (50+ types), all base
  types (PmefId, RevisionMetadata, GeometryReference, CatalogReference, Port),
  plant hierarchy, piping domain (13 component subtypes + topology rules),
  equipment domain (10 subtypes + Nozzle), E&I, steel, 10 relationship types,
  11 property sets, extension mechanism
- **Ch. 03 Serialisation**: NDJSON primary format (canonical key order,
  LF, UTF-8, one-object-per-line), PMEFX container, CAEX XML secondary,
  delta package protocol
- **Ch. 04 Geometry**: 15 parametric primitives fully specified, 5 LOD levels,
  glTF 2.0 + STEP AP242 + OpenUSD layer specs, primitive-to-component mapping
- **Ch. 05 Adapters**: 8-step export + 6-step import pipeline, identity
  resolution, 8 general requirements, 6 tool-specific specs (E3D, Plant3D,
  CADMATIC, Tekla, COMOS, EPLAN), full PCF mapping + unit conversion tables
- **Ch. 06 Conformance**: 3 conformance levels (L1/L2/L3), 32 normative
  clauses, entity type matrix (50+ × 3 levels), 5 benchmark datasets,
  RT fidelity definition, conformance report schema, certification process

### Added — JSON Schemas (schemas/)

9 schemas, all Draft 2020-12, all valid:
`pmef-base`, `pmef-piping-component`, `pmef-equipment`, `pmef-geometry`,
`pmef-property-sets`, `pmef-ei`, `pmef-steel`, `pmef-relationships`,
`pmef-catalog` (new)

### Added — Catalogs (catalogs/) — CC0

| File | Entries | Description |
|------|---------|-------------|
| `profiles-en.json` | 202 | HEA/HEB/IPE/UPE/RHS/SHS/CHS/L/Flat per EN |
| `profiles-aisc.json` | 119 | W / HSS Rect / HSS Round / L per AISC 16th |
| `pipe-dimensions.json` | 162 | ASME B36.10M + B36.19M, DN15–DN1200 |
| `piping-class-a1a2.json` | 203 | CS ANSI-150, DN15–DN600, 9 component types |
| `piping-class-b3c1.json` | 143 | CS ANSI-300, DN15–DN400, 8 component types |
| `caesarII-cii-mapping.json` | 29 records | PMEF↔CAESAR II .cii + ROHR2 .ntr |
| `rdl-uri-map.json` | 35 | ISO 15926-4 PCA-RDL URI cross-references |

### Added — Benchmark Datasets (examples/) — CC0

| Dataset | Objects | Disciplines | Key content |
|---------|---------|------------|-------------|
| DS-01 `pump-skid-complete.ndjson` | 35 | Piping + Equipment + Steel | P-201A (API 610), V-201 (EN 13445), CW-201/202/203, full piping chain, 2 welds, spool, geometry |
| DS-02 `heat-exchanger-station.ndjson` | 26 | Piping + Equipment + E&I | E-301A/B (TEMA BEM), FV-30101 control valve, FIC-30101 (HART), 8 lines |
| DS-03 `compressor-train.ndjson` | 34 | Piping + Equipment + E&I | K-101 (API 617), ST-101 (API 611), V-101 KO drum, E-101 (TEMA AES), STR-101, SIL2 anti-surge |
| DS-04 `ei-loop-fic-10101.ndjson` | 9 | E&I | FIC-10101 SIL1 controller, XV-10101, S7-1500, loop FIC-101 |
| DS-06 `stress-model-cw201.ndjson` | 13 | Piping + Stress | CW-201 CAESAR II model, anchor reactions, spring hanger, stress ratios |

### Added — Rust Reference Implementation (pmef-rs/) — Apache 2.0

6-crate Cargo workspace, 3,723 lines:

- **pmef-core** (1,831 lines): `PmefId` (FromStr), `PmefVersion`, `Coordinate3D`,
  `RevisionMetadata`, `PmefEntityType` (50 variants), `PmefEntity`/`PmefVisitor`/
  `PmefAdapter` traits, `AdapterError`/`AdapterStats`, all 50+ entity structs,
  `piping_component_base!{}` macro, domain methods
- **pmef-io** (375 lines): `NdjsonReader<R: BufRead>` iterator, `NdjsonWriter`,
  `canonical_json()`, `PmefPackageIndex` — 4 unit tests
- **pmef-validate** (21 lines): `schema_for_type()` routing table
- **pmef-geom** (18 lines): `Vec3`, `Aabb`, `cylinder_aabb()`
- **pmef-cli** (466 lines): 7 subcommands (validate, diff, stats, convert,
  index, conformance, check-refs) — clap 4 derive, Tokio async
- **pmef-adapter-plant3d** (672 lines): full PCF parser + export pipeline,
  `MaterialMapper`, coordinate-based topology, 9 unit tests

### Added — Conformance Test Suite (pmef-conformance/)

> **Note (2026-10-31):** `pmef-conformance/` is not part of this repository and does not appear
> in its Git history; the figures below cannot be reproduced from the published sources.
> See [docs/legacy-docx-review.md](docs/legacy-docx-review.md). The executable suite now lives in
> [`tests/`](tests/README.md) (see [Unreleased]).

82 tests: PIPE-U-001..016, EQUIP-U-001..012, EI-U-001..008, STEEL-U-001..006,
REL-U-001..008, GEOM-U-001..008, RT-001..018, INTG-001..006.
**80/82 passing (97.6%).**

---

### Fixed (schema bugs discovered by test suite)

| Fix | Schema | Details |
|-----|--------|---------|
| `PmefId` underscore | `pmef-base` | `_` now allowed in local-id segment |
| `PIPE_SUPPORT` enum | `pmef-property-sets` | Added to componentClass enum |
| `insulationType` enum | `pmef-equipment` | Short forms `HOT`/`COLD` (not `THERMAL_HOT`) |
| `revision` field | `pmef-geometry` | Added to ParametricGeometryObject |
| `operatingTemperature` | `pmef-piping-component` | Removed from PipingSpecification |

---

### Known Issues

| ID | Description | Status |
|----|-------------|--------|
| EQUIP-U-005 | `KNOCK_OUT_DRUM`/`ELECTRIC_ARC_FURNACE` not in vesselSubtype enum | Fix in v0.9.1 |
| PIPE-U-003 | Error message assertion too strict on lineNumber error text | Fix in v0.9.1 |
| pmef-geom | Full AABB for all 15 primitives is a stub | Planned v1.0 |
| pmef-validate | jsonschema Rust crate integration is a stub | Planned v1.0 |
| DS-05 | Mixed EAF + E&I + Steel benchmark not yet produced | Planned v0.9.1 |
| CADMATIC adapter | REST API adapter not yet produced | Planned v0.9.1 |

---
