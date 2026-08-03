# CelestinaStyle author validation

This queue contains only checks that need the author's real compositor,
perception, input devices or assistive-technology stack. It does not block
[ROADMAP.md](ROADMAP.md).

## VAL-STYLE-01 — Compositor glass and content-icon review

- **Status:** pending
- **Related implementation:** completed S2/S5 glass and content-icon work
- **Requires:** verified production artifacts on the real Niri/Wayland session
- **Procedure:** inspect regular/strong in-scene glass, the shell's compositor
  glass and fallback, then folder/file icons at 20, 24, 48 and 128 logical px
  over representative light and dark content
- **Pass condition:** blur is visibly real where advertised, fallback remains
  readable, no sharp wallpaper leaks through the panel region and icons retain
  recognisable shape/ink contrast at every size
- **Result:** not run against the latest tint and content-icon values
- **Evidence:** attach dated captures and note output scale/compositor state

## VAL-STYLE-02 — Keyboard focus and reduced motion

- **Status:** deferred
- **Related implementation:** STYLE-M1
- **Requires:** `STYLE-M1` artifact after its automated exit
- **Procedure:** traverse every finite interactive component by keyboard with
  reduced motion off and on; open/close modal and menu surfaces and return focus
  to the exact invoking control
- **Pass condition:** focus is always visible only for keyboard navigation,
  modal focus cannot escape, restoration is exact and spatial/scale motion is
  absent or instant when reduced motion is enabled
- **Result:** deferred until STYLE-M1 produces its verified artifact
- **Evidence:** dated component matrix and any screen recording needed to show
  motion differences

## VAL-STYLE-03 — Assistive technology and hostile contrast

- **Status:** deferred
- **Related implementation:** STYLE-M1
- **Requires:** real AT-SPI stack and the verified gallery/consumer artifacts
- **Procedure:** inspect roles, names, state and actions for buttons, switches,
  sliders, menus, lists and modal surfaces; review high-risk text/control pairs
  over hostile light/dark artwork or wallpaper
- **Pass condition:** every action is announced and operable, state changes are
  exposed semantically and every tested normal-text pair measures at least
  4.5:1 while every large-text pair measures at least 3:1 in light/dark and all
  enabled, hovered, focused, pressed, selected, disabled and error states
- **Result:** deferred until STYLE-M1 and a real AT-SPI stack are available
- **Evidence:** AT-SPI observations and dated captures

## Closed historical observations

`VAL-STYLE-BASE` and `VAL-STYLE-BLUR` are preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).

If a row fails, preserve the result here and open a linked corrective
implementation unit; do not place the patch in this file.
