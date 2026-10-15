# PANEL-1-B prototype sequence — dense content glass and transparent contextual veil

- **Date:** 2026-08-11
- **Scope:** cumulative Celestina unit `PANEL-1-B`; historical prototype label
  `PANEL-1-G`
- **Artifact:** Celestina 0.11.0 with CelestinaStyle 1.3.0
- **Environment:** Release production workflow plus the already-running nested
  Niri `wayland-2` session on output `winit`, 1896 by 998 logical pixels at
  scale 1
- **Plan:** [panel glass redesign ledger](../plans/archive/2026-08-08-panel-glass-redesign.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

### Material boundary

Only two information-bearing shell surfaces adopt the new dense material:
every contextual `MenuSection` and every `PanelPill`. Both consume
`GlassSurface.ContentSurface`, use the output-local foreground decision to pick
the opposite material polarity and have zero elevation. The contextual
`SoftMenuField` alone consumes `ContextualVeil`, leaving its complete backdrop
visibly subordinate to the cards.

`CompositorGlassRegion` remains the only KWindowEffects adapter. A complete
menu publishes one compositor region; its content cards reuse that backdrop
without starting `ShaderEffectSource`, adding another region or creating a
shadow. The permanent panel capsules each retain their finite compositor
geometry and add the same canonical content material beneath their readings.
Other Style glass consumers retain `StandardMaterial`.

Samsung's public guidance supports the restrained depth and grouped-content
boundary but does not publish opacity constants. The `0.64` content strength
is a rounded measurement from the supplied One UI 8.5 crop; the normal
contextual tint is approximately two percent after its translucent source tint
and `0.12` role strength are combined.

## Result

### Automated evidence

- `bash scripts/check-architecture-contract.sh`: passed.
- Focused CTest for `celestina-indicator-menu`,
  `celestina-overlay-contract`, `celestina-surface-manager` and
  `celestina-output-chooser`: 4/4 passed. The contracts require one contextual
  veil, uniform dense sections, matching dense panel material, no QML capture,
  zero elevation and one compositor region per menu.
- CelestinaStyle's focused QuickTest passed 1/1 and its canonical 1.3.0
  production verification passed 29 production-common tests, compiled QML
  lint, CTest 1/1 and the eight-second gallery smoke.
- `celestina/scripts/complete-production.sh`: all Rust unit and integration
  suites, QML lint and QuickTests, CTest 17/17 and the eight-second release
  smoke passed. The verified 0.11.0 bundle was deployed to the normal test
  prefix without activating the host session.
- `python3 scripts/version_tool.py check`: passed for all six owners.

### Nested-session evidence

Before mutation, the process tree proved that PID 1224284 was the Celestina
owner using `wayland-2` and
`/run/user/1000/niri.wayland-2.1144687.sock`. The registered
`dev-session.sh --restart` command replaced only that owner with PID 1336218
and adapters 1336400 and 1336401. Nested Niri remained PID 1144687, host Niri
remained PID 1224 and Noctalia remained PID 1276.

`celestina msg control-centre-toggle` returned `confirmed`. The live overlay
armed one compositor shape with 30 region fragments; the observed startup and
interaction stream contained no QML construction, required-property or binding
error. `/tmp/celestina-oneui-content-glass.png` records the contextual surface
without unrelated desktop content, and `/tmp/celestina-oneui-panel-glass.png`
records the complete panel row.

The active wallpaper is bright around the menu. The capture therefore also
exercises the adaptive inverse pair: dense light matte cards and panel capsules
carry dark ink, while the contextual field remains nearly transparent and the
wallpaper is still recognizable around and between cards. No elevation shadow
or second blur region is visible. The parallel portal application-id warning
and oversized wallpaper decoder refusal are established nested-session
limitations unrelated to this material change.

## Limits

This controlled cycle proves construction, material hierarchy, one-region
ownership and isolation from the host session at scale 1. It is agent-run
visual evidence, not author acceptance of every menu, scale, dark foreground
pair or hostile application background. Those checks remain explicitly queued
in `VAL-PANEL-1`.
