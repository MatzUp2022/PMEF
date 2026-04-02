---
name: New Adapter Proposal
about: Propose a new bidirectional adapter for a specific engineering tool
title: '[ADAPTER] '
labels: ['adapter', 'needs-triage']
assignees: ''
---

## Tool Information

| Field | Value |
|-------|-------|
| **Tool name** | |
| **Vendor** | |
| **Version(s)** | |
| **Discipline(s)** | Piping / Equipment / Steel / E&I / … |
| **Market position** | Brief description of tool's market share/importance |

## API / Export Capabilities

<!-- What interfaces does the tool expose for programmatic access?
     Rate each from 1 (poor) to 5 (excellent). -->

| Interface | Available? | Quality | Notes |
|-----------|-----------|---------|-------|
| Native .NET / COM API | | | |
| REST API | | | |
| Python API | | | |
| Neutral file export (PCF, IFC, STEP, …) | | | |
| Neutral file import | | | |
| Direct DB access (SQL, etc.) | | | |

## Proposed Adapter Architecture

<!-- Which approach would you use?
     - Direct API (preferred)
     - Via neutral format bridge (PCF, IFC, STEP, …)
     - Hybrid

     Example: "E3D: RVM export → rvmparser → PMEF for geometry;
               PML scripting for semantic attributes" -->

## PMEF Domain Coverage

<!-- What PMEF entity types would this adapter support? -->

| PMEF Type | Export (tool → PMEF) | Import (PMEF → tool) | Notes |
|-----------|---------------------|---------------------|-------|
| PipingNetworkSystem | | | |
| PipingComponent | | | |
| EquipmentObject | | | |
| Nozzle | | | |
| SteelObject | | | |
| InstrumentObject | | | |
| Geometry (parametric) | | | |
| Geometry (glTF mesh) | | | |

## Known Limitations / Challenges

<!-- What will be difficult or impossible to map?
     Tool-specific quirks, missing API surface, licensing restrictions. -->

## Contributor Availability

- [ ] I can lead the development of this adapter
- [ ] I can provide test data / access to the tool
- [ ] I can review PRs but not lead development
- [ ] I am proposing this for someone else / the community to pick up

## References

<!-- Tool documentation, existing open-source parsers, prior art. -->
