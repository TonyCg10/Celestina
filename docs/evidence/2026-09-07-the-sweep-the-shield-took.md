# Evidence: 2026-09-07 the sweep the shield took

- **Date:** 2026-09-07
- **Scope:** `FEEDBACK-7-SID` — `siderita` 1.5.11. The author recorded the
  1.5.10 path editor and a Grafita editor side by side: "not that it shows the
  cursor — that it works like text"
- **Environment:** the author's Arch-derived Linux, Qt 6.11; the real binary
  driven offscreen with QtTest `TestEvent` presses and moves from a temporary
  probe, then removed
- **Artifact:** `siderita/target/release/siderita`

## The defect

With the editor open, a sweep over `/home/toni` selected nothing and, in the
recording, the crumbs came back. The isolated interaction test could not
reproduce it: under `qmltestrunner` the sweep selected. The real binary
reproduced it at once.

The pill's `CelestinaInputShield` exists so that a press on the floating pill
never reaches the file rows behind it. Its drag handler has a zero threshold
and `CanTakeOverFromAnything`, and a press delivers a passive grab to every
handler under the point even after the text input has accepted it. On the
first pixel of movement the shield took the exclusive grab from the text
input, and the selection died at the caret. Making the shield yield
(`yieldsToHost`) was not enough: the file rows' own drag handlers hold the
same passive grabs and, without the shield to beat them, took the sweep at
eight pixels and dragged a file — the two `FloatingSurfaces` tests said so.

## What changed

While the editor is open the shield yields, and the pill's **own** drag
handler claims the sweep from the first pixel — beating the rows behind — and
turns it into the field's selection: the press has already placed the caret,
the handler anchors there and, as the pointer moves, `select(anchor,
positionAt(pointer))` grows the selection exactly as the text input would
have. The search pill gets the same handler for its field. Nothing behind
either pill ever sees the sweep.

## Procedure

| Check | Result |
|---|---|
| The author's recording: sweep over the open editor's text → no selection, crumbs back | the defect |
| Offscreen probe on the real binary after the fix: press in the open editor at the caret, three moves, release → `sel=[e/toni]`, cursor 10, still selected on release | observed |
| `qml-tests.sh` — 113 pass, including the two `FloatingSurfaces` sweeps that must not drag the content and the new sweep-inside-the-editor case | pass |
| `build-production.sh`, `verify-production.sh`, deployment to the author's prefix | pass — `siderita` 1.5.11 deployed |

## Result

The path pill edits like the editor in the author's second recording: a press
places the caret, a sweep paints a growing selection, and the rows behind the
pill stay where they are.

## Limits

- The isolated harness does not reproduce the steal — its scene has no
  passive grabbers behind the pill — so the recording is the evidence for the
  defect, the real-binary probe for the fix, and the harness for the
  protections that had to be kept.
