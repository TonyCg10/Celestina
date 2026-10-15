# PANEL-1-A — borderless glass panel baseline

- **Date:** 2026-08-08
- **Scope:** Celestina `PANEL-1`, unit `PANEL-1-A`
- **Artifact:** celestina 0.10.0, canonical production bundle with
  CelestinaStyle 1.2.0
- **Environment:** Linux, Qt 6.9 compiled-module floor, CMake Release, offscreen
  Qt for automated surface tests and one nested Niri session for author review
- **Plan:** [borderless glass panel](../plans/archive/2026-08-08-panel-glass-redesign.md)
- **Author validation:** `VAL-PANEL-1`, partial

## What changed

The panel no longer paints one hard full-width background. Its window keeps a
soft shadow over the wallpaper, while `PanelPill` supplies borderless fallback
anatomy and the host publishes each visible pill as a finite
`ext-background-effect-v1` blur region. Geometry, visibility and late provider
changes rearm those regions; an empty set withdraws them.

Visible workspaces are positional colour marks and remain grouped by monitor.
Folded monitor groups keep separate count capsules without output names, and
synthetic empty workspace labels do not become visible content. The active
window title was removed from the bar rather than confused with another
workspace.

Network, Bluetooth, audio, brightness, power profile, capture, phone and the
existing shell surfaces use canonical glyphs. CPU and memory retain their
percentages; the phone retains battery state but omits its device name; volume
keeps its full value in the accessible name without painting the percentage.
Permanent buttons now open the control centre, clipboard, notification centre
and session menu that the host already owned.

The tray has two independent startup protections. Its watcher reconciles the
registry after registration, and the QML wrapper follows the published item
model rather than a child's transient effective visibility. Bounded diagnostics
record the watcher snapshot and presentation count without logging foreign
item content.

## Procedure

The candidate was iterated in a nested Niri session while Noctalia retained the
author's primary session. The author reviewed the shadow, blur visibility,
capsule clipping, workspace grouping, status/action glyphs and tray return.
After acceptance, Celestina moved from 0.9.0 to 0.10.0.

The canonical production exit first demonstrated its isolation boundary: under
the restricted runner, 16 of 17 CTest targets passed and the private-D-Bus tray
watcher target could not start. Repeating the registered exit with permission
to create its private bus passed all 17 targets, sealed the exact artifact and
deployed it to the normal test prefix without activation.

## Automated evidence

- `celestina/scripts/complete-production.sh`: canonical build, verification,
  deployment and status completed outside the restricted D-Bus sandbox
- Rust tests: Niri adapter 26, provider adapter 51 plus lifecycle/integration
  tests, `celestina-core` 32, `celestina-shell-core` 314 and `magnetita-core` 98
- QML production lint passed; the complete CTest matrix passed 17/17, including
  the private-D-Bus tray watcher and the focused QML surface targets
- CelestinaStyle verification passed its architecture, colour, contrast, QML
  visual, compiled-module lint, CTest and eight-second gallery smoke checks
- `bash scripts/check-architecture-contract.sh`
- `python3 scripts/version_tool.py check`
- `python3 scripts/check-staged-units.py celestina/docs/inventories/2026-08-08-panel-glass-redesign/PANEL-1-A.numstat.tsv`
- `git diff --cached --check`
- `celestina/scripts/status-production.sh`: artifact current and verified; all
  registered installed files reported `OK`

The compiled CelestinaStyle lint retained pre-existing unqualified-access
warnings in `CelestinaLineGutter.qml`; it passed and this delivery did not
change that file.

## Result

Celestina 0.10.0 is the clean, deployed baseline for continuing the panel
redesign. The author accepted the nested candidate, the tray publishes and
presents its four items, and the registered automated exit is green. The live
shell session was not replaced.

## Limits

The visual result was not recorded as a complete scale-1 and scale-2 matrix, so
`VAL-PANEL-1` remains partial. Automated evidence cannot prove compositor blur,
perceptual contrast, GPU stability or assistive-technology behaviour. Menus,
overlays, clock/calendar/weather and provider semantics remain outside this
baseline.
