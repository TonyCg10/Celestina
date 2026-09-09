# Evidence: 2026-09-08 the side buttons that stopped navigating

- **Date:** 2026-09-08
- **Scope:** `FEEDBACK-8-SID` — `siderita` 1.5.12. A regression introduced by
  `FEEDBACK-6-SID`
  ([record](2026-09-07-the-cursor-that-was-always-an-arrow.md)) and reported by
  the author: the mouse's Back and Forward buttons no longer navigate
- **Environment:** the author's Arch-derived Linux, Qt 6.11.2; the real binary
  driven offscreen with QtTest `TestEvent` presses from a temporary probe in
  `Main.qml`, then removed
- **Artifact:** `siderita/target/release/siderita`

## The regression, and the wrong reasoning behind it

`FEEDBACK-6` moved the window-wide `HistoryMouseArea` — the owner of the
mouse's Back and Forward buttons — from `z: 1000` to `z: -1` in `Main.qml`,
and from `z: 9999` to `z: -1` in `PickerWindow.qml`. It gave two reasons and
both were wrong.

**"A `MouseArea` always imposes a cursor."** It does not. `QQuickMouseArea`
declares `cursorShape` with `RESET unsetCursor`, and an item owns a cursor only
once `setCursor` has been called — `QQuickItemPrivate` tracks that in its
`hasCursor` bit, and the window's cursor resolution skips items without it.
`HistoryMouseArea` never assigns `cursorShape`, so it never had a cursor and
never hid anything below it. The author's actual complaint had nothing to do
with the cursor — they said so plainly — and its real cause was fixed in
`FEEDBACK-7`: the pill's input shield was taking the sweep from the text input.

**"Nothing above accepts those buttons, so a refused press falls through."**
Not in this window. Driven with real Back presses, an area at `z: -1` received
none, over the file view or over the sidebar, and navigation by mouse was
simply gone.

## What changed

Both areas go back on top: `z: 1000` in `Main.qml`, `z: 9999` in
`PickerWindow.qml`. Being on top costs nothing — the area accepts only Back and
Forward, so every ordinary click still reaches the controls below it, and it
owns no cursor. The comments at both sites now record the measurement rather
than the theory, so the change is not made again.

The explicit I-beam `HoverHandler` `FEEDBACK-6` added to the path editor stays:
it states the shape instead of relying on the text input's default, which is
correct on its own terms. The `focusPolicy: Qt.NoFocus` it added to the search
glyphs stays too — that fix was real and unrelated.

The interaction test `FEEDBACK-6` added asserted the sunk arrangement and so
guarded the defect. It is replaced by one that mirrors the real window: content
below taking the ordinary buttons, the history area above it, and both halves
checked — Back reaches the area, and a left click still reaches the content.

## Procedure

| Check | Result |
|---|---|
| Offscreen probe, `z: -1`: navigate in, Back over the file view, Back over the sidebar | path unchanged both times — the regression |
| Offscreen probe, `z: 1000`: the same | back to `/home/toni` both times |
| Offscreen probe: left click on the path pill with the area on top | `editing=true` — ordinary clicks are not blocked |
| `/usr/include/qt6/QtQuick/6.11.2/QtQuick/private/qquickmousearea_p.h`, `qquickitem_p.h` | `cursorShape` has `RESET unsetCursor`; `QQuickItemPrivate::hasCursor` gates cursor ownership |
| `qml-tests.sh` — 114 pass, 2 replacing the misleading one | pass |
| `build-production.sh`, `verify-production.sh`, deployment to the author's prefix | pass — `siderita` 1.5.12 deployed |

## Result

Back and Forward navigate again, over the file view and over the sidebar, in
both windows, and every ordinary click still reaches the control under the
pointer.

## Limits

- The isolated harness cannot see this class of defect: its scene has no real
  window, so an area at `z: -1` receives presses there that it never receives
  in the application. The offscreen probe on the real binary is the only guard
  that caught it, and the procedure above is the way to repeat it.
- `VAL-SID-14` gains the side buttons; a physical mouse with side buttons is
  the author's check.
