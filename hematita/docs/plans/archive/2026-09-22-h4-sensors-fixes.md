# H4-D — The Sensors page's keyboard, its failure state and its gate

- **Opened:** 2026-09-22
- **Plan ID:** h4-sensors-fixes
- **Status:** done
- **Closed:** 2026-09-22
- **Successor:** H5
- **Authorization:** the whole-branch review of `H4` came back "With fixes" on
  2026-09-22 and the author's coordinator assigned the wave as its own unit
- **Scope:** hematita
- **Implementation checkpoint:** H4-D
- **Author-validation checkpoint:** `VAL-H4` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

The Sensors page is one Tab stop whose arrows scroll, a failed tick shows only
its reason, and the smoke proves the page has chips.

## Tangible outcome

The installed 0.5.1 crosses Sensores the way the rest of the application is
crossed — one stop for the page, arrows card by card, no focus stranded where
nothing can scroll to it — shows only the reason when `/sys/class/hwmon`
cannot be read rather than the last good values beside an error line, and its
smoke fails if the page ever publishes no chip.

## Scope

- `H4-D` — the eight findings of the `H4` whole-branch review that the
  controller did not defer, and 0.5.1.

## Exclusions

Deferred by the controller and not attempted here: the chip's static files
are never re-enumerated while its name is unchanged (a chip that gains a
channel is not noticed until the name changes), power channels are never
graded (only temperatures carry a load), and one unreadable tick still erases
a channel's session extremes. All three are `H5`.

## Build order

1. `H4-D` alone, after `H4-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 0.5.1.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H4-D | `hematita:` | done | [inventory](../../inventories/2026-09-22-h4-sensors-fixes/H4-D.numstat.tsv) | 18 files, +416/-70 | The page a `ListView` that is the one Tab stop with arrows by card and rows that take no focus; a failed tick publishing empty lists so only its reason shows; a smoke gate on the page's chip and channel counts; the extremes retain by set; the row's name including its extremes; a null-prototype ordinal map; every chip of a repeated driver numbered; the search applied on `accepted`; 0.5.1 | [sensors fixes](../../evidence/2026-09-22-h4-sensors-fixes.md) | `VAL-H4` |
