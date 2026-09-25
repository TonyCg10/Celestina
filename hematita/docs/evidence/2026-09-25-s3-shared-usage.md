# The shared usage view and the folder argument — S3-A

- **Date:** 2026-09-25
- **Scope:** `S3-A` of
  [`../plans/active/2026-09-25-s3-shared-usage.md`](../plans/active/2026-09-25-s3-shared-usage.md):
  `hematita-core::usage::view`, the Storage page's list and map, the folder
  argument
- **Environment:** the author's checkout; offscreen Qt platform, no session
  bus
- **Artifact:** `hematita/target/release/hematita` built by
  `hematita/scripts/build-production.sh` and verified by
  `hematita/scripts/verify-production.sh`; not deployed (`S3-Z` does)

## What changed

1. **The projection in the crate.** `hematita-core::usage::view` (new) holds
   `unreadable_below` and `flat_rects`, moved from `hematita/src/analysis_view.rs`
   with their tests, and adds `children_rows` (a folder's children biggest
   first; past `limit` the rest merge into one remainder row that keeps
   their sums, `id: None`, `merged` counting them), `Row`, `MAX_ROWS` and
   `REMAINDER_ID`. `flat_rects` now takes `&[Option<NodeId>]`: `None` and
   squarify's own remainder both carry `REMAINDER_ID`. Hematita's session
   wraps its children in `Some` and calls the crate; `findings()` imports
   `unreadable_below` from it. No copy is left in `hematita/src`.
2. **The shared controls.** `qml/components/Treemap.qml` and
   `qml/components/UsageList.qml` are deleted;
   `qml/CelestinaTreemap.qml` and `qml/CelestinaUsageList.qml` are
   symlinks to `celestina-style` registered in `build.rs`'s shared block.
   The page gives each row `detail` ("%1 copias" when the entry has
   candidate copies, else empty), each tile `kind`, and replaces the tiles'
   `matches` field with `page.dimmedIds`: the ids of tiles the filter left
   out of the list, plus `-1` for the remainder whenever a filter is on.
   The list's empty sentence comes through `emptyText`.
3. **Space and Delete.** The shared controls handle only the arrows,
   Enter and Backspace, and leave Space and Delete unaccepted. The list's
   focus sits on a plain slot item, not on the row button, so Space is no
   longer consumed as the button's click (the old `UsageList`'s own Space
   handler very likely never fired for that reason). The unaccepted key
   travels up the item parents to the `RowLayout` that holds both the list
   column and the map, where the page's `Keys.onSpacePressed` toggles
   `page.currentId` and `Keys.onPressed` turns Delete into
   `requestTrash()` when there is a selection. `DuplicateList` keeps
   accepting its own Space and Delete, so nothing fires twice.
4. **The folder argument.** `main.rs` reads the first argument as OS text
   (a non-UTF-8 argument cannot panic; it becomes lossy and is ignored as
   not a folder), hands it to a running window through
   `activation::hand_off(Some(path))`, which calls D-Bus `Open(s)` instead
   of `Activate`, and otherwise passes it to QML as `startPath`. The
   running window's `Open` queues `openRequested(path)` on the Qt thread;
   `Main.qml` shows section 5 and calls `analysisHub.openPath(path)`, as
   `Component.onCompleted` does for `startPath`. `openPath` does one
   `is_dir` stat on the Qt thread and either replaces the browse stack by
   that folder and browses it on the browse thread, or prints
   `hematita: not a folder, ignored: PATH` and leaves the page as it was.
   The desktop entry reads `Exec=hematita %f`. The argument is made
   absolute with `std::path::absolute` against the launch's own working
   directory before it goes to D-Bus or to QML (on error it stays as
   given and `open_path` reports it). When a running instance predates
   `Open` and answers `UnknownMethod`, the launch falls back to `Activate`
   rather than opening a second window. The page's Space handler sets
   `event.accepted` only when it toggles, so an unhandled Space bubbles on.
5. **The smoke's start gate.** A second, five-second run of the smoke,
   started with the scratch folder as working directory, passes the
   relative argument `data`, proving the absolutization, and requires
   `hematita-start browsing 1`, no QML error and no `not a folder`. The
   first run, with the section walk and the `hematita-storage <n>` line, is
   unchanged.

## Procedure

```sh
cd celestina-rs && cargo test -p hematita-core
bash hematita/scripts/build-production.sh && bash hematita/scripts/verify-production.sh
# the verified binary, twice, offscreen with no session bus:
QT_QPA_PLATFORM=offscreen HEMATITA_SMOKE_SHAPE=1 timeout 5 hematita/target/release/hematita "$scratch/data"
QT_QPA_PLATFORM=offscreen HEMATITA_SMOKE_SHAPE=1 timeout 5 hematita/target/release/hematita /etc/hostname
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** 0 for the build, the verification and the guards; each direct
  run ended by `timeout`.
- **Tests:** `hematita-core` lib `94 passed` (88 before: 2 moved in, 4
  new — `children_rows` merging past the limit with shares summing to one,
  no remainder under the limit, an empty folder, a `None` id as the
  remainder), `captures` `11 passed`, `usage_tree` `28 passed`; `hematita`
  `55 passed` (57 before, the 2 moved tests gone). `cargo clippy -D
  warnings` and `cargo fmt --check` clean for both, inside the verify.
- **Verify:** `qmllint-production: OK — org.celestina.hematita (0
  non-fatal baseline warning(s))`; `smoke: OK — ... the Storage page listed
  locations, a folder argument landed in the storage section, no QML
  errors, no auto-bindings`; `manifest: hematita/target/production-artifact.toml
  (verified)`.
- **Direct runs:** with a folder, `qml: hematita-start browsing 1` and no
  QML error; with `/etc/hostname`, `hematita: not a folder, ignored:
  /etc/hostname` and the page stayed on its locations
  (`hematita-start locations 0`).
- **Lines:** `analysis.rs` 784 before, 803 after (the invokable and its
  doc); `analysis_view.rs` 438; `StoragePage.qml` 613; `view.rs` 293
  (about 150 of them tests).

## Limits

- No key was pressed: Space and Delete reaching the page from the shared
  controls is argued from the controls' contract and the item-parent key
  propagation, and awaits `VAL-S3` on a real session.
- The D-Bus hand-off of a second launch was not exercised (the smoke has no
  session bus); `VAL-S3` covers it.
- Accessibility needs nothing of the page: the shared list announces each
  row from its focused slot item.

## Follow-up

`S3-Z`. `VAL-S3` pending.
