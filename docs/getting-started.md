# Getting Started with PMEF

**Version:** 0.9.0-rc
**Date:** 2026-03-31

---

## What is PMEF?

The **Plant Model Exchange Format (PMEF)** is an open, cross-discipline data exchange
format for industrial plant models. If you have ever tried to move a 3D piping model
from AVEVA E3D into Hexagon Smart 3D, or pull equipment data from AutoCAD Plant 3D
into an ERP system, you know the pain: every tool speaks its own proprietary language.
PMEF gives them a shared one.

PMEF covers eight engineering disciplines -- Piping, Equipment, Structural Steel,
Electrical and Instrumentation, Pipe Stress Analysis, Process Simulation, ERP/EAM,
and Civil/Architecture -- in a single information model grounded in ISO 15926,
DEXPI 2.0, CFIHOS V2.0, and IEC 81346. It stores objects in **NDJSON**
(Newline-Delimited JSON), one object per line, so models are human-readable,
git-friendly, and streamable.

Whether you are building an adapter for a CAD tool, writing a data pipeline for
project handover, or just exploring what a modern plant exchange format looks like,
this guide will get you up and running.

---

## Prerequisites

You need a few tools on your machine before working with PMEF files:

- **Python 3.10+** -- for running the validation and example scripts in `scripts/`
- **pip** -- install the project's Python dependencies with `pip install -r requirements.txt`
- **A JSON-aware text editor** -- VS Code with the
  [Red Hat YAML extension](https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml)
  gives you autocomplete against the PMEF schemas
- **Git** -- PMEF files are designed for version control; you will want Git to
  track changes at the object level
- **jq** (optional) -- handy for pretty-printing individual NDJSON lines on the command line
- **Docker** (optional) -- for running the future `pmef-cli` container image

### Installing the PMEF CLI (future)

Once the Rust CLI is published to crates.io you will be able to install it directly:

```bash
cargo install pmef-cli
```

Or pull the Docker image:

```bash
docker pull ghcr.io/pmef/pmef-cli:latest
```

Until then, use the Python validation scripts in the `scripts/` directory.

---

## Quick Validation of an Example File

The repository ships with a complete benchmark example. Validate it against the
schemas with the Python helper:

```bash
# Clone the repository
git clone https://github.com/pmef/specification.git
cd specification

# Install Python dependencies
pip install -r requirements.txt

# Validate all schemas are well-formed JSON Schema Draft 2020-12
python scripts/validate-schemas.py

# Validate example files against the schemas
python scripts/validate-examples.py
```

If everything passes you will see no errors. Any validation failures are printed
with the object `@id` and a JSON Schema error path so you can locate the problem
quickly.

---

## Understanding the NDJSON Format

A PMEF file is plain text. Each line is a self-contained JSON object. The first
line is always a `pmef:FileHeader`, followed by hierarchy objects (Plant, Unit),
then domain objects (Pump, Vessel, Pipe, Valve, ...), geometry, and relationships.

Here is what the first three lines of the benchmark example look like
(`examples/pump-skid-complete.ndjson`):

```jsonc
// Line 1 -- FileHeader: declares version, project, and authoring tool
{"@type":"pmef:FileHeader","@id":"urn:pmef:pkg:eaf-2026:pump-skid-ds01",
 "pmefVersion":"0.9.0","projectCode":"EAF-2026",
 "description":"PMEF-DS-01 Benchmark: Cooling Water Pump Skid (complete)"}

// Line 2 -- Plant: top-level container
{"@type":"pmef:Plant","@id":"urn:pmef:plant:eaf-2026:EAF-LINE-3",
 "pmefVersion":"0.9.0","name":"Electric Arc Furnace Line 3",
 "location":"Duisburg, Germany"}

// Line 3 -- Unit: a process unit inside the plant
{"@type":"pmef:Unit","@id":"urn:pmef:unit:eaf-2026:U-100",
 "pmefVersion":"0.9.0","name":"Cooling Water Unit","unitNumber":"U-100",
 "isPartOf":"urn:pmef:plant:eaf-2026:EAF-LINE-3"}
```

> **Note:** The above is reformatted for readability. In a real PMEF file each
> object is a single line with no line breaks inside.

### Inspecting a single object with jq

```bash
# Pretty-print the 4th line (the Pump object)
sed -n '4p' examples/pump-skid-complete.ndjson | jq .
```

### Why NDJSON?

- **Git diffs are meaningful.** Changing one valve produces a one-line diff.
- **Streaming.** Millions of objects can be read without loading the whole file.
- **No special parser.** Any language with a JSON library can read it.

---

## Key Concepts

### Entity Types

Every PMEF object has an `@type` field that identifies what it is. Types are
grouped into domains:

