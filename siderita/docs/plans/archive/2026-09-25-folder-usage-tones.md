# SID-U1-C — Rank tones in the folder occupation

- **Opened:** 2026-09-25
- **Plan ID:** folder-usage-tones
- **Status:** done
- **Closed:** 2026-09-25
- **Successor:** none
- **Authorization:** the author saw the occupation section on 1.7.1 and asked
  for a distinct colour per entry and clean corners; the coordinator assigned
  the Siderita side as its own unit after `celestina-style` 1.9.1 (c3be51e)
  added `CelestinaTheme.usagePalette` and fixed the shared corner geometry
- **Scope:** siderita
- **Implementation checkpoint:** SID-U1-C
- **Author-validation checkpoint:** `VAL-SID-U1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

Every row of the occupation section and its tile paint `dir` or `file`, so
most of the map is one colour; mapping the size rank to the shared palette
gives neighbours distinct hues while a row and its tile stay paired.

## Tangible outcome

The installed 1.7.2 paints each row's bar and its tile with the same one of
six palette tones, rotating by size rank; a row with unreadable folders below
stays faint and the merged remainder stays neutral.

## Scope

- `SID-U1-C` — the rank tones in `FolderUsage.qml`, two QML tests, and 1.7.2.

## Exclusions

No change to the shared controls, `CelestinaTheme`, `SideritaUsage` or
Hematita. The corner geometry arrives through the `celestina-style` symlinks.

## Build order

1. `SID-U1-C` alone, after `SID-U1-B` and `celestina-style` 1.9.1.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 1.7.2.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-U1-C | `siderita:` | done | [inventory](../../inventories/2026-09-25-folder-usage-tones/SID-U1-C.numstat.tsv) | 11 files, +194/-8 | `FolderUsage.toneColors` maps `p0`..`p5` to `CelestinaTheme.usagePalette` and drops `dir`/`file`; `weave` gives a row `unreadable` when it has unreadable folders below, `other` when it is the merged remainder, else `"p" + (index % 6)`, and the tile inherits it; two QML tests; 1.7.2 | [folder usage tones](../../evidence/2026-09-25-folder-usage-tones.md) | `VAL-SID-U1` |
