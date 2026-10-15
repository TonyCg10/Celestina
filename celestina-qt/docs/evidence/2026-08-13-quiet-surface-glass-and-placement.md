# Quiet surfaces: glass, placement, and the lifecycle defects underneath

- **Date:** 2026-08-13
- **Scope:** Celestina unit `PANEL-1-S`
- **Artifact:** Celestina 0.16.0 with CelestinaStyle 1.6.0
- **Environment:** nested niri (`dev-session.sh --own-bus`), one `winit` output
  at 3840x2160, scale 1.5, blur `passes 2 offset 2`; offscreen Qt verification
  for the automated exit; no live-session activation
- **Plan:** [panel glass redesign ledger](../plans/archive/2026-08-08-panel-glass-redesign.md)
- **Validation:** `VAL-PANEL-1`

This records what was measured rather than reasoned about, because the same
symptom — "the card does not appear" — had four different causes over the
session and three of them would have been invisible to a screenshot alone.

## Procedure

Each behaviour was driven through the routes a key or a provider would use —
`notify-send` on the nest's own bus, `wpctl` for volume, the shell's
`brightness-step` session verb for DDC — and read back three ways: screenshots
compared by pixel difference against a baseline of the same frame, the
diagnostics journal, and a `WAYLAND_DEBUG` capture of the protocol dialog.
`scripts/dev-quiet-demo.sh` replays the whole sequence in one run.

## Result

### Automated exit

- CTest 18/18, including the new contracts: the quiet placement arithmetic and
  its zone question (`overlaycontract_test`), the attached and fallback layer
  descriptions (`surfacemanager_test`), the card-file list operations
  (`osdreadings_test`), the display's own attachment and two-card file
  (`tst_sessionosd.qml`), and the toast column's single gripping card
  (`tst_notificationjoin.qml`).
- `qmllint-production.sh` clean; documentation contract clean.

### What the compositor was actually told

`WAYLAND_DEBUG` capture of one display and one toast stack, normalized and
diffed: identical dialogs — same anchors (top|right), same
`set_exclusive_zone(-1)`, same viewport and fractional-scale requests, same
`ack_configure`, same `attach`/`damage_buffer`/`commit` sequence. The display
committed real buffers for a surface niri listed in `niri msg layers` and drew
nothing. That ruled out the QML content (a plain red rectangle in the same
scene was equally invisible), the layer namespace (renaming it to the toast's
changed nothing), the input mask, and the delegate model shape.

### The four causes, in the order they were isolated

1. **A quiet window schedules no first frame of its own.** Its scene renders
   only when something dirties it, so the first commit arrived on the
   provider's two-second poll — longer than a card's 1.8-second life. Fixed by
   kicking one update after mapping, and by keeping the surfaces alive rather
   than mapping one per reading.
2. **Premapping poisoned the overlay layer.** Bringing the persistent surfaces
   up during the shell's own start stopped niri compositing the whole overlay
   layer *and the wallpaper*; disabling the premap brought both back on a
   fresh nest, twice. This also retro-explains the intermittency of every
   earlier run, each of which had premapped before its first reading.
3. **The effect withdraw was gated on `isExposed()`.** That flag flaps on idle
   Wayland windows — a mapped, committing surface reported unexposed — so
   `enableBlurBehind(false)` was skipped and an expired card's region kept
   blurring bare wallpaper. It withdraws unconditionally now and rides the
   heartbeat's next commit.
4. **A dying delegate never republished its glass.** The union kept the dead
   card's region, so the controller never saw it empty. An empty card file now
   publishes empty glass without walking the scene, and the host clears the
   published properties itself, so the withdraw cannot depend on destruction
   timing.

The compositor-effect region was briefly suspected and disabled; that
isolation was confounded by running alongside the premap experiments. With
the real causes fixed, the effect is on and the card renders over live blur —
verified by screenshot through a full cycle: card with blurred wallpaper
behind it, expiry, clean wallpaper, no ghost.

### Also measured

- **The overflow the author reported**: `mapLayerSurface` declared a desired
  size once, so a toast column that grew afterwards committed buffers larger
  than that stale size and niri drew them past the screen edge. The desired
  size now follows the window for every non-centered quiet surface.
- **One command that moved volume and brightness raised one display**, because
  `OsdReadings::apply` returned only the first changed capability. It returns
  the full list now; `osd.pushed … cards 2` was then observed live.
- **Journal events added**: `quiet.placed` (which surface, which placement,
  whether it anchored or yielded, where the card landed) and `osd.pushed`
  (front kind, card count). Both are bounded technical facts, and both were
  what turned this from guesswork into subtraction.

## Limits

This is a nested session on one synthetic output: it proves the descriptions,
the arithmetic and the lifecycle, never how the material reads on the author's
three real monitors, and never per-output behaviour — the nest presents one
output and cannot present two.

The pointer-driven check — raising a card and clicking a panel menu over it —
stays with the author in `VALIDATION.md`: injecting input into the live nested
session is out of scope for an agent. `scripts/dev-quiet-demo.sh` drives every
other behaviour in one run.
