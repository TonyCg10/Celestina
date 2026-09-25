# SID-U1-B — Corrections after the folder-usage review

- **Opened:** 2026-09-25
- **Plan ID:** folder-usage-fixes
- **Status:** done
- **Closed:** 2026-09-25
- **Successor:** none
- **Authorization:** the final whole-branch review of the folder-usage work,
  run on the deployed 1.7.0 on 2026-09-25, found four defects and the
  coordinator assigned their correction as its own unit
- **Scope:** siderita
- **Implementation checkpoint:** SID-U1-B
- **Author-validation checkpoint:** `VAL-SID-U1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The occupation section keeps every arrow and Enter it receives, hands
Hematita the scanned root before the tree lands, and the quick look takes the
hub back as soon as the properties dialog lets it go.

## Tangible outcome

The installed 1.7.1 opens Hematita on the folder while it is still being
analysed; Up on the first row, Down on the last and Enter on an empty list
no longer step or close the quick look; a failed analysis shows its cause
and no buttons; and after the properties dialog closes, a folder shown in
the quick look is analysed again without stepping away.

## Scope

- `SID-U1-B` — the four defects, and 1.7.1.

## Exclusions

No change to `SideritaUsage`, `UsageSession`, the shared controls or
Hematita. The shared byte formatter stays a separate, unplanned unit.

## Build order

1. `SID-U1-B` alone, after `SID-U1-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 1.7.1.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-U1-B | `siderita:` | done | [inventory](../../inventories/2026-09-25-folder-usage-fixes/SID-U1-B.numstat.tsv) | 12 files, +270/-15 | `FolderUsage.openInHematita` falls back to `usage.root`; the footer hidden while failed; the list-and-map row accepts Up, Down, Return and Enter after its children; `QuickLookView` re-syncs the hub on `ownerChanged` to empty; the non-folder hint in `qsTr()`; three QML tests; 1.7.1 | [folder usage fixes](../../evidence/2026-09-25-folder-usage-fixes.md) | `VAL-SID-U1` |
