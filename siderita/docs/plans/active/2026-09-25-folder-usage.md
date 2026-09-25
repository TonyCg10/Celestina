# SID-U1 — Folder usage

- **Opened:** 2026-09-25
- **Plan ID:** folder-usage
- **Status:** active
- **Authorization:** the author asked on 2026-09-25 for Siderita to show a
  folder's usage with Hematita's projection and controls, planned in
  [the folder usage plan](../../../../docs/superpowers/plans/2026-09-25-siderita-folder-usage.md)
  after [its design](../../../../docs/superpowers/specs/2026-09-25-siderita-folder-usage-design.md)
- **Scope:** siderita
- **Implementation checkpoint:** SID-U1
- **Author-validation checkpoint:** `VAL-SID-U1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The walk, the tree and the projection Hematita's storage section runs are
the right answer to "how big is this folder", and with the treemap and the
usage list shared in `celestina-style` Siderita needs only its own hub and
one composition to show them in both modals that describe a folder.

## Tangible outcome

The properties dialog of a folder and its space-bar quick look show crumbs,
totals, a size-ordered list and a treemap of the folder, scanned once per
open on its own thread and cancelled on close; drilling in and going up need
no second scan; Ctrl+Enter or «Ir a la carpeta» goes there in Siderita, and
«Abrir en Hematita» hands the folder over. The recursive sum the dialog used
is gone.

## Scope

- `SideritaUsage` (`src/usage.rs`): the scan on a `siderita-usage` thread,
  coalesced progress, generation-checked results, path keys both ways.
- `UsageSession` (`src/usage_session.rs`): which scan is current, drill, up,
  projection; unit-tested on real temporary folders.
- `FolderUsage.qml` over the shared `CelestinaTreemap` and
  `CelestinaUsageList`, in `PropertiesDialog.qml` and `QuickLookView.qml`,
  owned by `FolderActions.qml`.
- `properties::directory_size` and the controller's `prop_size_cancel`
  removed; the controller publishes `prop_key` so the dialog can hand the
  hub a path key.

## Exclusions

- Deleting, trashing, moving or renaming from the section; `usage::remove`
  is never linked.
- Duplicates and empty folders beyond the totals.
- The author's own pass on the real session, which is `VAL-SID-U1`.

## Build order

1. `SID-U1-A`, then `SID-U1-Z` (1.7.0, `complete-production.sh`).

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary shows a
folder's occupation in both modals.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-U1-A | `siderita:` | done | [inventory](../../inventories/2026-09-25-folder-usage/SID-U1-A.numstat.tsv) | 25 files, +1992/-115 | `SideritaUsage` and `UsageSession` over `hematita-core::usage`; `FolderUsage.qml` over the shared treemap and usage list in the properties dialog and the quick look; `directory_size` and `prop_size_cancel` removed, `prop_key` published | [folder usage](../../evidence/2026-09-25-folder-usage.md) | `VAL-SID-U1` |
| SID-U1-Z | `siderita:` | planned | pending | pending | Implementation exit, `1.7.0`, documents closed, plan archived | pending | `VAL-SID-U1` |

Like every plan in this repository, this one records intent and grants no
authority.
