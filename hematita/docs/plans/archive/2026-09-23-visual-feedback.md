# VIS-1 — The first real-session visual and naming defects

- **Opened:** 2026-09-23
- **Plan ID:** visual-feedback
- **Status:** done
- **Closed:** 2026-09-23
- **Successor:** none
- **Authorization:** the author looked at the deployed 0.6.1 on the real
  session for the first time on 2026-09-23, reported the defects with
  screenshots, and the coordinator assigned the correction as its own unit
- **Scope:** hematita
- **Implementation checkpoint:** VIS-1
- **Author-validation checkpoint:** `VAL-VIS-1` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

The first real-session look found layout, colour and naming defects that one
unit closes without changing any contract.

## Tangible outcome

The installed 0.6.2 centres the strip's items, keeps every sparkline inside
its rounded background, colours each resource and each sensor kind in its own
theme accent, gives the Processes and Services pages a bar on the canvas and
one inset card holding the titles and the rows, shows a path-named process by
its last segment, leads a unit with its description, words the known hwmon
labels, drops the kernel's sentinel limits, and tells a measured idle disk
rate from one that cannot be read.

## Scope

- `VIS-1` — the nine defects of the author's first look, and 0.6.2.

## Exclusions

No new page, no new published contract beyond the additive
`processDisplayNames` list; no change to the load thresholds, the sampler's
cadence or the privilege path.

## Build order

1. `VIS-1` alone, after `H5-D`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 0.6.2.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| VIS-1 | `hematita:` | done | [inventory](../../inventories/2026-09-23-visual-feedback/VIS-1.numstat.tsv) | 29 files, +850/-132 | the strip's item centred; the sparkline drawn in a clipped inset; a trace colour per resource kind and a value colour per sensor kind, each overridden by the load; the Processes and Services bar on the canvas with spaced capsules and one inset card with a section-label header and a hairline; `process_view::display_name`; the unit description leading; the hwmon label dictionary; `sensors::plausible_limit` and its four ceilings; "límite" and ungrouped readings; `NO_RATE` for an unreadable disk rate; the hand-off failure on stderr; 0.6.2 | [visual feedback](../../evidence/2026-09-23-visual-feedback.md) | `VAL-VIS-1` |
