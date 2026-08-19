# Checkpoint archival — SID-A1

- **Date:** 2026-08-19
- **Scope:** `SID-A1-Z`, the administrative closure of
  [`../plans/archive/2026-08-18-archive-compression.md`](../plans/archive/2026-08-18-archive-compression.md),
  one of three plans archived together — see
  [the shared closure record](2026-08-19-checkpoint-archival.md) for the full
  account
- **Environment:** no build, no test, no phone, no live session; a
  documentation-only move
- **Artifact:** none. No source changed and no version moved

## Procedure

`SID-A1` shipped in full on 2026-08-18 with both its ledger units already
`done`, but the plan stayed under `plans/active/` after the roadmap's active
checkpoint moved on. This unit moves it to `plans/archive/` with a `Closed`
date and a `Successor` link to `SID-A2`, alongside the same move for `SID-G7`
and `SID-A2` recorded together in the shared closure record.

## Result

The plan is archived; `scripts/check-documentation-contract.sh` and
`scripts/check-staged-units.py` both pass for the batch.

## Limits

Purely administrative. It does not revisit `SID-A1`'s own implementation or
author-validation record.
