# Evidence: 2026-10-11 slow drag selection in a dialog's text field

- **Date:** 2026-10-11
- **Scope:** `STYLE-G7-AE` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.12.0; every QML test runs offscreen
- **Artifact:** not applicable in the session; the landing builds and verifies the registered module and its consumers

## Procedure

Siderita's unit `SID-H1-S` found the cause on 2026-10-11: a text field
inside any modal never extended its selection under a slow pointer drag.
Measured there: 2 px steps in the modal selected nothing, 50 px steps
worked, 2 px steps with the shield's `DragHandler` disabled or outside the
modal selected everything. Siderita's `tst_modal_and_pill_drag.qml` moves
50 px at a time and missed it.

**Why the shield has the drag claim.** `9c0781e0` extracted
`CelestinaInputShield`: a floating box that swallows clicks still lets the
covered surface's *pointer handlers* react, because Qt 6 keeps delivering a
press to the handlers of every item under it after an item accepted it, so
a row's `DragHandler` takes a passive grab and starts its drag after its
threshold. A sweep over a dialog card dragged the file the card covered.
The claim (`dragThreshold: 0`, `CanTakeOverFromAnything`) takes the
exclusive grab on the first pixel, also from the card's own `MouseArea`
(an item grabber, the path the sweep escaped through), so the handler
below never gets it. `tst_modal.qml`'s `test_pointer_blocking_covers_drags`
and Siderita's `test_a`/`test_bb`/`test_bc` are its tests.

**Why it broke selection.** Qt's `TextInput` sets `keepMouseGrab` only
past the platform start-drag distance (about 10 px), so the claim took the
grab from it on the first pixel. The comment that "a text field holds its
own grab while it selects" was wrong. The field also never received the
release, and in Qt 6.12 a Controls `TextField` takes the focus on release,
so the slow drag did not even focus it. `TextEdit` was not affected: it
kept its selection in the same test under the old shield.

**Candidates rejected by measurement:**

- *Do not take over from items* (`yieldsToHost: true` on the modal's
  shield, in the scratch copy): the slow selection still came out empty,
  because the covered surface's `DragHandler` (threshold 8) takes the
  pointer from the `TextInput` in the same gap. The two drag-blocking tests
  also failed, so the takeover from items is the shield's purpose itself.
- *A zero-threshold `DragHandler` in `CelestinaTextField`* would refuse to
  give up any drag that starts on a field, which also stops a scrollable
  form from being flicked from a field, and it would leave plain fields as
  they were.

**The fix.** The claim stays as it was. When it goes active, the shield
looks for the text field the press landed on and, while it holds the drag,
moves that field's cursor selection to the pointer
(`moveCursorSelection(positionAt(…), mouseSelectionMode)`). On a real
release (`UngrabExclusive` with the point released) it gives the field the
focus it would have taken. Every move and that focus require the shield
and the field to be still enabled and visible, ancestors included. A
cancelled or taken-over grab gives nothing, and neither does a dialog
accepted mid-drag. The field is found by walking the window's items from
the top, the way the press was delivered (children over their parent,
children below zero under it). The walk stops at the first enabled item
under the press that takes a press: a `TextInput`/`TextEdit` (the field,
if `selectByMouse`), a `MouseArea`, any `QtQuick.Templates` `Control`
(buttons, panes, a popup's or menu's padding) or a `Flickable` (none of
these is a field). Those are judged after their own children, so a field
inside one is still found first. The walk also stops at the shield itself,
so a field of the content the shield covers never qualifies. The type is
`T.Control` and not `QtQuick.Controls`' `Control`, which is the style's
own derived type: a `Pane` or a `Button` is not an instance of it, and a
test showed the walk passing through a `Pane` with that check.

**A trap found on the way.** Declaring any property on the shield's
`DragHandler` made the old drag-blocking tests fail. A property makes the
handler a QML type of its own, and a plain `DragHandler` underneath may
take over from handlers of a *different* type (its default permissions),
so it took the drag back. The handler keeps no properties. The state lives
in a `QtObject`, and a comment in the file records why.

Tests added (failing first against the old shield in a scratch copy):

- `tst_modal.qml`: a `CelestinaTextField` swept at 2 px steps selects
  exactly from the press to the release position and takes the focus; a
  plain `TextField` at 1 px steps selects; a slow sweep out of the field
  over the covered list keeps selecting and starts no drag below; a slow
  sweep from the card's empty part starts no drag below (the original
  purpose at a slow pace); a `TextEdit` selects; a sweep from the empty
  card, which lies over a dialog field and over a covered field, selects
  in neither, nor in a field it crosses; a drag pressed on a `Pane` or a
  `Flickable` lying over a field neither selects in it nor focuses it; a
  dialog closed mid-drag stops moving its field's selection and gives it
  neither the active focus nor the focus flag on the release; a sweep that
  runs out of the field selects from the press to the end of the text.
