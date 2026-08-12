# CelestinaStyle author validation

This queue contains only checks that need the author's real compositor,
perception, input devices or assistive-technology stack. It does not block
[ROADMAP.md](ROADMAP.md).

## VAL-STYLE-01 — Compositor glass and content-icon review

- **Status:** pending
- **Related implementation:** completed S2/S5 glass, the cumulative
  `STYLE-G7-F` prototype snapshot and active `STYLE-G7-J`
- **Requires:** verified production artifacts on the real Niri/Wayland session
- **Procedure:** inspect regular/strong in-scene glass, then the shell's nearly
  transparent external-backdrop carrier, dense matte contextual cards, matching
  panel capsules and fallback; confirm the carrier remains one blur region and
  its bar, body and membrane paint no shadow, outline, lit edge or apparent
  edge halo. Compare attached menus at real widths 328, 360, 424, 460, 530 and
  620 and confirm vertical travel grows to its bounds. For clicked glyphs of
  different widths and positions, confirm the membrane's only seam contact is
  one narrow droplet mouth centred on the exact clicked glyph, clinging to
  the bar with a meniscus on both sides, and that its swell lands tangent on
  the body's flat top edge inside ordinary rounded top corners; confirm the
  mouth clamps enough to remain inside that flat span near either output
  edge, and that the hanging neck thins with tension without ever reading as
  an hourglass suspended between two wide edges. Confirm the clicked control still places
  the menu, keeps only its ordinary hover-circle feedback while that menu
  remains open, and releases it on dismissal. Every panel capsule and dense
  content card must remain geometrically and materially unchanged, and the
  membrane must be solely `ContextualVeil` with no dense bridge. The neck
  width, curves and opener feedback are shell behavior, not new Style tokens
  or API. Inspect folder/file icons at 20, 24, 48 and 128
  logical px over representative light and dark content
- **Pass condition:** blur is visibly real where advertised, the contextual
  field remains subordinate to the denser content/panel material, fallback
  remains readable, no shadow, border halo or sharp wallpaper leak appears
  around the contextual carrier, no panel capsule changes when a menu attaches,
  no dense material enters the membrane, the tension proportions remain
  legible at every tested width and icons retain recognisable shape/ink
  contrast at every size
- **Result:** the final `STYLE-G7-F` consumer cycle ran at scale 1 in the
  existing nested Niri session. Its dark wallpaper retained fixed light/white
  foregrounds over dense dark matte contextual cards and matching panel
  capsules; the outer contextual field remained visibly subordinate and no
  elevation shadow was observed. This is focused agent evidence, not the
  still-pending author review of every menu, output scale and content icon at
  every listed size. The later whole-capsule/open-edge/dense-bridge,
  glyph-mouth, body-wide icon-scaled and fluid body-proportional-waist
  iterations are explicitly superseded — the last was rejected live by the
  author on 2026-08-11 as a strange hourglass — and supply no author
  validation for the current droplet mouth, meniscus, tangent landing,
  persistent opener feedback or immutable-capsule contract. The
  body-wide icon-scaled revision's Celestina focal QuickTest 210/210, canonical
  Style verification and registered Celestina completion verify only that
  previous geometry. For the current droplet revision, canonical
  Style verification passes production-common 29/29, the architecture,
  contrast and QML visual guards, `qmllint` with only the pre-existing
  `CelestinaLineGutter` warnings, CTest 1/1 and the eight-second smoke.
  Registered Celestina completion passes the full Rust suites, CTest 17/17 and
  its eight-second release smoke, then reports the deployed artifacts current
  and verified without activating a session. Automated evidence cannot replace
  this still-pending perceptual result
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

## VAL-STYLE-04 — Shared reading controls at the author's scale

- **Status:** pending
- **Related implementation:** checkpoint STYLE-G7,
  [plan](docs/plans/active/2026-08-04-shared-reading-controls.md)
- **Requires:** the author's real display scale, compositor and pointer, with
  the verified Grafita and Siderita artifacts that consume both components
- **Procedure:** open a long document in Grafita's window and in Siderita's
  embedded editor and quick look; read the gutter's numerals at the author's
  scale in light and dark; drag each scroll bar, click its empty track and
  leave the pointer away from it
- **Pass condition:** the numerals are legible and right-aligned against the
  text without competing with it, the bar is visible at rest and reaches at
  least 3:1 against the surface behind it, and both look and move identically
  in all three surfaces
- **Result:** pending
- **Evidence:** dated captures naming the output scale and colour scheme

## Closed historical observations

`VAL-STYLE-BASE` and `VAL-STYLE-BLUR` are preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).

If a row fails, preserve the result here and open a linked corrective
implementation unit; do not place the patch in this file.
