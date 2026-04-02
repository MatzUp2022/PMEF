# PMEF Design Decisions

This document records the key design decisions for the PMEF data model, including alternatives considered and rationale.

---

## DD-001: JSON Schema Draft 2020-12

**Decision:** Use JSON Schema Draft 2020-12 as the normative schema language.

**Alternatives considered:**

- JSON Schema Draft-07 — wider tooling support but lacks `$dynamicRef`, `prefixItems`
- OWL 2 DL — more expressive but requires RDF toolchain
- SHACL — good for RDF validation but no JSON native support
- Protobuf/Avro — binary, not human-readable, no semantic extensions

**Rationale:** Draft 2020-12 is the current stable spec, supported by `jsonschema` (Python), `ajv` v8 (JS),
`jsonschema-rs` (Rust). It allows `$defs` for reuse, `allOf` for inheritance, and proper `$ref` to external
schema files. OWL remains the normative ontology (`pmef-base.schema.json` references OWL URIs via `rdlType`)
but is not used for instance validation.

---

## DD-002: NDJSON as Primary Serialisation

**Decision:** Newline-Delimited JSON (one complete object per line) as the primary PMEF serialisation.

**Rationale:**

- Git produces meaningful diffs: adding/modifying one component = one changed line
- Streamable: large plant models can be read/written without loading the full document
- No special parser needed: standard JSON libraries + line split
- UUID-based `@id` guarantees stable identity across revisions
- Deterministic key order (alphabetical within each object) prevents spurious diffs

**Alternative:** CAEX/AutomationML XML — considered for `pmef-caex.xml` secondary serialisation. XML is
preferred by some industry partners; a CAEX-compatible XML serialisation is planned as a secondary format.

**Git LFS:** Binary geometry files (`.glb`, `.usdc`, `.stp`) are stored in Git LFS; only NDJSON topology
and property files are tracked natively.

---

## DD-003: ISO 15926-14/IDO as Upper Ontology Basis

**Decision:** Use ISO 15926-14 (Industrial Data Ontology, IDO) as the semantic foundation via `rdlType`
URIs pointing to `https://rds.posccaesar.org`.

**Rationale:** IDO provides a formally rigorous 4D ontology with `PhysicalObject`/`FunctionalObject`
separation that maps directly to the PMEF P&ID→3D linking problem. The PCA-RDL SPARQL endpoint provides
12,000+ resolved class URIs. Using URIs (not embedded ontology triples) keeps PMEF files small and
human-readable while maintaining semantic richness for tools that resolve them.

**"ISO 15926 Lite":** The full Part 2 EXPRESS model (~201 entities) is too complex for most implementers.
PMEF defines a pragmatic subset of ~45 core classes while retaining compatibility with the full IDO for
advanced use.

---

## DD-004: Port-Based Topology Graph

**Decision:** Model piping connectivity as a Port-based topology graph (`Port.connectedTo` → `Port.@id`),
not as coordinate-based proximity (PCF approach).

**Rationale:** PCF relies on matching coordinates within tolerance — fragile for large models and provides
no semantic connectivity. PMEF Ports are first-class objects with explicit `portId` + `connectedTo`
references, enabling:

- Reliable topology traversal (e.g. "find all components on line CW-201")
- Clear flow direction (`portType`: INLET/OUTLET/BIDIRECTIONAL)
- Nozzle-to-piping linking without coordinate matching


---

## DD-005: Nozzle as Cross-Domain Connector

**Decision:** The `Nozzle` object on `EquipmentObject` is the explicit cross-domain link between Equipment
and Piping domains.

**Rationale:** This mirrors the real engineering boundary: equipment vendors design to nozzle faces;
piping engineers connect from nozzle faces. The Nozzle carries both the equipment-side attributes
(flangeRating, facing) and the piping-side connection reference (`connectedLineId`, `connectedPortId`).
This avoids the common problem of different coordinate systems between equipment and piping models.

---

## DD-006: Geometry Layers Are Optional and Additive

**Decision:** Geometry is never required; all three geometry layers (parametric, glTF mesh, STEP B-Rep)
are optional and can coexist on the same object.

**Rationale:** Different use cases need different geometry fidelity:

- Lightweight data exchange: no geometry, just semantics
- Spatial clash detection: LOD2 parametric or glTF mesh
- Construction/fabrication: LOD4 parametric + STEP B-Rep
- Simulation/Digital Twin: OpenUSD prims


The `GeometryReference.lod` field communicates what detail level is provided, so consumers can request
the appropriate layer.

---

## DD-007: RevisionMetadata on Every Object

**Decision:** Every PMEF object carries `RevisionMetadata` (revisionId, changeState, changedAt,
authoringTool, authoringToolObjectId).

**Rationale:** Round-trip fidelity requires knowing the source system's native ID for re-import. The CDE
workflow (WIP→SHARED→PUBLISHED→ARCHIVED) from ISO 19650 is embedded at object level, not just at file
level. `parentRevisionId` enables a full revision chain per object, supporting "time travel" queries on
As-Built models.

---

## DD-008: CustomAttributes Extension Mechanism

**Decision:** Every PMEF object has an optional `customAttributes` object for project-specific key/value extensions.

**Constraints:**

- Values are restricted to scalar types (string, number, boolean, null)
- Keys must not shadow defined PMEF properties
- Custom attributes should eventually be promoted to standard PMEF properties via RFC


**Rationale:** Real plant projects always have project-specific attributes (procurement codes, PBS
references, material class codes). Refusing to model them forces users to use parallel spreadsheets.
Formalising them as a typed extension namespace prevents chaos while keeping the schema clean.

---

## DD-009: SKEY Extension (PCF++ Compatibility)

**Decision:** `PipingComponentSpec.skey` extends the PCF 4-character SKEY to 8 characters and adds `rdlTypeUri`.

**PCF SKEY:** 4 chars (2 type + 2 end-type), e.g. `ELBW`
**PMEF SKEY:** 8 chars (2 type + 2 end-type + optional 4 variant), e.g. `ELBWLR90` (long-radius 90° elbow)

**Rationale:** Full PCF round-trip requires preserving SKEY. Extending to 8 chars enables variants
(pressure rating, type suffix) without breaking SKEY-based toolchains. `rdlTypeUri` provides the semantic
link that PCF lacks entirely.

---

## DD-010: Units Convention

| Quantity | PMEF Unit | Rationale |
|----------|-----------|-----------|
| Length | mm | Plant engineering standard; matches E3D, CADMATIC, Plant 3D |
| Pressure | Pa | SI; unambiguous vs bar/psi/barg |
| Temperature | K | SI; avoids °C/°F confusion; 0=absolute zero |
| Mass | kg | SI |
| Flow (volumetric) | m³/h | Industry convention |
| Flow (mass) | kg/s | SI |
| Energy/Power | W / kW | SI |
| Angles | degrees | Industry convention (not radians) |

All unit conversions are the adapter's responsibility. PMEF always stores in the above units.
