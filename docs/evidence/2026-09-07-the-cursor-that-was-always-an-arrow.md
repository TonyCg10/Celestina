# Evidence: 2026-09-07 the cursor that was always an arrow

- **Date:** 2026-09-07
- **Scope:** `FEEDBACK-6-SID` — `siderita` 1.5.10. A correction of
  `FEEDBACK-5` ([record](2026-09-07-the-press-that-stayed-grey.md)) after the
  author reported that the path pill still did not "become text" on click and
  that the magnifier closed and reopened the search in one click
- **Environment:** the author's Arch-derived Linux, Qt 6.11, `cargo` stable;
  the real release-shaped binary driven offscreen with QtTest's `TestEvent`
  from a temporary probe in `Main.qml`, then removed
- **Artifact:** `siderita/target/release/siderita`

## What the author reported, and what the binary did

Two sentences: the path bar still did not behave as text when clicked, "so
that it shows that icon" — an I-beam over a text field — and clicking the
magnifier on an open search closed and reopened it at once.

The running process (`pid 581025`, started 19:41) was the 1.5.9 binary
deployed at 19:04, so this was not a stale instance. Driven offscreen, that
binary did everything the source says: a press on empty pill turned editing
on with the field focused and `/home/toni` in it; a click on the current crumb
opened the editor with `toni` selected; the magnifier opened the search and,
after the `FEEDBACK-5` fix was applied to the probe build, a second click
closed it without reopening. Behaviour was not the defect. The cursor was.

`Main.qml` and `PickerWindow.qml` each carry a `HistoryMouseArea` — the owner
of the mouse's Back and Forward buttons — filling the window at `z: 1000` and
`z: 9999`, above everything. A Qt Quick `MouseArea` always imposes a cursor,
`ArrowCursor` by default, and the window resolves the pointer's shape from the
topmost item under it that has one. So the shape over every control in the
application was an arrow: over the path pill's I-beam, over the crumbs' hand,
over the editor while editing, over every row and glyph. The bar *was* text
under the pointer; the pointer was never allowed to say so. The area accepted
only Back and Forward, so clicks passed — which is why nothing else had ever
pointed at it.

The magnifier's fault is separate and confirmed in source: a `Button` takes
focus on press, the emptied search field collapsed the pill on losing it, and
`focusSearch()` in the same click expanded it again.

## What changed

**The history areas sink under the content**, `z: -1` in both windows. They
still receive every Back and Forward press: nothing above accepts those
buttons, and a refused press falls through. The cursor is now whatever the
control under the pointer declares — I-beam over the path pill and its editor,
hand over the crumbs, arrow over the rest.

**The path editor states its I-beam** with a `HoverHandler` of its own rather
than relying on the text input's default.

**The search glyphs take no focus** (`focusPolicy: Qt.NoFocus`), so a click
on them never collapses the field first. The magnifier is a plain toggle: it
opens the search, and on an open, empty search it closes it. The clear glyph
gets the same policy for the same reason.

## Procedure

| Check | Result |
|---|---|
| Offscreen probe on the real binary: press on empty pill → `editing=true`, field focused, text `/home/toni`; current crumb → `sel=toni`; magnifier twice → expanded true, then false | observed |
| `qml-tests.sh` — 112 pass, 1 new: a Back press refused by a left-only MouseArea above reaches the history area underneath | pass |
| `build-production.sh`, `verify-production.sh`, deployment to the author's prefix | pass — `siderita` 1.5.10 deployed |

## Result

The pointer changes shape over the controls that declare one. The path pill
shows the I-beam, its crumbs the hand, its editor the I-beam. The magnifier
opens and closes the search in one click each.

## Limits

- A cursor shape cannot be read from QML, so the fix is proven by the removal
  of the only item that could override it and by Qt's documented resolution
  order, not by an assertion. `VAL-SID-14` gains the check by hand.
- Any other full-window `MouseArea` added above content in future will bring
  the arrow back. The rule is now written where the two areas live.
