# PMEF Governance

## Overview

PMEF is governed as an open, community-driven specification project.
Decisions are made transparently via GitHub issues, discussions, and
recorded votes by the Technical Steering Committee (TSC).

---

## Technical Steering Committee (TSC)

The TSC is responsible for:

- Approving breaking/normative specification changes
- Releasing new versions
- Approving new working groups
- Resolving escalated disputes
- Maintaining the roadmap

**Initial TSC composition (bootstrap phase):** The project founders serve as
the initial TSC. The TSC must expand to at least 5 members from at least 3
different organisations before the v1.0 release.

**TSC member term:** 2 years, renewable. Elections held annually for open seats.

**Quorum:** Majority of active TSC members. A member is "inactive" if they
have not participated in TSC votes for 60 consecutive days.

**TSC decisions:** Lazy consensus (no objection in 7 days) for minor matters;
simple majority vote for significant decisions; 2/3 supermajority for
changes to governance, licensing, or project scope.

---

## Working Groups

Working groups (WGs) operate under the TSC and focus on specific domains:

| WG | Scope | Lead |
|----|-------|------|
| WG-Piping | Piping schema, PCF++, stress interface | TBD |
| WG-Equipment | Equipment subtypes, nozzle model | TBD |
| WG-Steel | Structural steel, CIS/2, profiles | TBD |
| WG-EI | E&I, AutomationML, MTP, OPC UA | TBD |
| WG-Geometry | Primitive library, glTF/USD/STEP | TBD |
| WG-Adapters | Tool adapters, round-trip testing | TBD |
| WG-Catalogs | Catalog format, RDL, eCl@ss | TBD |

**WG formation:** Any contributor can propose a new WG via a GitHub Discussion.
TSC approval required. WGs need at least 3 participants to be chartered.

**WG decisions:** Consensus within the WG; escalate unresolved disputes to TSC.

---

## Roles

| Role | Description | How to become |
|------|-------------|---------------|
| **Contributor** | Anyone who opens issues, PRs, or participates in discussions | Automatic |
| **Committer** | Trusted contributor with merge rights on non-normative changes | Nominated by TSC after sustained contributions |
| **WG Lead** | Chairs a working group, approves WG-scope PRs | Elected by WG members |
| **TSC Member** | Votes on normative changes and governance | Elected by existing TSC |

---

## RFC Process

See [CONTRIBUTING.md — RFC Process](CONTRIBUTING.md#specification-change-process-rfc).

Summary:

1. Open RFC issue → 30-day discussion period
2. WG review and approval
3. TSC approval for merging
4. Recorded in CHANGELOG.md with RFC number

---

## Versioning Policy

PMEF follows **Semantic Versioning 2.0**:

- **PATCH** (`0.9.x`): Non-normative fixes (typos, clarifications, new examples)
- **MINOR** (`0.x.0`): Backward-compatible additions (new optional fields, new entity types)
- **MAJOR** (`x.0.0`): Breaking changes (required field additions, entity removals, serialisation format changes)

Breaking changes require a TSC supermajority vote and a minimum 6-month
deprecation period with migration guide.

---

## Code of Conduct Enforcement

The TSC is the enforcement body for the [Code of Conduct](CODE_OF_CONDUCT.md).
Reports can be sent to [conduct@pmef.net](mailto:conduct@pmef.net) (private,
handled by a rotating 3-person CoC committee, none of whom may be the subject
of the report).

---

## Amendment

This governance document may be amended by a TSC supermajority vote (2/3)
with at least 14 days of public notice before the vote.
