# H3-E — The process table's cursor, outcome and signal

- **Opened:** 2026-09-22
- **Plan ID:** h3-table-fixes
- **Status:** done
- **Closed:** 2026-09-22
- **Successor:** H4
- **Authorization:** the whole-branch review of `H3` came back "With fixes" on
  2026-09-22 and the author's coordinator assigned the wave as its own unit
- **Scope:** hematita
- **Implementation checkpoint:** H3-E
- **Author-validation checkpoint:** `VAL-H3` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

The table's cursor and selection stay one thing across ticks, and the signal
path re-validates its target.

## Tangible outcome

The installed 0.4.1 keeps the selected row under the cursor while the numbers
move, while the search narrows the list and while an application folds; it
tells the truth about the last terminate or kill until the selection moves;
it will not signal a PID that stopped being the process the table showed; and
its columns, its sort header and its folds are reachable from the keyboard at
the minimum window width.

## Scope

- `H3-E` — the ten findings of the `H3` whole-branch review that the
  controller did not defer, and 0.4.1.

## Exclusions

Deferred by the controller and not attempted here: the subscribers lock held
across the publish, the double weave on the page that is not showing, a
debounce on the search field, telling a refused `io` read apart from a zero
rate, the per-tick snapshot clones, and the `revision` wrap.

## Build order

1. `H3-E` alone, after `H3-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 0.4.1.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H3-E | `hematita:` | done | [inventory](../../inventories/2026-09-22-h3-table-fixes/H3-E.numstat.tsv) | 17 files, +546/-46 | Cursor re-anchored wherever the entries are rebuilt; the outcome cleared only by a new selection or a new action; the signal path re-reads `/proc` before acting; the list the one Tab stop; keyboard folds; the user column sortable; columns that fit the minimum width; a sampler hub that can start again; accessible checkable state; unavailability announced before a row's note; 0.4.1 | [table fixes](../../evidence/2026-09-22-h3-table-fixes.md) | `VAL-H3` |
