# Evidence: menu anchor correction

- **Date:** 2026-08-08
- **Unit:** UX-1-E
- **Scope:** transient panel-menu placement and the author-only incremental
  visual-iteration wrapper
- **Base revision:** `f6f54beb1a086d96e05af7b355e7d9d64db2b018`
- **Environment:** release build and offscreen verification with Noctalia
  retaining ownership of the live session
- **Artifact:** celestina 0.7.0

## Result

Transient panel-menu surfaces now ask the compositor to place them after the
real exclusive zone and place their card relative to the invoking control
inside that surface. This removes the assumption that the Celestina panel is
always the first 40-pixel top surface. The clamp allows only the shadow margin,
not the visible card, to extend past an output edge.

The narrow `scripts/dev-restart.sh` wrapper provides the requested incremental
design loop. It rebuilds only the style plugin and shell target, verifies that
the current session-bus owner is a known Celestina executable before sending
SIGTERM, waits for that process to leave and runs build-tree bytes in the
foreground. It is explicitly non-canonical and was syntax-checked but not run
as part of this non-activating delivery.

## Procedure

Preserve the invoking control's global horizontal anchor, let layer-shell place
the full-output surface after the compositor's real exclusive zone, clamp the
visible card rather than its shadow and cover that contract with focused Qt and
QML tests. Syntax-check the incremental wrapper, then run the canonical
production exit without activating the resulting bundle.

## Automated evidence

- `celestina-surface-manager` covers the full-output, zero-exclusive-zone
  surface contract.
- `celestina-indicator-menu` covers the pure output-relative anchor conversion,
  edge clamping, outside dismissal and the non-modal menu contract.
- `sh -n celestina/scripts/dev-restart.sh`: passed.
- `git diff --check`: passed.
- `celestina/scripts/complete-production.sh`: passed end to end; 29 production
  fixtures, the style checks and smoke, all Rust tests, QML lint, CTest 17/17
  and the eight-second shell smoke passed before the verified 0.7.0 bundle was
  deployed without activation.

## Limits

The author confirmed that the card no longer opened at the hypothetical
unstacked-panel height. The remaining direct switch from one already-open menu
to another still took two clicks in the last live observation. The subsequent
non-modal change has automated coverage only; this record does not call that
live behavior fixed. No shell was activated for this delivery, and Noctalia
remained the session owner.
