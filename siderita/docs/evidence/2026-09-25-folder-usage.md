# A folder's occupation in the properties dialog and the quick look — SID-U1-A

- **Date:** 2026-09-25
- **Scope:** `SID-U1-A` of
  [`../plans/active/2026-09-25-folder-usage.md`](../plans/active/2026-09-25-folder-usage.md)
- **Environment:** the author's checkout; offscreen Qt platform
- **Artifact:** `siderita/target/release/siderita` built by
  `siderita/scripts/build-production.sh` and verified by
  `siderita/scripts/verify-production.sh`; not deployed (`SID-U1-Z` does)

## What changed

1. **The hub.** `SideritaUsage` (`src/usage.rs`, a `qml_element`) scans the
   folder a path key names on a thread named `siderita-usage`, reading
   `/proc/self/mountinfo` there, through `hematita_core::usage::walk::scan`.
   Progress is coalesced to one publication per 250 ms; the tree lands in one
   queued call. Both carry the generation they were asked under and a stale
   one is dropped. `close()` cancels the walk's token, so no thread keeps
   reading once the modal is gone; the thread is detached, never joined on
   the Qt thread. `enter(id) -> bool` and `up() -> bool` reproject the tree
   in memory. `pathOf(id)` and `currentPath` are path keys (ADR 0008). A
   failure is published as a token (`root`, `not-a-folder`, `too-many`,
   `thread`) that the page words; the English cause goes to the log.
2. **The session.** `UsageSession` (`src/usage_session.rs`) holds no Qt:
   which root and generation are current, the tree, the current folder and
   the running token. A second `open` of the folder already scanned or being
   scanned keeps its tree or walk. The projection is `children_rows` with
   `MAX_ROWS`, `squarify` in the unit square and `flat_rects`; crumbs,
   totals, folders below and mounts below are counted in one iterative walk.
3. **The composition.** `qml/dialogs/FolderUsage.qml` weaves the hub's
   index-aligned lists into rows and tiles for the shared `CelestinaUsageList`
   and `CelestinaTreemap` (symlinked into `qml/` and registered in
   `build.rs`), adds crumbs, a totals line, «Ir a la carpeta» and «Abrir en
   Hematita». `FolderActions.qml` owns one `SideritaUsage` for both modals,
   arbitrated by an owner token: `open(path, owner)` takes the hub over and
   publishes `owner`, `close(owner)` acts only for the owner that opened it
   last, and each modal presents the section only while it owns the hub
   (`properties` or `quicklook`). A crumb jumps to its depth in one step
   (`upTo`).
4. **The properties dialog.** For a folder the card widens to 760 px and
   carries the section; the size row reads the hub's total (`Calculando…`,
   the total, or `No legible`). The controller publishes `prop_key`, the
   path key of the entry, so the dialog can hand the hub a key rather than
   the display path.
5. **The quick look.** A folder's body is the section instead of an icon and
   the word `Carpeta`; the footer names the keys.
6. **The old sum is gone.** `properties::directory_size`, the worker
   `open_properties` spawned for it and the controller's `prop_size_cancel`
   are removed; a folder's `prop_size` is empty.
7. **The hand-off to Hematita** is `SideritaUsage::open_in_hematita`, through
   `apps::launch_with("org.celestina.Hematita", path)` (the desktop entry
   applies `%f`). It lives on the hub rather than on `SideritaController`
   because the controller is a legacy coordinator under the line ratchet.

## Visible change

A folder's size now counts a hard-linked file once, per `(dev, ino)`, and
reports allocated bytes, as `du` does; the removed sum added `st_size` for
every name, so a folder holding hard links reads smaller than before and
matches `du -sh`. `UsageSession`'s `totals_count_a_hard_link_once` pins it.

## Keys and focus

Read from the focus chain, not tried by hand:

- The list's focused slot and the map's field accept only the arrows, Enter
  and Backspace. Space and Escape travel up the item parents — the list or
  map, `FolderUsage`, the quick look's body, the card, the modal layer's
  content host — to `QuickLookView`'s own `Keys.onPressed`, which closes the
  quick look for both. In the properties dialog Escape reaches the layer's
  dismissal.
