# What the first day as the session shell surfaced, read from the journal

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-H`
- **Artifact:** Celestina 0.29.5
- **Environment:** the author's real session — three outputs at two scales,
  stock niri 26.04 — and the diagnostic journal of the author's own use of it.
  Nothing was typed into the session by the assistant.
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

The author reported erratic menus — sometimes unanimated, sometimes opening
detached at the screen's left edge — after the transition. The journal of
their real clicks was mined instead of guessing: every `ctx.menu`,
`ctx.toggle` and `blur.armed` record of the day, in order.

## Result

### The journal reproduced what the author described

- A menu opened and, 19 ms later, a second request for the same menu arrived
  with `same_again: true` — the pair that makes an opening animation vanish,
  because the second request tears down the surface mid-reveal.
- Menus armed their glass at `x: 0` with the body directly under the bar —
  the calendar, whose opener sits mid-bar, placed at the left edge with a
  collapsed height. That is the "detached menu" the author photographed.
- One sequence re-armed the same `x: 0` region nine times at ~500 ms
  intervals: a hover dwell re-firing over a misplaced surface.
- Every one of the author's `ctx.toggle` records says `was_open: false`,
  including two on the same overlay one second apart — each click found the
  previous surface already gone.

### The race that is now closed

The hover dwell and the press race: a click lands right after the dwell
fired, `menuOpen` is set asynchronously by the attachment lease, so the
press's guard reads stale state and requests again. The press now stops the
dwell outright, and a press inside 250 ms of a dwell-opening is treated as
the click that merely confirms it — whether or not the lease has reported.

### The keybind route followed a cursor that does not exist

`OverlayController` chose its output with `QCursor::pos()`, which on Wayland
is stale or zero for a layer-shell client — the same defect the prompt had,
now fixed the same way: every keybind-opened overlay follows the output the
compositor says holds the focused workspace. Before this, `Mod+Space` could
open the launcher on a blacked-out monitor, taking the keyboard with it.

### The prompt joins the shell's material and its closing beat

The authorization prompt was the one surface drawn with no compositor glass
and no exit animation: nothing attached a blur controller to it and its
window was hidden on the spot. It now exposes the same `glassRects` seam
every overlay does, is armed the same way, and closes through the shared
soft-close fade. Its namespace joined the session's and the nest's xray
layer rules.

### What is instrumented rather than fixed

The `x: 0` openers are real but their origin is not yet provable — the
record did not say what geometry the gesture delivered. `ctx.menu` now
carries the opener rectangle, so the next detached menu names its cause
instead of its symptom.

## Limits

The race fix and the output fix follow directly from the journal; the
detached-menu cause is instrumented, not closed. The dense material still
cannot exist on stock niri, and animation judgements ("does the membrane
read right") remain the author's to make on the fixed build.
