## Summary

<!-- What does this PR do? Link to the related issue: "Closes #NNN" -->

Closes #

## Type of change

- [ ] 🐛 Bug fix (non-normative correction)
- [ ] 📝 Documentation / spec text improvement (non-normative)
- [ ] ✨ New optional schema field(s)
- [ ] 🏗️ New entity type (requires WG approval)
- [ ] ⚠️ Breaking change (requires RFC + TSC approval)
- [ ] 🧪 New example / benchmark data
- [ ] 🔧 Tooling / CI change
- [ ] 📋 RFC implementation

## RFC / Issue reference

- Related RFC: <!-- RFC-NNN if applicable, or "N/A" -->
- WG approval: <!-- link to WG meeting notes or comment, or "N/A" -->

---

## Schema Checklist

*(Complete this section for any schema changes)*

- [ ] All new properties have `title` AND `description`
- [ ] Units are specified in the description for numeric fields (e.g. `[mm]`, `[Pa]`, `[K]`)
- [ ] Upstream standard reference included (CFIHOS / ISO 15926 / IEC 81346 / API / etc.)
- [ ] `additionalProperties: false` on all new object definitions
- [ ] New `enum` values use `SCREAMING_SNAKE_CASE`
- [ ] `$id` updated if adding a new top-level schema file
- [ ] No new required fields added without RFC (breaking change)
- [ ] At least one example exercises the new field(s)

## Validation Checklist

- [ ] `python scripts/validate-schemas.py` passes locally
- [ ] `python scripts/validate-examples.py` passes locally
- [ ] No existing valid instances broken by this change

## Example Checklist

*(Complete this section for new or modified examples)*

- [ ] Example is syntactically valid NDJSON (one object per line, no comments in production lines)
- [ ] Example validates against the relevant schema(s)
- [ ] Example uses realistic engineering values (correct units, realistic dimensions)
- [ ] Example is licensed CC0 (no proprietary project data)
- [ ] Example header comment clearly describes what it demonstrates

## Documentation Checklist

- [ ] CHANGELOG.md updated under `[Unreleased]`
- [ ] Relevant diagram(s) updated if schema structure changed
- [ ] `design-decisions.md` updated if a new architectural decision was made

---

## Notes for Reviewers

<!-- Anything the reviewer should pay special attention to.
     Which parts are you uncertain about?
     What alternative approaches did you consider? -->
