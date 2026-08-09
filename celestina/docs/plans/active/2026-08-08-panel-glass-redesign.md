# PANEL-1 — borderless glass panel

- **Opened:** 2026-08-08
- **Plan ID:** panel-glass-redesign
- **Status:** active
- **Scope:** celestina
- **Implementation checkpoint:** PANEL-1
- **Predecessor:** [WMAP-1 — the workspace window map](../archive/2026-08-08-workspace-window-map.md)
- **Decision:** [ADR 0002](../../decisions/0002-borderless-glass-panel.md)
- **Author-validation checkpoint:** `VAL-PANEL-1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

A transparent layer surface can read as a bar without becoming a solid strip:
a soft shadow establishes its edge, and finite compositor-blur capsules give
each content group enough visual separation. The blur remains visible only if
the QML drawn above it stops covering it with opaque tint and strokes.

## Tangible outcome

The panel has no hard full-width background. Its content sits on borderless
capsules that visibly disperse wallpaper detail, including the phone capsule at
both horizontal ends, while a soft shadow fades into the desktop below.

## Scope

- The panel surface, shadow and content grouping.
- One local capsule component and its readable no-blur fallback.
- Finite compositor blur regions, dynamic geometry updates, withdrawal and
  protocol commit/flush behaviour.
- Flank sizing and phone geometry needed to keep the outer capsule whole.
- One consistent panel-reading type/ink scale and the status-glyph consumers
  needed to replace network, Bluetooth and audio labels while pairing CPU and
  memory percentages with canonical glyphs.
- A workspace strip whose visible workspaces are colour state marks and whose
  other-monitor groups retain their compact count capsules, without workspace
  numbers, output names or active-window titles.
- Permanent icon entry points for the control centre, clipboard, notification
  centre and session menu, wired to the overlays the host already owns.
- Icon-only power-profile, volume and brightness readings, with their full
  values retained in accessible names.
- A delayed tray-registry reconciliation and model-driven wrapper visibility so
  a restarted host cannot remain empty after either a premature watcher
  snapshot or the initially empty QML state; bounded diagnostics cover both
  seams.
- The narrow canonical Lucide catalogue additions those consumers require;
  they remain a separately owned CelestinaStyle delivery and are not absorbed
  into the `celestina:` inventory.
- A nested Niri reference profile and exact opt-in live-session instructions.
- Automated construction/geometry checks and author visual checks at scale 1
  and scale 2.

## Exclusions

- Restyling menus, overlays, notifications, OSD or the workspace map.
- Clock, calendar or weather feature work.
- Provider, DDC, media, network or Bluetooth behaviour.
- Tray-item semantics, activation, menus or ownership; only host-side startup
  reconciliation, presentation visibility and their bounded diagnostics are
  included.
- Any shared-style change beyond the exact status glyphs consumed by this bar.
- Editing the author's live Niri configuration or replacing the live shell.
- The unrelated pending wallpaper-provider correction in this worktree.

## Build Order

1. Establish one larger primary-ink scale for panel readings, replace the named
   textual status labels with canonical icons, and remove the phone's visible
   device name without changing provider semantics.
2. Reduce visible workspaces to coloured interactive state marks, preserve the
   original folded grouping for other monitors without visible output names,
   remove the active-window label, and add permanent buttons for the existing
   overlays.
3. Reconcile a foreign tray registry after host registration and make its QML
   wrapper follow the independent item model so neither an empty snapshot nor
   an initially hidden child can strand the drawer.
4. Remove the capsule stroke and every successful-blur fill that hides the
   compositor result; retain one borderless fallback.
5. Preserve the full-width scrim as a shadow rather than a second panel fill.
6. Make region updates and protocol commits follow every capsule geometry
   change, including late providers and empty state.
7. Keep the phone capsule within the flank without clipping either cap.
8. Tune the nested Niri blur profile from the official offset/pass contract,
   then compare the result with the author's reference crop.
9. Add construction and geometry regressions, run the canonical exit only after
   the author accepts the visual direction, and document the optional live
   compositor profile without applying it.

## Implementation exit

- A busy wallpaper loses recognizable detail inside every capsule while staying
  sharp immediately outside it.
- No capsule has a visible border or opaque successful-blur fill, and the phone
  capsule retains both round ends.
- Provider insertion, removal and width changes rearm the finite region without
  blurring the complete 112-pixel panel surface.
- Build, QML lint, focused surface tests, the architecture guard and the
  canonical production exit pass before delivery.
- Scale 1 and scale 2 author screenshots are recorded separately as
  `VAL-PANEL-1`; an automated smoke never claims the visual pass.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| PANEL-1-A | `celestina:` | active | `qml/Panel.qml`, `qml/PanelPill.qml`, panel flanks/readings and action buttons, `src/panelblurcontroller.*`, `src/panelmanager.*`, tray startup/presentation reconciliation, nested-session config, tests and this plan; exact `CelestinaIcons` glyph additions are a separately delivered style dependency | pending | Replace the hard panel plate with a soft shadow and borderless real compositor-glass capsules, reduce workspace and status readings to positional colour/icon semantics without discarding monitor grouping or CPU/memory values, expose the existing overlays, and keep the tray populated and visible across host restarts | pending | `VAL-PANEL-1` |

## Active unit boundary

`PANEL-1-A` is the only open unit. It may change presentation, blur-region
ownership, typed overlay entry points, the tray host's bounded startup and
presentation reconciliation, and the visual harness named above. It does not absorb the
unrelated wallpaper-provider edit already present in the worktree or provider
logic. The icon catalogue additions are owned and verified by CelestinaStyle
and must not be included in this unit's eventual inventory or commit.
