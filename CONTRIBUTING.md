# Contributing to PMEF

Thank you for your interest in contributing to the Plant Model Exchange Format.
PMEF is a community-driven open standard — every contribution matters.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Ways to Contribute](#ways-to-contribute)
- [Development Setup](#development-setup)
- [Contribution Workflow](#contribution-workflow)
- [Schema Contribution Guidelines](#schema-contribution-guidelines)
- [Specification Change Process (RFC)](#specification-change-process-rfc)
- [Working Groups](#working-groups)
- [Commit Convention](#commit-convention)
- [Review Criteria](#review-criteria)

---

## Code of Conduct

All contributors must follow our [Code of Conduct](CODE_OF_CONDUCT.md).
We are committed to a welcoming, inclusive, and harassment-free community.

---

## Ways to Contribute

| Contribution type | Where to start |
|------------------|----------------|
| 🐛 **Bug report** in schema or example | [Bug report template](.github/ISSUE_TEMPLATE/bug_report.md) |
| 💡 **Feature suggestion** | [Feature request template](.github/ISSUE_TEMPLATE/feature_request.md) |
| 📝 **Schema change / new property** | [RFC template](.github/ISSUE_TEMPLATE/rfc.md) → PR |
| 🔧 **Fix typo / improve docs** | Direct PR — no RFC needed for non-normative changes |
| 🗺️ **New adapter specification** | RFC + separate adapter repo under `pmef/` org |
| 🧪 **New example / benchmark data** | PR to `examples/` — CC0 licence required |
| 🌐 **Translation** | Contact [i18n working group](https://github.com/pmef/specification/discussions) |
| 🏗️ **Working group participation** | See [Working Groups](#working-groups) |

---

## Development Setup

### Prerequisites

```bash
# Required
python >= 3.10          # schema validation scripts
node >= 18              # Mermaid diagram rendering (optional)

# Optional but recommended
rustup                  # if contributing to reference implementation
docker                  # for CI reproduction
```

### Clone and validate locally

```bash
git clone https://github.com/pmef/specification.git
cd specification

# Install Python validation dependencies
pip install jsonschema check-jsonschema

# Validate all schemas
python scripts/validate-schemas.py

# Validate all examples against their schemas
python scripts/validate-examples.py

# Render Mermaid diagrams (requires Node + @mermaid-js/mermaid-cli)
npm install -g @mermaid-js/mermaid-cli
python scripts/render-diagrams.py
```

### Run the full CI check locally

```bash
# Mirrors what GitHub Actions runs
python scripts/ci-local.py
```

---

## Contribution Workflow

```
1. Open / find an issue  →  discuss scope and approach
2. Fork the repository
3. Create a feature branch:  git checkout -b feat/your-topic
4. Make changes (follow guidelines below)
5. Run local validation:  python scripts/validate-schemas.py
6. Commit with conventional commit message
7. Push and open a Pull Request using the PR template
8. Address review feedback
9. Merge after approval (requires 2 reviewers for normative changes)
```

### Branch naming

| Type | Pattern | Example |
|------|---------|---------|
| Feature / new content | `feat/<topic>` | `feat/add-hvac-domain` |
| Bug fix | `fix/<topic>` | `fix/pump-spec-required-fields` |
| RFC / spec change | `rfc/<number>-<topic>` | `rfc-007-steel-profiles` |
| Documentation | `docs/<topic>` | `docs/improve-getting-started` |
| CI / tooling | `chore/<topic>` | `chore/update-validators` |

---

## Schema Contribution Guidelines

### When do you need an RFC?

| Change type | RFC needed? |
|-------------|------------|
| Adding a new top-level entity type | ✅ Yes |
| Adding required fields to existing entity | ✅ Yes — breaking change |
| Adding optional fields to existing entity | 🟡 Recommended for large additions |
| Fixing a typo in a description | ❌ No — direct PR |
| Adding a new enum value | 🟡 Recommended |
| Adding a new example | ❌ No — direct PR |

### Schema quality requirements

All schema changes must:

1. **Be valid JSON Schema Draft 2020-12** — validated by CI
2. **Include `title` and `description`** on every property
3. **Specify units in the description** for numeric fields (e.g. `[Pa]`, `[mm]`, `[K]`)
4. **Reference an upstream standard** where applicable (CFIHOS attribute name, ISO 15926 class, IEC code)
5. **Have at least one passing example** in `examples/` that exercises the new field
6. **Not break existing valid instances** (new required fields are always RFC-level changes)
7. **Use `additionalProperties: false`** on all object definitions to enable strict validation

### Property naming convention

```
camelCase                        # standard
nominalDiameter                  # quantity: "nominal" prefix for design values
designPressure                   # design envelope value
operatingTemperature             # normal operating value
rdlTypeUri   / rdlType           # ISO 15926 RDL reference
iec81346...                      # IEC 81346 designation fields
cfihos...                        # CFIHOS-specific attributes
```

### Enum value convention

```
SCREAMING_SNAKE_CASE             # all enum values
CENTRIFUGAL_PUMP                 # descriptive, not abbreviated
SHELL_AND_TUBE                   # full name preferred
SCH40                            # industry abbreviations OK for well-known codes
```

---

## Specification Change Process (RFC)

For normative changes (new entity types, breaking schema changes, new
serialisation rules), use the RFC process:

1. **Open an RFC issue** using the [RFC template](.github/ISSUE_TEMPLATE/rfc.md)
2. **Discussion period**: minimum 30 days open for community comment
3. **Draft PR**: submit schema/spec changes referencing the RFC issue
4. **Working group review**: relevant WG must approve (see below)
5. **TSC sign-off**: Technical Steering Committee approves merge
6. **Merge and changelog**: RFC number recorded in CHANGELOG.md

RFC numbering: sequential integers, prefixed `RFC-NNN`.
Current RFCs: see [open RFC issues](../../issues?q=label%3ARFC+is%3Aopen).

---

## Working Groups

| WG | Scope | Meets |
|----|-------|-------|
| **WG-Piping** | Piping domain schema, PCF++, stress analysis interface | Bi-weekly Thu |
| **WG-Equipment** | Equipment subtypes, nozzle model, CFIHOS alignment | Bi-weekly Fri |
| **WG-Steel** | Structural steel, CIS/2 mapping, profile catalogs | Monthly |
| **WG-EI** | E&I schema, AutomationML/AML, MTP, OPC UA | Bi-weekly Wed |
| **WG-Geometry** | Parametric primitives, glTF/USD/STEP geometry layers | Monthly |
| **WG-Adapters** | Tool-specific adapters, round-trip testing | Monthly |
| **WG-Catalogs** | Open catalog format, RDL resolution, eCl@ss | Monthly |

Join via [GitHub Discussions](https://github.com/pmef/specification/discussions)
or [Discord #working-groups](https://discord.gg/pmef).

---

## Commit Convention

PMEF uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>

[optional body]

[optional footer: closes #issue]
```

| Type | Use for |
|------|---------|
| `feat` | New schema entity, property, or example |
| `fix` | Correction to existing schema or example |
| `spec` | Normative specification text change |
| `docs` | Non-normative documentation |
| `chore` | CI, tooling, dependency updates |
| `rfc` | RFC proposal or resolution |
| `refactor` | Schema restructuring without semantic change |

**Examples:**

```
feat(piping): add EccentricFlat field to Reducer schema

fix(equipment): correct VesselDesign headType enum — add FLAT

spec(serialisation): clarify NDJSON line-length recommendation

rfc: open RFC-008 — HVAC domain initial schema

chore(ci): pin jsonschema to 4.21.1 for reproducible builds
```

---

## Review Criteria

Pull requests are evaluated on:

| Criterion | Detail |
|-----------|--------|
| **CI passes** | All schema validations and example checks green |
| **Spec alignment** | Change consistent with normative spec text |
| **Standard grounding** | New properties reference upstream standard (CFIHOS, ISO 15926, etc.) |
| **Backward compatibility** | No breaking changes without RFC and major version bump |
| **Example coverage** | New normative fields exercised in at least one example |
| **Description quality** | All new fields have clear title + description + units |
| **RFC process followed** | For normative changes: RFC open ≥ 30 days, WG approval noted |

### Approval requirements

| Change type | Required approvals |
|-------------|-------------------|
| Typo / non-normative docs | 1 maintainer |
| Optional schema field | 2 maintainers |
| Required schema field / new type | 2 maintainers + relevant WG lead |
| Breaking change | TSC vote (majority) |

---

## Recognition

All contributors are listed in [CONTRIBUTORS.md](CONTRIBUTORS.md).
Significant contributions are highlighted in release notes.

Thank you for helping build the open standard the plant engineering industry needs.
