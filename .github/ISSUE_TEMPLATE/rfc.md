---
name: RFC — Request for Comments
about: Propose a normative specification change (new entity type, breaking change, new serialisation rule)
title: '[RFC-NNN] '
labels: ['RFC', 'needs-triage']
assignees: ''
---

<!--
BEFORE SUBMITTING: Check that no existing RFC covers the same ground.
Open RFCs: https://github.com/pmef/specification/issues?q=label%3ARFC+is%3Aopen

RFC numbering: The TSC will assign the final number. Use NNN as placeholder.
Discussion period: Minimum 30 days before merge.
-->

## RFC Title

<!-- Short, descriptive title. Will become RFC-NNN: <title> -->

## Status

`DRAFT` <!-- → REVIEW → APPROVED → REJECTED → SUPERSEDED -->

## Working Group

<!-- Which WG should review this RFC? -->
- [ ] WG-Piping
- [ ] WG-Equipment
- [ ] WG-Steel
- [ ] WG-EI
- [ ] WG-Geometry
- [ ] WG-Adapters
- [ ] WG-Catalogs
- [ ] TSC only

## Abstract

<!-- 2–3 sentences summarising what this RFC proposes and why. -->

## Motivation

<!-- Why is this change needed?
     What use cases does it enable?
     What problems does it solve?
     Which adapter or integration scenario requires it? -->

## Detailed Design

### Schema Changes

<!-- Show the exact JSON Schema additions/modifications:

```jsonc
// schemas/pmef-DOMAIN.schema.json
// BEFORE:
{
  ...
}

// AFTER:
{
  ...
  "newField": {
    "type": "string",
    "description": "..."
  }
}
```
-->

### Serialisation Impact

<!-- Does this change the NDJSON format?
     Show a before/after NDJSON example if applicable. -->

### Upstream Standard Reference

<!-- Which standard motivates this change?
     Cite the exact clause if possible. -->

### Backward Compatibility

<!-- Is this breaking?
     If yes: describe migration path and deprecation period.
     If no: explain why existing instances remain valid. -->

## Alternatives Considered

<!-- What other designs were evaluated? Why was this approach chosen? -->

## Open Questions

<!-- What is still undecided? What feedback is specifically requested? -->

## Implementation Notes

<!-- For adapter implementors: what changes are needed in adapters?
     Estimated implementation complexity: LOW / MEDIUM / HIGH -->

## References

<!-- Links to relevant issues, PRs, standards documents, prior art. -->

---

**Discussion period opens:** <!-- Date TSC marks as REVIEW -->
**Discussion period closes:** <!-- 30 days after REVIEW -->
**Target milestone:** <!-- e.g. v1.0, v1.1 -->
