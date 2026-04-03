---
name: Feature Request
about: Suggest a new entity type, property, adapter, or other addition
title: '[FEAT] '
labels: ['enhancement', 'needs-triage']
assignees: ''
---

## Summary

<!-- One sentence: what should be added or changed? -->

## Domain / Scope

<!-- Which part of PMEF does this affect? -->
- [ ] Piping domain schema
- [ ] Equipment domain schema
- [ ] E&I / Instrumentation schema
- [ ] Structural Steel schema
- [ ] Geometry primitives
- [ ] Property sets
- [ ] Serialisation format
- [ ] New adapter: ___ (tool name)
- [ ] Catalog / RDL
- [ ] Specification text
- [ ] Tooling / CI
- [ ] Documentation

## Problem Statement

<!-- What problem does this solve? Who is affected?
     "As a [role], I need [capability] so that [benefit]." -->

## Proposed Solution

<!-- Describe the change. For schema additions, include a draft:

```jsonc
{
  "newField": {
    "type": "string",
    "description": "What this field means [unit if applicable]"
  }
}
``` -->

## Upstream Standard Basis

<!-- Does this map to an existing standard?
     - CFIHOS attribute: ___
     - ISO 15926 class: ___
     - IEC 81346 code: ___
     - PCF field: ___
     - API standard: ___
     - Other: ___ -->

## Alternatives Considered

<!-- What else did you consider? Why is this approach preferred? -->

## Backward Compatibility

- [ ] This is a purely additive change (new optional field / new entity type)
- [ ] This modifies an existing field (potential breaking change → RFC required)
- [ ] This adds a required field (breaking change → RFC required)

## Priority / Urgency

<!-- Why does this matter for the v1.0 milestone? Is it blocking an adapter? -->

## Additional Context

<!-- Screenshots, links to relevant standards, adapter requirements, etc. -->