- **Plant Hierarchy** -- `pmef:FileHeader`, `pmef:Plant`, `pmef:Unit`, `pmef:Area`
- **Piping** -- `pmef:PipingNetworkSystem`, `pmef:PipingSegment`, `pmef:Pipe`,
  `pmef:Elbow`, `pmef:Valve`, `pmef:Flange`, ...
- **Equipment** -- `pmef:Pump`, `pmef:Vessel`, `pmef:HeatExchanger`, `pmef:Column`, ...
- **E&I** -- `pmef:InstrumentObject`, `pmef:CableObject`, `pmef:MTPModule`, ...
- **Structural Steel** -- `pmef:SteelSystem`, `pmef:SteelMember`, `pmef:SteelNode`, ...
- **Geometry** -- `pmef:ParametricGeometry`
- **Relationships** -- `pmef:IsPartOf`, `pmef:IsConnectedTo`, `pmef:Supports`, ...

### PmefId

Every object has a globally unique `@id` in URN format:

```text
urn:pmef:<domain>:<project>:<local-id>
```

For example, `urn:pmef:obj:eaf-2026:P-201A` identifies pump P-201A in
project EAF-2026. The ID is stable across revisions -- it identifies the
physical thing, not a particular version of the data.

### Relationships

Objects reference each other through typed fields:

- **`isPartOf`** -- structural containment (Unit is part of Plant, Pipe is part of Segment)
- **`connectedTo`** -- physical connection between ports (pipe to elbow, nozzle to flange)
- **`isDerivedFrom`** -- traceability from 3D object back to a P&ID functional object

Explicit relationship objects (`pmef:IsPartOf`, `pmef:IsConnectedTo`, etc.) can
carry additional metadata such as revision info and discipline tags.

### Geometry Layers

PMEF supports three geometry representations, and objects can have all three
simultaneously:

1. **Parametric primitives** -- built from 15 primitive types (cylinder, cone, dish,
   torus, box, ...). Lossless, compact, great for engineering queries.
2. **glTF 2.0 mesh** -- triangulated mesh for visualisation. Referenced via
   `"type": "mesh_ref"`.
3. **STEP AP242 B-Rep** -- exact boundary representation for fabrication. Referenced
   via `"type": "step_ref"`.

The `geometry` field on each object specifies which layer is present and at what
level of detail (LOD), from `BBOX_ONLY` to `LOD4_FABRICATION`.

### Revision Metadata

Every domain object carries a `revision` block with ISO 19650 CDE workflow state
(`WIP`, `SHARED`, `PUBLISHED`, `ARCHIVED`), a monotonic revision ID, change reason,
and an optional SHA-256 checksum for integrity verification.

---

## Next Steps

Now that you understand the basics, here is where to go next:

### Read the specification

- [Chapter 01 -- Introduction](../spec/01-introduction.md) -- scope, design goals,
  and relationship to other standards
- [Chapter 02 -- Information Model](../spec/02-information-model.md) -- full entity
  type registry, base types, and domain models
- [Chapter 03 -- Serialisation](../spec/03-serialisation.md) -- NDJSON rules, PMEFX
  container format, ordering constraints
- [Chapter 04 -- Geometry](../spec/04-geometry.md) -- the 15 parametric primitives
  and multi-layer geometry strategy
- [Chapter 05 -- Adapters](../spec/05-adapters.md) -- round-trip requirements for
  tool adapters
- [Chapter 06 -- Conformance](../spec/06-conformance.md) -- the three conformance
  levels and the test suite

### Explore the schemas

All JSON Schema files live in [`schemas/`](../schemas/). Key files:

- `pmef-base.schema.json` -- shared primitives (PmefId, Coordinate3D, RevisionMetadata)
- `pmef-hierarchy.schema.json` -- FileHeader, Plant, Unit, Area
- `pmef-piping-component.schema.json` -- piping network and 12 component types
- `pmef-equipment.schema.json` -- 10 equipment types with nozzles
- `pmef-geometry.schema.json` -- 15 parametric geometry primitives

### Look at the examples

- [`examples/pump-skid-complete.ndjson`](../examples/pump-skid-complete.ndjson) --
  fully annotated benchmark with pump, vessel, piping, welds, and supports

### Contribute

See [`CONTRIBUTING.md`](../CONTRIBUTING.md) for how to file issues, propose schema
changes, and submit adapters. Join the community on
[Discord](https://discord.gg/pmef).

### Design rationale

Curious why certain decisions were made? Read
[`docs/design-decisions.md`](design-decisions.md) for the architectural decision
records.

---

*This guide is informative, not normative. The authoritative reference is the
[PMEF Specification](../spec/).*
