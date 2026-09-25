# S3-C — The storage analysis resetting on a section change

- **Opened:** 2026-09-25
- **Plan ID:** s3-section-persistence
- **Status:** done
- **Closed:** 2026-09-25
- **Successor:** none
- **Authorization:** the author reported on 2026-09-25 that switching to
  another section and back lost the storage analysis and its settings
  (folder, scan, filters, details), and asked for the details card to be on
  by default
- **Scope:** hematita
- **Implementation checkpoint:** S3-C
- **Author-validation checkpoint:** `VAL-S3` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

`Main.qml` calls `analysisHub.open()` every time section 5 is shown, and
`open()` resets the session, the stack and the mode. Calling it only while
the hub is on the locations zone keeps the mount refresh and leaves any
folder state alone; the page itself stays alive inside the `StackLayout`, so
its QML state already persists.

## Tangible outcome

The installed 1.2.2 keeps the browsed folder, a scan in progress or its
result, the filters, the selection, the cursor and the details card across a
section change, and shows the details card from the first visit.

## Scope

- `S3-C` — the section-change guard in `Main.qml`, `detailsShown: true` in
  `StoragePage.qml`, and 1.2.2.

## Exclusions

No change to `analysis.rs`, `hematita-core`, `celestina-style` or the shared
controls.

## Build order

1. `S3-C` alone, after `S3-B`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the deployed binary is 1.2.2.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| S3-C | `hematita:` | done | [inventory](../../inventories/2026-09-25-s3-section-persistence/S3-C.numstat.tsv) | 13 files, +164/-11 | `onCurrentSectionChanged` calls `analysisHub.open()` only when `analysisHub.mode === "locations"`; `StoragePage.detailsShown` defaults to `true`; 1.2.2 | [section persistence](../../evidence/2026-09-25-s3-section-persistence.md) | `VAL-S3` |
