# Checkpoint archival — SID-A2

- **Date:** 2026-08-19
- **Scope:** `SID-A2-Z`, the administrative closure of
  [`../plans/archive/2026-08-19-after-the-archive-verbs.md`](../plans/archive/2026-08-19-after-the-archive-verbs.md),
  one of three plans archived together — see
  [the shared closure record](2026-08-19-checkpoint-archival.md) for the full
  account
- **Environment:** no build, no test, no phone, no live session; a
  documentation-only move
- **Artifact:** none. No source changed and no version moved

## Procedure

`SID-A2` shipped in full on 2026-08-19 with both its ledger units already
`done`, but the plan stayed under `plans/active/` after the roadmap's active
checkpoint moved on to `SID-A3`. This unit moves it to `plans/archive/` with a
`Closed` date and a `Successor` link to `SID-A3`, alongside the same move for
`SID-G7` and `SID-A1` recorded together in the shared closure record.

## Result

The plan is archived; `scripts/check-documentation-contract.sh` and
`scripts/check-staged-units.py` both pass for the batch.

## Limits

Purely administrative. It does not revisit `SID-A2`'s own implementation or
author-validation record.
