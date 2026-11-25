# ARC-2 — Archive the STYLE-G7 plan

- **Opened:** 2026-11-25
- **Plan ID:** style-g7-archive
- **Closed:** 2026-11-25
- **Successor:** none; the author named no next suite checkpoint. Its one row (ARC-2-A) is done
- **Status:** done
- **Authorization:** on 2026-11-25 the author asked to check `STYLE-G7-K` in
  the code and then to archive STYLE-G7, with `STYLE-G7-J` as obsolete and
  `STYLE-G7-K` as delivered. Opened and closed in one commit; a suite unit
  because archived plans of Grafita and Siderita link to STYLE-G7
- **Scope:** suite
- **Implementation checkpoint:** ARC-2
- **Author-validation checkpoint:** none

## Hypothesis

STYLE-G7 stays active only because two rows were never closed by a landing:
`STYLE-G7-K`, whose code is on `main`, and `STYLE-G7-J`, whose consumer is
gone. Recording both and archiving the plan leaves `celestina-style` idle.

## Tangible outcome

[The STYLE-G7 plan](../../../celestina-style/docs/plans/archive/2026-08-04-shared-reading-controls.md)
in its archive with `Closed`, `Successor` and `Status: done`, and both rows
recorded under its "Rows closed outside the ledger".

## Scope

- `ARC-2-A` — the move, the two rows, `celestina-style`'s roadmap, status and
  active-plans index, and every Markdown link to the moved plan outside the
  sealed inventories.

## Exclusions

- Removing the unused `GlassSurface` silhouette API and `ContextualVeil` role.
- MAG-D1 and AND-6, which still hold rows that are not done.

## Build order

1. `ARC-2-A`.

## Implementation exit

The checkpoint closes when `ARC-2-A` is `done`: the documentation guard and
the staged-units check pass on the sealed commit.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| ARC-2-A | `suite:` | done | [inventory](../../inventories/2026-11-25-style-g7-archive/ARC-2-A.numstat.tsv) | 25 files, +536/-339 | Archive the STYLE-G7 plan with its two open rows recorded as delivered and obsolete | [evidence](../../evidence/2026-11-25-style-g7-archive.md) | None |

This plan records intent; it grants no authority.
