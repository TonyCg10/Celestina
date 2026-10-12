# Evidence: 2026-10-11 renaming in place, and the drag that never selected

- **Date:** 2026-10-11
- **Scope:** `SID-H1-S` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.12.0, offscreen QML tests; nothing was renamed on a real session
- **Artifact:** the session's release build at `/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita`; the landing builds, verifies and deploys the registered binary

## The report

On 2026-10-11 the author reported that dragging the pointer over the name
while renaming selected nothing, and approved a design for renaming in place:
a slow second click on the name of the only selected entry, F2 and the entry
menu's «Renombrar» open an editor over the name in the list or the grid; the
name prompt dialog stays only for creating a folder or a file.

## Item 1: why a drag selected nothing

`CelestinaTextField` is a Qt `TextField` with `selectByMouse` on, so the field
was not refusing the drag: something took the pointer from it.

**Reproduction.** A scratch QML test (not committed) put a
`CelestinaTextField` in a `GlassCard` inside a `CelestinaModalLayer`, the shape
of `NamePromptDialog`, and dragged across it the way a mouse does, 2 px per
move:

| Drag | `selectedText` |
|---|---|
| 2 px steps inside the modal layer | empty |
| 50 px steps inside the modal layer | `n nombre de archivo larg` |
| 2 px steps, the layer's shield `DragHandler` disabled at run time | the whole text |
| 2 px steps, the same field outside the layer | the whole text |

`grabChanged` on the shield's `DragHandler` during the 2 px drag printed
`GrabPassive` (0x01) on the press, `GrabExclusive` (0x10) on the first move,
`UngrabExclusive` (0x20) on the release and `UngrabPassive` (0x02): the
handler took the exclusive grab from the field on the first pixel.

**Root cause.** The shared style, not Siderita:
`celestina-style/CelestinaInputShield.qml:64-79`, the shield's `DragHandler`,
which `CelestinaModalLayer.qml:262` arms under every modal card. It has
`dragThreshold: 0` (line 73) and, unless `yieldsToHost`,
`PointerHandler.CanTakeOverFromAnything` (line 78). Its comment (lines 71-72)
assumes that "a text field holds its own grab while it selects", but
`QQuickTextInput` sets `keepMouseGrab` only once the pointer has travelled the
platform's start-drag distance (10 px here); until then nothing vetoes the
takeover, so the zero-threshold handler wins on the first pixel and the field
never extends its selection. A first move longer than that distance lets the
field claim its grab first, which is why
`siderita/tests/qml/tst_modal_and_pill_drag.qml:156-161`
(`test_ba_card_controls_stay_alive`, 50 px steps) passes and never caught it.

The defect is therefore in every text field inside a `CelestinaModalLayer`
in the suite (in Siderita: the create prompt and the format, password,
compress and batch-rename dialogs, and possibly the embedded Grafita editor's
dialog). This unit does not edit `celestina-style/`; the shield needs its own
style unit. Renaming no longer goes through a modal, so the rename itself no
longer meets it.

**The editor's own thieves.** The inline editor sits outside any modal, but
Siderita's own items would have taken it apart the same way:

- the entry's `DragHandler` (`FolderRowDelegate.qml`, `FolderCellDelegate.qml`,
  `dragThreshold: 8`, below the field's 10 px) takes the press passively
  through the field and starts an entry drag on the eighth pixel. It is
  disabled while a rename is open; with it re-enabled,
  `test_p_a_drag_across_the_name_selects_text` fails (empty selection);
- a field inside the `ListView` lost its focus to the view's current delegate
  on every model reset (an item view is a focus scope), which ended the edit
  on any refresh (`test_r_the_editor_follows_its_entry_through_a_refresh`
  failed so); the editor now lives beside the view, over it;
- with `interactive` left on, the list still flicks under the editor
  (`test_q_scrolling_and_keys_stay_with_the_editor` fails).

A window-wide `TapHandler` tried first as the "click elsewhere" observer took
the press itself (a handler whose parent contains the point accepts it), so
the field never saw it; the observer is a `MouseArea` under the whole window
instead, which only hears presses nothing else took.

## Item 2: what was built

- `qml/components/folder/EntryNameEditor.qml` (new, registered in
  `build.rs`): one per view. It owns the slow click (armed by a press on the
  painted name of the sole selected entry with the left button and no
  modifier, started by the click, fired after
  `Application.styleHints.mouseDoubleClickInterval`, dropped by any press, a
  double click, a moved selection or a view without the keyboard), the
  `CelestinaTextField` laid over the delegate's `nameLabel` in its font and on
  its first line (accessible name «Nuevo nombre»), the initial selection (the
  base name; a folder's or a dotfile's whole name), Return through
  `controller.renamePath(path, value)`, Escape, the end on a lost focus, a
  press nothing took, a vanished entry or a hidden view, and the outcome: an
  `opError` keeps the editor open with the text, a landed listing selects the
  renamed entry and gives the view the keyboard back. Empty and unchanged
  names end the edit without a call. Switching to another window keeps it.
