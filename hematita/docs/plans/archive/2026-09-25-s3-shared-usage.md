# S3 — Shared usage view and the folder argument

- **Opened:** 2026-09-25
- **Closed:** 2026-09-25
- **Successor:** none
- **Plan ID:** s3-shared-usage
- **Status:** done
- **Authorization:** the author asked on 2026-09-25 for Siderita to show a
  folder's usage with Hematita's projection and controls, planned in
  [the folder usage plan](../../../../docs/superpowers/plans/2026-09-25-siderita-folder-usage.md)
  after [its design](../../../../docs/superpowers/specs/2026-09-25-siderita-folder-usage-design.md)
- **Scope:** hematita
- **Implementation checkpoint:** S3
- **Author-validation checkpoint:** `VAL-S3` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

A second consumer proves the projection and the two controls are shared
anatomy, and a folder handed on the command line lands in the storage
section.

## Tangible outcome

Hematita draws the storage page with `CelestinaTreemap` and
`CelestinaUsageList` from `celestina-style`, takes its rows and treemap from
`hematita-core::usage::view`, and `hematita FOLDER` (or D-Bus `Open`, or the
desktop entry's `%f`) opens the storage section browsing that folder.

## Scope

- `S3-A` — `hematita-core::usage::view` (`children_rows`, `flat_rects` over
  `Option<NodeId>`, `unreadable_below`, `REMAINDER_ID`, `MAX_ROWS`), the app's
  copies removed; the page's own `Treemap.qml` and `UsageList.qml` replaced
  by the shared controls, Space and Delete handled by the page; the folder
  argument through `main.rs`, `activation.rs` (`Open`), `analysis.rs`
  (`openPath`), `Main.qml` and the desktop entry; the smoke's start gate;
  S3 opened in the documents.
- `S3-Z` — implementation exit.

## Exclusions

- Siderita's own use of the view and the controls (its own plan); new
  storage features; any change to the deletion.

## Build order

1. `S3-A`, then `S3-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds; the installed binary draws the
shared controls and opens a folder handed on the command line.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| S3-A | `hematita:` | done | [inventory](../../inventories/2026-09-25-s3-shared-usage/S3-A.numstat.tsv) | 23 files, +780/-518 | `hematita-core::usage::view` with the app's copies removed; `CelestinaTreemap` and `CelestinaUsageList` symlinked and consumed, the own `Treemap.qml` and `UsageList.qml` deleted, Space and Delete on the page; `openPath`, D-Bus `Open(s)`, `startPath`, `Exec=hematita %f`; the smoke's start gate | [shared usage](../../evidence/2026-09-25-s3-shared-usage.md) | `VAL-S3` |
| S3-Z | `hematita:` | done | [inventory](../../inventories/2026-09-25-s3-shared-usage/S3-Z.numstat.tsv) | 12 files, +225/-83 | implementation exit and 1.2.0 | [production completion](../../evidence/2026-09-25-s3-production-completion.md) | `VAL-S3` |
