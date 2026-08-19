# Checkpoint archival — SID-G7, SID-A1, SID-A2

- **Date:** 2026-08-19
- **Scope:** `SID-G7-Z`, the administrative closure of
  [`../plans/archive/2026-08-04-shared-reading-surface.md`](../plans/archive/2026-08-04-shared-reading-surface.md),
  and the same move for `SID-A1` and `SID-A2`
- **Environment:** no build, no test, no phone, no live session; a
  documentation-only move verified with
  `scripts/check-documentation-contract.sh` and `scripts/check-staged-units.py`
- **Artifact:** none. No source changed and no version moved

## Procedure

`SID-G7`, `SID-A1` and `SID-A2` each already shipped in their own milestone
commit, weeks or days apart, with every ledger unit in their plan already
`done`. None of the three plan documents was ever moved to `plans/archive/`
when its checkpoint closed, so all three sat under `plans/active/` with
`Status: active` at once — after `SID-A3` had already become the roadmap's
own declared active checkpoint. That is the exact state
`docs/plans/README.md` describes as requiring `../archive/`.

Each plan moved with its basename, checkpoint, Plan ID, units and links
intact, gained a `Closed` date matching the date its shipping commit actually
landed, and a `Successor` link completing the chain: `SID-G7` to `SID-A1`, to
`SID-A2`, to the already-valid `SID-A3`. Every stale
`docs/plans/active/<name>.md` reference this repository still held — ROADMAP
prose links, evidence `Scope` lines, `VALIDATION.md`, `plans/active/README.md`,
and one cross-project reference from `celestina-style` and `grafita` — was
repointed to the new location. No implementation, evidence or validation
content changed.

## Result

`scripts/check-documentation-contract.sh` reports zero blocking errors
repository-wide (the pre-existing `SID-G7-D` erratum, itself unrelated and
already present before this move, is informational and not fatal). This
inventory and administrative unit satisfy `scripts/check-staged-units.py`'s
requirement that an archiving commit carry one.

## Limits

Purely administrative. It does not revisit, re-run or re-validate any of the
three checkpoints' own implementation or author-validation record.
