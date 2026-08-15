# Every surface forms and leaves as one block, on frames somebody sees

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-K`
- **Artifact:** Celestina 0.29.8
- **Environment:** the nested reference session on HDMI-A-1 at scale 1 —
  moved there by the author so its output records — driven by `celestina msg`
  and `pkexec`, recorded at 60 fps with `gpu-screen-recorder`, decomposed
  with `ffmpeg` and measured as per-frame regional difference profiles
- **Plan:** [polkit authentication agent](../plans/active/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

The author's complaint was precise: surfaces open background-first and
content-later, close content-first and background-later, sometimes do not
animate at all, and every type should keep its own animation — the fall for
panel menus, the centre scale-and-fade for overlays, the slide routes for
the quiet surfaces — with one shared departure. Each overlay open and close
was recorded before and after every change, so each cause below carries a
frame profile rather than an impression.

## Result

### What the frames showed before

- An overlay opening was one dominant frame — difference profiles like
  `[5.3, 21.5, 5.2]` and `[9.3, 33.6, 12.8]` — the card simply existed.
- Decomposed, the open was worse than instant: the compositor's milky blur
  slab appeared alone and led the card's paint by six to eight frames, then
  the paint arrived essentially at once. That is the author's
  "primero el bg claro y luego el contenido", measured.
- The close was three faint frames of an 80 ms fade.

### The three causes, each proven by its own re-recording

1. **The reveal ran before anyone could see it.** `visible` fires before the
   configure, the first render and the first commit; the entry animation ran
   to completion against discarded buffers, and the first presented frame was
   its final state. The reveal now waits for the configure (the growth past
   the bootstrap width) and then for the first `frameSwapped` that carries a
   presented buffer. Re-recorded: the paint acquired a real ease-out ramp.
2. **The material armed before the paint.** The compositor's region cannot
   fade — it exists or it does not — and both the veil's region and the dense
   companions armed on expose, frames before any paint. Both now publish
   nothing until the field's reveal has begun, so the snap lands under paint
   already forming. Re-recorded: the slab-then-content sequence became one
   card at ~70 % on its first visible frame, settling over five.
3. **The departure was a fade alone, and only sometimes.** `retire()` now
   also shrinks the block into the screen (`retireScale`, centre origin) —
   the author's universal exit — and `softCloseWindow` invokes it on every
   field aboard the window, extends the fade to the theme's fast token, and
   withdraws the compositor region a third of the way in, under paint still
   opaque enough to cover the swap. Re-recorded: closes are four to six
   graded frames, `[6.4, 5.7, 5.9, 2.9]`, with no trailing bare slab.

### What this does not close

The giant disc suspected on the control centre was the wallpaper's own
motif; retracted. The panel menus' fall, the membrane on a second monitor,
the OSD's retreat route and the toast stack could not be driven from here:
the first two need the author's pointer, and the nest shares the desktop's
bus, whose `org.freedesktop.Notifications` is owned by the shell the author
is currently running. Those are the author's pass, on this build.

## Limits

The profiles measure the launcher, the control centre, the notification
centre and the session menu over one wallpaper; the numbers will differ over
other content, the shape of the ramps should not. The suite passes at 23/23,
with three regressions taught that glass publishes only from the reveal
onward — offscreen they now start it by hand, as the first presented frame
does live.
