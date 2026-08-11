# PANEL-1-B final prototype snapshot — fixed white shell ink

- **Date:** 2026-08-11
- **Scope:** cumulative Celestina unit `PANEL-1-B`; historical prototype label
  `PANEL-1-H`; removal of the wallpaper-adaptive foreground pipeline while
  preserving wallpaper identity, gallery/import behaviour and the established
  shell glass hierarchy
- **Artifact:** Celestina 0.11.0 with CelestinaStyle 1.3.0
- **Environment:** release production workflow plus the already-running nested
  Niri `wayland-2` session on output `winit`, 1896 by 998 logical pixels at
  scale 1
- **Plan:** [panel glass redesign ledger](../plans/active/2026-08-08-panel-glass-redesign.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

The shell-core wallpaper contrast module and its pixel-sampling, crop,
candidate-comparison and hysteresis policy were removed. The provider no
longer publishes `wallpaper-appearance`; its replacement
`wallpaper-identity` publication contains only output, source, generation, file
revision and optional geometry. That non-visual identity remains necessary so
replacing image bytes at one unchanged path still invalidates Qt's image
request without coupling the wallpaper to foreground colour.

The host now validates only that identity and no longer exposes a wallpaper
appearance QObject or propagates tone, uncertainty or blur-derived polarity to
QML. `BackdropInk` has no input properties: text and glyph roles resolve to
`CelestinaTheme.text`, content cards and panel capsules use the dark
`CelestinaTheme.canvas` tint, and the contextual carrier retains the
near-transparent `CelestinaTheme.glassHighlight` tint also reused by the local
low-opacity text-field plate. Existing interaction, gallery selection, bounded
image validation, atomic import and compositor region ownership remain
unchanged.

## Result

### Automated evidence

- `bash scripts/check-architecture-contract.sh`: passed.
- Provider and shell-core Rust suites passed 77/77 and 322/322 respectively;
  their Clippy runs passed with `-D warnings`.
- Focused CTest passed 4/4 for surface management, overlay construction,
  indicator menus and the complete QML QuickTest target.
- `celestina/scripts/complete-production.sh` passed the registered Rust suites,
  QML lint and QuickTests, CTest 17/17 and both eight-second release smokes.
  The verified 0.11.0 bundle was deployed to the normal test prefix without
  activating the host session.
- `python3 scripts/version_tool.py check`, the documentation contract and
  `git diff --check` passed.
- A source-wide symbol audit found no active `wallpaper_contrast`,
`wallpaper-appearance`, `WallpaperAppearance`, `inkTone`, `inkUncertain` or
`usesDarkInk` reference in source, QML, tests or build registration; the
retired runtime tokens are also absent from the installed host and provider
binaries.

The first restricted canonical run could not bind the private D-Bus socket
used by the tray-watcher test. Repeating the exact production workflow outside
that sandbox passed the tray-watcher case and all 17 CTest cases; the failure
was an execution restriction rather than a product failure.

### Nested-session evidence

Before the restart, the process tree identified nested Niri PID 1349248 and
Celestina PID 1349330 on `wayland-2` with socket
`/run/user/1000/niri.wayland-2.1349248.sock`. The registered
`dev-session.sh --restart` command replaced only that shell with PID 1424970
and adapters 1425141 and 1425145. Nested Niri remained PID 1349248, host Niri
remained PID 1224 and Noctalia remained PID 1276.

`celestina msg control-centre-toggle` returned `confirmed`. The rebuilt panel
armed seven compositor shapes and the control centre constructed over the
current dark wallpaper with the fixed white foreground, dense dark content
material and near-transparent outer carrier. The observed startup and opening
stream contained no QML construction, required-property or binding error.
`/tmp/celestina-fixed-white-ink.png` records that nested output.

## Limits

This controlled cycle proves the fixed light instance, construction and host
session isolation at scale 1 on the current dark wallpaper. It does not claim
author acceptance of every contextual surface, scale 2 or a bright-wallpaper
perceptual comparison. The source and automated contracts prevent a polarity
switch, but the remaining bright/dark application and wallpaper visual matrix
stays author-owned in `VAL-PANEL-1`.
