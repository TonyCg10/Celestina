# The storage analysis resetting on a section change — S3-C

- **Date:** 2026-09-25
- **Scope:** `S3-C` of
  [`../plans/archive/2026-09-25-s3-section-persistence.md`](../plans/archive/2026-09-25-s3-section-persistence.md):
  the section-change hook and the details default
- **Environment:** the author's checkout; offscreen Qt platform, no session
  bus
- **Artifact:** `hematita/target/release/hematita` 1.2.2 built, verified and
  deployed by `hematita/scripts/complete-production.sh`

## Procedure

1. `qml/Main.qml`: `onCurrentSectionChanged` calls `analysisHub.open()` only
   when section 5 is shown and `analysisHub.mode === "locations"`; in
   `browsing`, `scanning` or `analysed` it does nothing. The two paths that
   hand a folder (`onOpenRequested` and the start folder) still set the
   section and then call `openPath`, which needs no prior `open()`.
2. `qml/components/StoragePage.qml`: `detailsShown` defaults to `true`.
   Read-through of the page's remaining local state: the `StackLayout` keeps
   every page alive, `onVisibleChanged` only calls `weave()`, which rebuilds
   rows only when the folder or usage key changed, so `currentId` and
   `detailsShown` survive a section change without further machinery.
3. `python3 scripts/version_tool.py bump hematita bug --unit S3-C` moved
   1.2.1 to 1.2.2.
4. `bash hematita/scripts/complete-production.sh`, once.
5. `bash scripts/check-architecture-contract.sh`,
   `bash scripts/check-documentation-contract.sh`,
   `python3 scripts/check-language-contract.py`,
   `python3 scripts/version_tool.py check`.

## Result

`complete-production.sh` exited 0: build, 188 tests, `qmllint-production`
(0 non-fatal baseline warnings) and the offscreen smoke (every section shown,
the storage page listed locations on its first visit, a folder argument
landed in the storage section, no QML errors) passed; the verified artifact
was deployed to `~/.local` and `status-production.sh` reports it current and
installed. The guards passed.

## Limits

The smoke walks each section once and never returns to storage after a scan,
so the round trip itself is not automated; it is in `VAL-S3` in the author's
lane, together with the mount refresh from the locations zone.
