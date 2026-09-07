# Evidence: 2026-09-07 the press that stayed grey, and the pill that went blank

- **Date:** 2026-09-07
- **Scope:** `FEEDBACK-5` — `celestina-style` 1.8.7, `siderita` 1.5.9; the
  other three applications are rebuilt and redeployed because they compile the
  shared button. A same-day correction of `FEEDBACK-4`
  ([record](2026-09-07-two-families-and-a-path-editor.md)) after the author
  recorded the delivered bytes
- **Environment:** the author's Arch-derived Linux, Qt 6.11, `cargo` stable,
  offscreen QPA for tests
- **Artifact:** the registered production artifacts of the five owners

## What the recording shows

Read frame by frame at 5 and 10 frames per second, cropped to the bottom pills,
the search band and the sidebar.

1. **The floating pills still pressed in the old grey.** Hovering the sort pill
   lifted it to `surfaceHover`; pressing it darkened it to `surfaceStrong`. The
   shared button's Tonal and Ghost roles did the same. `FEEDBACK-4` had given
   the *row plate* the accent's pressed wash but left the buttons on the grey,
   so a sidebar row and the pill beside it answered the same finger in two
   languages, and the pill's press read as "nothing happened".
2. **The path pill went blank after a search.** The author clicked the
   magnifier, the pill expanded, the author clicked into the content; the
   search collapsed and the path pill showed no crumbs for about three
   seconds, until the next navigation put them back. In the crop the pill
   looked empty because the editor's monospace text is left-aligned and the
   crumbs right-aligned: the pill was in *editing* mode, uninvited. The
   `FEEDBACK-4` field turns editing on when it receives focus, so that Tab can
   reach it, and it received focus here for another reason: the collapsing
   search field is disabled, and Qt hands the focus of a disabled item on.
3. **The search pill ignored a click beside its glyph.** Its input shield
   swallowed the press, so only the 32 px magnifier opened the search.
4. **The sidebar**, which the author had left in the Control family, was asked
   onto the same accent ramp as the centre.

## What changed

**One press vocabulary.** `CelestinaButton`'s Tonal and Ghost roles press in
`pressedWash`, as the row plate does; `FloatingButton` follows, with its four
states in order: rest, the grey lift, the wash under the finger with the sink,
and `badgeAccentFill` while on. Primary, Destructive and Selected keep their own
pressed tints, which were already the role's hue.

**The path field opens only for a deliberate arrival.** Its focus handler now
reads `focusReason`: Tab, Backtab and a shortcut open the editor with the whole
path selected; any other reason — the hand-over from a disabled field, a
popup closing, a window activating — sends the focus back to the list and
leaves the crumbs alone.

**The search pill is a field everywhere.** A `MouseArea` under its controls
focuses the search on a click anywhere on the glass and shows the I-beam.

**The sidebar joins the Content family.** Its eight plates — places, devices,
phone, favourites, bookmarks, and the three in the picker's sidebar — declare
`family: Content`, so a sidebar row hovers at 7 %, presses at 26 % and settles
to its current-row blue like a file does. Section headers stay Control.

## Procedure

| Check | Result |
|---|---|
| `celestina-style`: `cmake --build`, `ctest` (73 Qt Quick tests) | pass |
| `siderita`: `qml-tests.sh` — 111 pass, 3 new: focus arriving for a non-keyboard reason leaves the crumbs; focus arriving by Tab selects the whole path; a click on the search glass beside the glyph focuses the search | pass |
| Production build and verify, five owners; deployment of the four applications and the daemon | pass — `celestina-style` 1.8.7, `siderita` 1.5.9; Grafita 1.2.4, Fluorita 1.3.5 and Magnetita 1.2.3 rebuilt on the new shared button and redeployed |

## Result

Every neutral control in the suite — button, pill, row, cell, tab — presses in
the same accent wash and sinks; the sidebar runs the same ramp as the centre;
the path pill edits only when asked; the search pill answers a click anywhere
on it.

## Limits

- The focus hand-over could not be reproduced in the isolated test harness:
  disabling the focused search field there returns focus to the test root, not
  to the path field, so the route the real window takes is inferred from the
  recording and the code, not observed. The guard is by reason, so it covers
  that route whichever item hands the focus on. `VAL-SID-14` gains the check.
- Whether the sidebar reads better on the accent ramp than on the grey lift is
  the author's call to keep or reverse; it is one `family` line per row.
