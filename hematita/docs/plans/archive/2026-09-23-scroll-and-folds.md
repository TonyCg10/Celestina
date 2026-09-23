# VIS-2 — The table scrolling back on refresh, and applications opening folded

- **Opened:** 2026-09-23
- **Plan ID:** scroll-and-folds
- **Status:** done
- **Closed:** 2026-09-23
- **Successor:** none
- **Authorization:** the author reported both defects on the deployed 0.6.2
  on 2026-09-23 while running `VAL-VIS-1`, and the coordinator assigned the
  correction as its own unit
- **Scope:** hematita
- **Implementation checkpoint:** VIS-2
- **Author-validation checkpoint:** `VAL-VIS-2` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

A rebuilt list keeps its viewport, only the person scrolls it, and
applications open folded.

## Tangible outcome

The installed 0.6.3 keeps the process, application, service and sensor
lists where the person left them across every refresh, moves the view only
when the person moves the cursor, and shows every application folded until
it is opened.

## Scope

- `VIS-2` — the two defects, and 0.6.3.

## Exclusions

No change to the published contracts, the integer models, the sampler or
the privilege path.

## Build order

1. `VIS-2` alone, after `VIS-1`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 0.6.3.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| VIS-2 | `hematita:` | done | [inventory](../../inventories/2026-09-23-scroll-and-folds/VIS-2.numstat.tsv) | 14 files, +285/-15 | the viewport offset restored after every rebuild of the process, service and sensor lists, clamped to the new length; `highlightFollowsCurrentItem: false` with `positionViewAtIndex(..., ListView.Contain)` only on a cursor move the person made; `ProcessTable.expanded` replacing `collapsed`, pruned on weave; 0.6.3 | [scroll and folds](../../evidence/2026-09-23-scroll-and-folds.md) | `VAL-VIS-2` |
