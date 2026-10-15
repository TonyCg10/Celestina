# Quiet surfaces present paint and compositor material together

- **Date:** 2026-08-16
- **Scope:** Celestina unit `R8-P-P`
- **Environment:** Qt 6.11.1; the author's 1920x1080, 60 fps recording
  `recording_20260816_001614.mp4`; offscreen regressions; nested Niri on
  `wayland-2`
- **Artifact:** Celestina 0.29.11 delivery batch, built, verified and deployed
  to the normal test prefix without activating the main session
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

The source contains 778 frames at 60 fps and 1920x1080. Its SHA-256 is
`c2aec9dee5120a8aa37a8e3f709434a3b4a4b7cfabccda791e710c2aa0548c5f`.
Two bottom-right OSD cycles reproduce the same ordering. The first shows bare
weak blur at frames 113–114, the first paint at frame 115 and the dense-material
change at frame 126. During departure, frames 340–341 retain the blur footprint
after paint has almost disappeared, then frame 342 cuts clean. The second cycle
repeats the sequence at frames 383, 386 and 396, followed by bare blur at frames
503–505 and a clean frame 506. The exact 13-frame delay rules out a one-time
mapping accident.

The same recording shows an overlay handoff. Launcher remains intact through
frame 516, frames 517–520 contain neither overlay, Clipboard starts painting at
frame 521, and its dense material changes only at frame 536. The blank handoff
and late material change expose separate map, paint and compositor-material
clocks.

## Ownership correction

- OSD bottom entry begins only when its field reveals. Weak and dense regions
  come from the animated field geometry rather than a static resting footprint,
  so departure cannot leave a bare compositor rectangle.
- `SoftMenuField` publishes material during reveal and scale changes instead of
  waiting `motionNormal + space3xl`; that expression mixed a motion token with
  a spatial token and created the observed 232 ms material snap.
- `OverlayController` is the single presentation gate for all five contextual
  overlays. Readiness is emitted only after non-empty glass reaches the next
  exposed frame swap, and the previous overlay follows idempotent soft
  retirement instead of being destroyed when the replacement maps.
- Every toast placement uses one whole-block entry and departure. Re-entry
  cancels both row and block departure state, and the controller completion
  signal owns normal closure while the 260 ms timer remains only a watchdog.
- Dense and weak blur publication ignore hidden or retiring fields, including a
  pending probe that fires after retirement has begun.

## Result

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
celestina/scripts/complete-production.sh
```

- Focused QML regressions pass for attachment fall 18/18, PerformanceMenu 9/9,
  SessionOsd 19/19 and notification joining 19/19.
- The overlay contract and surface-manager integration tests pass.
- The complete registered verification passes every Rust suite, QML lint, the
  complete QML runner, all 23 CTest contracts and the eight-second release
  smoke.
- The verified 0.29.11 delivery artifact is deployed to `~/.local`; production
  completion does not activate the main session.
- Nested Niri remains PID 80685 with a 1920x1080 scale-1 `winit` output. The
  final build-tree host is PID 464884 on `WAYLAND_DISPLAY=wayland-2` and
  `NIRI_SOCKET=/run/user/1000/niri.wayland-2.80685.sock`.

## Limits

The source recording predates this correction. Automated pixels and a mapped
nested panel do not prove compositor timing after the change. In one 60 fps
recording, trigger two bottom-right OSD cycles and switch Launcher to Clipboard.
Weak blur, dense material and paint must appear, move and leave together; no
bare-material frame, late dense snap or empty overlay-handoff frame is
acceptable. Repeat one toast entry, replacement, re-entry and close only in a
nested session that owns `org.freedesktop.Notifications`; another server owns
that name on the current shared bus.
