# The parked carriers, observed through the compositor's own layer list

- **Date:** 2026-08-20
- **Scope:** Celestina unit `SURF-1-A`/`SURF-1-B`/`SURF-1-C`
- **Artifact:** Celestina 1.0.0 working tree at `c666434`, nested session
- **Environment:** `dev-session.sh --own-bus` with `CELESTINA_DDC=0` (the
  nest's brightness worker must not touch the live session's I²C buses), on
  the author's live session; surfaces observed with `niri msg layers` against
  the nest's own socket, verbs driven with `celestina msg` against the nest's
  own bus. No input was injected anywhere.
- **Plan:** [persistent carriers](../plans/active/2026-08-20-persistent-carriers.md)
- **Validation:** `VAL-SURF-1` (perceptual acceptance, not run here)

## Procedure

The compositor's layer list is the ground truth this unit changes, so every
claim below is a `niri msg layers` reading rather than an inference. The
launcher and control centre were cycled through `launcher-toggle` and
`control-centre-toggle`; toasts through `notify-send` on the nest's own bus;
the fullscreen tenancy through a `kitty` window fullscreened and released with
`niri msg action fullscreen-window` on the nest's socket. Separately, the live
session's own windows were read (read-only) to check the fullscreen
discriminator against real data.

## Result

### The carriers rest mapped and are reused

- Baseline: `celestina-panel`, `celestina-wallpaper`.
- Launcher open: one `celestina-overlay` joins; the three
  `celestina-dense-glass` companions map with the first published sections.
- Launcher closed: **the overlay carrier and all three companions stay
  mapped** — the park, where the pre-SURF-1 shell unmapped five surfaces.
- Launcher reopened: still exactly one `celestina-overlay` — the same mapped
  surface resumed; three open/close cycles never changed the count.
- Control centre open beside it: a second `celestina-overlay`, one per
  controller, both parked after closing.
- A toast mapped `celestina-toasts`; after the server expired it the carrier
  stayed mapped, and a second toast reused it.

### The fullscreen tenancy takes exactly the resting surfaces

With two parked overlay carriers, three parked companions and the parked
toast stack mapped, fullscreening a window in the nest removed **all six**
within a snapshot — only the panel and wallpaper remained. Releasing
fullscreen remapped nothing on its own; the next `launcher-toggle` mapped
fresh and parked again. The whole pipeline ran for real: the adapter's
`Request::Outputs` fetch, the tile comparison, the `fullscreen_outputs`
field, the client's signal and every owner's yield.

### The discriminator against the live session's own windows

Read-only, from the real compositor: a fullscreen window (Moonlight, and the
author's browser while fullscreened) reports `tile_size` exactly the output's
logical size — `[2560.0, 1440.0]` on the 4K at scale 1.5 — while the same
browser tiled reports `[2536.0, 1370.0]`: gaps and the panel's exclusive zone
keep every non-fullscreen tile short of the match, which is what `SURF-1-C`'s
tolerance-1 comparison relies on.

### One defect found and fixed here

The first nest wrote **110 `blur.unavailable` records in ten minutes**
against zero on the live pre-park shell: every park exhausted the blur
probe's fast attempts and printed one fallback record — the journal
amplification class of the 2026-08-12 performance audit. The probe now stops
quietly once its window is parked (after the withdraw, so no armed region
outlives the park). Re-measured on a fresh nest over the same cycles: two
records, the ordinary pre-glass probes. Fixed in `c666434`.

## Limits

- The nest's compositor is niri `main`, not the patched 26.04 release the
  live session runs; what the layer list proves here is the shell's own
  surface lifecycle, not the release compositor's behaviour.
- Everything requiring hands — outside-click and Escape dismissal, keyboard
  focus return, the membrane's look on reuse, and the physical flicker
  itself — remains `VAL-SURF-1` on the live session.
- The nest has one output; the same-output/other-output reuse split is pinned
  headless, not exercised here.
