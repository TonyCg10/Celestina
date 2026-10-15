# Nothing above the seam, and the resting twin stops painting

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-M`
- **Artifact:** Celestina 0.29.10
- **Environment:** the 4K nest (scale 1.5) recorded at 60 fps for the
  failures; a live QML probe through the session journal for the mechanism;
  the offscreen suite, now grabbing real pixels, for the proof
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

After the one-block unit the author reported everything unchanged. The OSD —
same fall, same membrane, drivable without a pointer — was recorded at 60 fps
and reproduced both defects; a probe timer inside the card delegate then
printed the runtime numbers to the journal, which is what replaced theory
with the two causes.

## Result

### The probe's answer

Two card instances were alive on every OSD: the attached window's, falling
correctly — `y 60, startY 40, topReq true, prog 0.25 → 1.00` — and a second
on another window with `anchored false, startY -1, y 0, prog 1.00`: fully
formed, never placed, never falling.

### Cause one: the finished card at the window's top edge

Before the routing properties settle, the delegate's position read 0 — and 0
on the attached window is the screen's top edge, over the bar's own icons.
Clamping the position was not enough, because the seam itself
(`attachmentStartY`) is `-1` until the same routing arrives. The law is now
enforced where every card lives: the attached window's scene carries a clip
at the seam, so nothing can show above it — whatever geometry race puts a
card there. The membrane's mouth is tangent at the seam and paints downward,
so it loses nothing. A new regression grabs real frames across the whole
fall and asserts the strip's pixels stay untouched.

### Cause two: the resting twin painted the file

`createWindow` seeded `readings` into every window it built — including the
bottom-right twin that rests mapped between visits. Born carrying the card
file, it painted a second volume card in its corner while the attached
window presented the real one; the recording shows both at once. No window
is seeded now: the presenting one receives the file through `pushReadings`,
and only it does.

## Limits

The OSD's own recording could not be repeated after the fix — the output was
blacked out by the author's own switch — so the after-proof is the pixel
regression rather than a second video. The menus' falls and the popup trio
remain the author's pass, on this build.