- `tst_inputshield.qml`: a bare pill whose shield covers a content field
  never selects in it.

```sh
# failing first and the mutations: a scratch CelestinaStyle/ copy of the
# style's *.qml, qmldir, fonts/ and icons/
QT_QPA_PLATFORM=offscreen qmltestrunner -import <scratch> -input celestina-style/tests/tst_modal.qml
QT_QPA_PLATFORM=offscreen qmltestrunner -import <scratch> -input celestina-style/tests/tst_inputshield.qml
cmake -S celestina-style -B <scratch>/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build <scratch>/build --parallel
QT_QPA_PLATFORM=offscreen <scratch>/build/celestina-style-modal-test   # three runs
cmake --build <scratch>/build --target all_qmllint
qmllint -I <scratch>/build celestina-style/CelestinaInputShield.qml
sh siderita/scripts/qml-tests.sh   # and calcita, cuprita, fluorita, selenita
bash scripts/qmllint-cxxqt.sh siderita   # and calcita, grafita, fluorita, magnetita, selenita, cuprita, hematita
sh celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **Exit:** with the old shield, `tst_modal.qml` passed 15 of 18; the three
  slow-selection cases failed with an empty selection. With the fix it
  passes 20 of 20 and `tst_inputshield.qml` 6 of 6. The whole QuickTest
  binary passes 200 of 200, three runs in a row. `all_qmllint` exits 0 and
  its only warnings are the existing ones of the untouched
  `CelestinaLineGutter.qml`. `CelestinaInputShield.qml` lints clean.
  Siderita's QML tests pass 213 of 213, including
  `tst_modal_and_pill_drag.qml` and `tst_celestina_input_shield.qml`;
  Calcita passes 135 of 135, Cuprita 66, Fluorita 35 and Selenita 86. The
  style, architecture, language and documentation contracts exit 0.
- **Mutations:** with the walk not stopping at the shield,
  `test_a_covered_field_does_not_select_through_the_shield` fails; with it
  not stopping at a `MouseArea`, `test_a_sweep_from_the_card_selects_nowhere`
  fails; without the `T.Control` stop or without the `Flickable` stop,
  `test_a_sweep_from_a_control_over_a_field_selects_nowhere` fails; without
  the live check on a move or before the focus,
  `test_a_dialog_closed_mid_drag_gives_its_field_nothing` fails; with a
  property on the handler, `test_pointer_blocking_covers_drags` and the
  slow card sweep fail.
- **Ratchets:** Calcita 1, Grafita 38, Fluorita 56, Magnetita 38,
  Selenita 1 and Cuprita 9 are unchanged. Siderita reads 340 against its
  row of 222 both with this change and with the base shield put back, with
  identical warnings. The script picked the newest Siderita build directory
  in the shared target, which belongs to another branch (its qmldir names
  `EntryNameEditor.qml`, absent here), so the change adds nothing. Hematita
  has no release module in the shared target and was not measured.

## Limits

- The session ran no window and dragged no real pointer; synthetic mouse
  events at 1 and 2 px steps stand in for a slow hand.
- The walk recognises `MouseArea`, `T.Control`, `Flickable` and the two
  text items as press takers. A pointer handler stacked above a field (a `TapHandler`
  that takes the press) is not one of them; no dialog of the suite has one.
- A drag that follows a double click extends by `mouseSelectionMode`
  (characters by default), as `moveCursorSelection` does. It does not
  switch to whole words the way the field's own drag might.
- The release no longer reaches the field, so whatever the field itself
  does on a release (such as offering the selection as the primary
  selection where the platform has one) does not happen; not checked.
- Whether it feels right is the author's check, `VAL-STYLE-10`.

## Follow-up

`VAL-STYLE-10` in [VALIDATION.md](../../VALIDATION.md).

## Landing

- **Base revision:** `244f546dcfbb14b5b0d32673ee3740c6476754c4`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:469c2a323a83a4dffc40a6ca0fe6c75e2e4ee133fb090a09888b5ac8450d1027, verification_fingerprint sha256:6ae5d01f530b1f2f996303a9142289c2f05f313b06dea041f347b2d80ed10041