- The crumbs are ordinary buttons, one Tab stop each, before the list and
  the map; Space on a focused crumb is its click.
- The modal layer gives focus to the first focusable item when it opens,
  which for a folder would be the list; the quick look takes it back to its
  surface after opening on a folder, so ↑ ↓ keep stepping entries until Tab
  moves in.
- The list and the map accept Return whatever the modifiers, so Ctrl+Enter
  is a `Shortcut` enabled only while focus is inside the section.
- Left and Right in the list are stopped by the section, so they no longer
  step the quick look to another entry; the map uses them to walk tiles.
  Focused buttons (crumbs, footer) take Space as their own click.

## Procedure

```sh
bash siderita/scripts/qml-tests.sh
bash siderita/scripts/build-production.sh && bash siderita/scripts/verify-production.sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- QML interaction tests: 157 passed, 0 failed, including six new
  `FolderUsage` cases (the woven rows and `otros (2)`, Return drills,
  Backspace goes up, Ctrl+Return goes to the folder and does not drill, the
  Hematita button hands the current folder over, Space is left unaccepted).
- `cargo test` in `siderita/`: 136 passed, 1 ignored, including seven
  `usage_session` tests (same root keeps the tree, stale tree refused, a new
  root cancels the running scan, drill and up without a second scan, close
  forgets everything and makes a late tree stale, a hard link counted once,
  rows, tiles and paths agree) and the progress-interval test in `usage`.
- `cargo fmt --check` and `cargo clippy --all-targets -D warnings` clean; the
  domain crates' tests green.
- `qmllint`: siderita's warnings fell from 259 to 258 and the row in
  `scripts/qmllint-baseline.tsv` is lowered in this unit.
- Verify tail:

```text
Totals: 157 passed, 0 failed, 0 skipped, 0 blacklisted
smoke: OK — binario vivo 8 s, sin errores QML, sin auto-bindings
manifest: siderita/target/production-artifact.toml (verified)
```

- The first verify stopped at the architecture guard (`Font.Normal` in a
  crumb, replaced by `CelestinaTheme.weightRegular`); the second at the
  qmllint ratchet asking to be lowered; the third passed.
- `SideritaController` did not grow: `controller.rs` stays at 754 lines,
  its baseline (one `prop_key` property added, one stray blank line in the
  C++ block removed).

## Fix round 1

Review findings applied: the owner token on the shared hub (a close from the
quick look no longer empties the properties dialog after it took the hub
over), Left/Right stopped inside the section, focusable crumbs, `up_to` in
one step. New tests: `only_the_last_owner_closes_the_session` and
`up_to_jumps_to_a_crumb_in_one_step` in `usage_session`; QML
`test_g_left_and_right_in_the_list_stay_in_the_section`,
`test_h_going_to_a_file_opens_the_folder_that_holds_it`,
`test_i_the_totals_say_scanning_and_failure`,
`test_j_enter_on_the_map_drills_through_the_hub`,
`test_k_a_crumb_jumps_in_one_step`, and `test_f` now asserts Space reaches
the host. QML tests: 162 passed.

The incremental build+verify pair passed on its first run: `cargo test` in
`siderita/` 138 passed, 1 ignored; clippy and fmt clean; qmllint at the
258-warning baseline; QML 162 passed; smoke OK; manifest verified.

## Limits

- Nothing was tried on the real session: the scan of a real home, the
  thread stopping on close, the keyboard walk, a screen reader and the
  Hematita hand-off are `VAL-SID-U1`.
- The hub itself has no headless test: its rules are `UsageSession`'s, which
  are tested, and the QML is tested over a stub hub. The offscreen smoke
  proves the module builds and the window constructs, not that a scan lands.
- The tree stays in memory while the modal is open; the arena is not
  bounded, the same risk Hematita carries.
- A Siderita refresh does not invalidate an open section's tree; closing the
  modal does.
