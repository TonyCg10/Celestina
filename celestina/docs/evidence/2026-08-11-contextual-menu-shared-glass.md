# PANEL-1-B prototype sequence — shared contextual-menu glass

- **Date:** 2026-08-11
- **Scope:** cumulative Celestina unit `PANEL-1-B`; historical prototype label
  `PANEL-1-F`; and a read-only suite glass inventory
- **Artifact:** Celestina 0.11.0 with CelestinaStyle 1.3.0
- **Environment:** Release production workflow plus the already-running nested
  Niri `wayland-2` session on output `winit`, 1896 by 998 logical pixels at
  scale 1
- **Plan:** [panel glass redesign ledger](../plans/active/2026-08-08-panel-glass-redesign.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

### Inventory result

Siderita, Grafita and Fluorita consume the canonical `Glass*` components through
their registered relative links to `celestina-style`. Magnetita registers the
canonical `GlassSurface` but currently instantiates no glass UI. No application
contains a copied `Glass*` implementation.

Magnetita's media-card blur and Fluorita's ambient-light effect process their
own artwork. They do not capture the scene or reproduce the glass material and
therefore remain content effects rather than parallel glass implementations.

Celestina's legitimate local seam is `PanelBlurController` plus
`CompositorGlassRegion`: it asks KWindowEffects for one finite real blur region
and supplies a fallback when that compositor facility is unavailable. The
outer menu veil and every `MenuSection` now delegate tint, noise, outline and
lit edge to `GlassSurface.ExternalBackdrop`. Neither starts a QML capture or
publishes another compositor region.

## Result

### Automated evidence

- `bash scripts/check-architecture-contract.sh`: passed.
- Focused CTest for `celestina-indicator-menu`, `celestina-overlay-contract`,
  `celestina-surface-manager` and `celestina-output-chooser`: 4/4 passed. These
  construct all contextual carrier families, require external-backdrop material
  on the outer veil and content sections, require capture to stay inactive and
  retain exactly one compositor region.
- `celestina/scripts/complete-production.sh`: Rust unit and integration tests,
  QML lint and QuickTests, CTest 17/17 and the eight-second release smoke passed.
  The verified 0.11.0 bundle was deployed to the normal test prefix without
  activating the host session.
- `python3 scripts/version_tool.py check`: passed for all six owners.

The first restricted verification attempt could not bind the private D-Bus
socket used by `celestina-tray-watcher`. Repeating the canonical command outside
that sandbox passed the exact test and all 17 CTest cases; this was an execution
restriction, not a product failure.

### Nested-session evidence

The registered `dev-session.sh --restart` command replaced only the previous
nested Celestina owner, PID 1214100, with PID 1224284 and adapters 1224469 and
1224470. Its environment names `wayland-2`, socket
`/run/user/1000/niri.wayland-2.1144687.sock` and the current Style/build-tree
paths. Nested Niri remained PID 1144687, host Niri remained PID 1224 and
Noctalia remained PID 1276.

`celestina msg control-centre-toggle` returned `confirmed`. The live overlay
armed one compositor shape with 30 region fragments; the observed startup and
interaction stream contained no QML construction, required-property or binding
error. `/tmp/celestina-shared-glass-menu.png` is a menu-only capture from that
state. Wallpaper structure remains visible through the light outer carrier and
through each denser content section, matching the intended hierarchy without an
elevated shadow. The parallel portal application-id warning and oversized
wallpaper decoder refusal were already established properties of this nested
session and are unrelated to the material consolidation.

## Limits

This live cycle proves construction, one-region ownership and isolation from the
host session. The author-owned perceptual comparison of the material across all
menus, scales and hostile application backgrounds remains pending in
`VAL-PANEL-1`; no offscreen or log evidence substitutes for that review.
