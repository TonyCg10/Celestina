# DRAWING — the shell's drawing system (SIMPLE-2, 2026-08-28)

The author's direction: stop patching the inherited drawing code and redo
it, so the shell wears macOS's anatomy in the frosted Samsung glass the
compositor already provides. This document is the contract; nothing
visible is drawn outside it.

## What the audit found (why from scratch)

Three uncoordinated background painters — `GlassSurface` (the old
multi-mode material, with its own shadow and its own tokens),
`MenuSection` (SIMPLE-1's mica card) and `PanelPill` (its copy for the
bar) — plus a `SoftMenuField` of 800 lines that was mostly dead machinery
from the pre-reset system (falls, membrane silhouettes, entry clips).
Every visual calibration had to be repeated in N places and one was always
missed: the shadow was corrected in the field and stayed smoky in the
overlays, because `GlassSurface` kept its own. That is the structural
defect this contract removes.

## The visual target (FINAL form, settled by the author 2026-08-28)

macOS × Samsung, with no containing backdrop:

- **The glass lives ONLY in the content cards**: each section is a piece
  of colour summary (the compositor's strong blur) under the `elevated`
  tint at 0.55 with a hairline. A menu is its group of loose cards over
  the desktop — a containing panel was tried and rejected on sight.
- **The background is shade, not a surface**: what a contextual surface
  paints behind its cards is the block's soft shadow plus the backdrop
  scrim it lays over its whole output — measured off the author's own
  macOS screenshots, where the sky far from the panel darkens exactly as
  much as the pixels beside it. Quiet surfaces (toasts, the display) never
  dim: a notification must not take the screen.
- **One shadow implementation**: `CelestinaShadow`, two analytic layers
  (contact + ambient), each inflated by twice its blur so the falloff
  reaches nothing instead of cutting a hard rectangle. A second shadow
  implementation is how this shell once shipped one calibrated surface and
  one smoking one at the same time.
- **One animation**: the field's 150 ms fade. Nothing else moves.

## The primitives (and nothing else paints)

- `MenuSection` (celestina/qml) — THE glass card: region marker (inset 2,
  for QRegion's unsmoothable corner steps) + `elevated` tint at 0.55
  (`celestina-panel-tint`) + hairline. It carries the objectName
  `celestina-menu-section` and `cornerRadius`: that pair is what the dense
  channel collects, one shape per card.
- `ShellPanel` (celestina/qml) — the same anatomy with an optional shadow
  of its own; today only the bar's capsules use it (pill radius, no
  shadow). A `ShellTile` (flat interior fill) was tried with the
  containing panel and died with it.
- `CelestinaShadow` (celestina-style) — the two layers. A second shadow
  implementation is forbidden.

Rules:

1. A contextual surface (menu, overlay, display, toast) is one
   `SoftMenuField` that paints only its backdrop — the scrim
   (`dimsBackdrop`) and the block's shadow (`castsShadow`) — behind its
   `MenuSection` cards. N cards = N regions = N dense shapes; one shadow
   per block.
2. The bar: capsules are ShellPanels with pill radius and no shadow; the
   strip itself paints nothing.
3. No component declares a background of its own: no card `Rectangle`, no
   `GlassSurface` (which remains only for the style gallery), no loose
   shadows.

## Compositor channels (no C++ contract changes)

- Weak per-window blur: only the bar's panel arms it (`weakArm`); it
  samples the capsules with its namespace's strong profile.
- Dense (companions): collects MenuSection/ShellPanel by objectName — one
  shape per glass card.
- The region beat is driven by the field's `glassRects` publication
  (compared fingerprint; the 500 ms heartbeat covers what no signal
  announces).