- `FolderListView`, `FolderGridView`: host the editor, expose `renaming`,
  `renamer` and `beginRename(index)`, stop flicking and the wheel while it is
  open, and ignore the keys the field leaves (no arrows, no type-to-select).
- `FolderRowDelegate`, `FolderCellDelegate`: publish `nameLabel`, hide it
  under the field, feed the slow click from their `MouseArea`, open on a
  double click through `hostWindow.activateEntry` (the same window, now
  reachable from a test), and disable their drag while a rename is open.
- F2 (`FolderShortcuts`) and «Renombrar» (`EntryContextMenu`, after the menu
  has closed and returned the focus) call `beginRename`; `FolderActions`
  counts an open rename as `navigationBlocked`, so the window and folder
  shortcuts and the history buttons stand down while it is open.
- `NamePromptDialog` keeps only the folder and file modes.
- `FolderView.qml` stays at its 738-line ratchet: one wiring line moved from
  the shortcuts to the actions.

## Procedure

```sh
(cd siderita && cargo fmt --all --check)
(cd siderita && cargo clippy --all-targets --locked -- -D warnings)
(cd siderita && cargo test)
(cd siderita && cargo build --release --locked)
sh siderita/scripts/qml-tests.sh   # three runs
sh siderita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

`siderita/tests/qml/tst_inline_rename.qml` drives the real `FolderListView`,
`FolderGridView`, delegates, `FolderShortcuts` and `EntryContextMenu` against
a stub controller: a slow click edits after the interval and not before; a
click on an unselected entry, a double click (which opens), a click on the
icon, a Ctrl click and a click whose selection moved on never edit; F2 and the
menu entry edit in place and keep the focus past the menu's close; the
extension stays out of the first selection; Return sends the name and the
landed listing selects the renamed entry; empty and unchanged names send
nothing; a refused name keeps the editor, its text and the selection; Escape,
a press elsewhere and a click on another entry cancel; a 2 px-step drag
selects the whole name in the list and in the grid without picking the entry
up; the wheel, the arrows and type-to-select leave the list alone; the editor
follows its entry through a refresh that moves it and ends when it vanishes;
the field's accessible name; switching the view ends the edit.

## Result

- **Exit:** formatting, clippy, the unit tests (195 passed, 0 failed, 1
  ignored), the release build, the smoke run, the architecture contract, the
  language contract and the documentation contract exited 0. The QML tests
  passed three runs in a row, 237 passed and 0 failed each time (24 of them
  in the new `tst_inline_rename.qml`: 22 cases and its set-up and tear-down).
- **qmllint:** 220 warnings, down from the 222 baseline, which this unit
  lowers: the two delegates no longer call `activateEntry` on the untyped
  `Window.window` (two `missing-property` warnings gone), and the new editor
  lints clean (it reads `Application.styleHints`, which is typed; the
  untyped `Qt.styleHints` cost one warning in a first measure of 221). The
  editor's loops sit in functions, not handlers, and its warnings were seen
  in the same run, so the file is linted.
- **Language:** rewriting `EntryContextMenu.qml`'s Spanish header in English
  and replacing the rename dialog's property comment lowered that file's
  legacy count from 12 to 7; its row in `scripts/language-baseline.tsv` is
  lowered with it.

## Limits

- Nothing ran on the real session: the platform's double-click interval and
  start-drag distance, a physical mouse drag over the field, the editor's
  placement at other text and icon scales, and AT-SPI reading «Nuevo nombre»
  are `VAL-SID-19`.
- The rename's outcome is read from what the controller already publishes
  (`opError`, `loading`, the listing), not from a dedicated signal:
  `controller.rs` is at its line ratchet. A refresh started by another
  operation that lands before the rename's own answer would close the editor
  early; the rename still lands or its notice still shows.
- Search results are not republished after a rename (as before, with the
  dialog): the hit keeps its old name until the search runs again.
- The create dialogs keep the shield's drag defect until the style unit.

## Follow-up

- A `celestina-style` unit for `CelestinaInputShield`'s zero-threshold
  takeover of a text field's press, with a realistic small-step drag test
  beside `tst_modal_and_pill_drag.qml`'s 50 px one.

## Landing

- **Base revision:** `45b22f4609ab92969c5a95e1f1805b8cd79e0c3e`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:fffe3aa4d67ebaecfde4b2f2af87ef1c844be49d60307c983d5c154c41392884, verification_fingerprint sha256:3dc4f17433ae77e5bf6cb93074d0c112b8a886148f2318d88714c8940694a529
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
