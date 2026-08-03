# Grafita author validation

This manual lane does not contain implementation and does not block
[ROADMAP.md](ROADMAP.md). Grafita has no currently requested pending author
validation.

## Closed historical observations

`VAL-GRA-EMBEDDED`, `VAL-GRA-STANDALONE` and `VAL-GRA-COMFORT` are preserved in
the [migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).

## VAL-GRA-METADATA — Cross-owner and extended-attribute save

- **Status:** deferred
- **Related implementation:** current loss-free save contract
- **Requires:** a disposable real file owned by another user or a non-temporary
  filesystem carrying representative ACLs and extended attributes
- **Procedure:** edit and save through a consuming Grafita surface, then compare
  owner, group, mode, ACL and extended attributes with the original fixture
- **Pass condition:** every declared metadata field survives unchanged, or the
  save refuses before replacing the original
- **Result:** deferred until a relevant filesystem fixture exists
- **Evidence:** none

## Coverage intentionally outside the current plan

IME, AT-SPI and reduced-motion were explicitly set aside in the version-1
record. They are not pending milestones. If the author requests one, add a
bounded `VAL-GRA-*` case here; a failure then opens a separate corrective
implementation unit.
