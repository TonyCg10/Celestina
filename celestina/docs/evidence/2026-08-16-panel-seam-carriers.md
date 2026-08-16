# Panel-attached buffers start below the panel seam

- **Date:** 2026-08-16
- **Scope:** Celestina unit `R8-P-O`; temporal follow-up is recorded separately
- **Environment:** Qt 6.11.1; the author's 1920x1080, 60 fps recordings
  `recording_20260815_230811.mp4` and `recording_20260816_001614.mp4`;
  offscreen regressions; nested Niri on `wayland-2`
- **Artifact:** Celestina 0.29.11 delivery batch, built, verified and deployed to the normal
  test prefix without activating the main session
- **Plan:** [polkit authentication agent](../plans/active/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

The source is 506 frames at 60 fps and 1920x1080. Its SHA-256 is
`62136e7e2315daf0d5fcbdf634efc3dde4296aa1be39951b06e5c7ef43aab8c8`.
Consecutive zero-based frames, rather than isolated thumbnails, show four
distinct openings whose first buffers occupy the panel strip before the
surface becomes coherent below it:

| Surface | First defective frame | First coherent frame | Observed interval |
|---|---:|---:|---|
| Calendar | 97 at 1.616666 s | 102 at 1.700002 s | cyan card/material overwrites the centre panel before settling below it |
| Tray inventory | 176 at 2.933332 s | 182 at 3.033333 s | field, icons and heading progressively paint inside the panel strip |
| Notification Centre | 271 at 4.516666 s | 280 at 4.666666 s | blur/material and title cover the panel before the complete overlay appears |
| Audio | 317 at 5.283333 s | 326 at 5.433332 s | rows and material alternate between incomplete and over-panel buffers |

The author's same retry confirms that the separate departure synchronization
fixed by `R8-P-N` is no longer the observed defect. It disproves the narrower
claim that a QML reveal gate and a scene clip alone protect the first composed
frame.

## Cause and ownership correction

`QQuickWindow.frameSwapped` reports that a frame was queued for presentation;
it is not proof that the compositor displayed that buffer. A layer surface
covering the complete output could therefore queue a transparent bootstrap
buffer and later expose its first painted buffer over the panel. A
`Popup.Item` also lives in the window overlay rather than underneath the
source item's clip, and the compositor-side dense-glass companion is outside
the QML image entirely. The three branches did not share one spatial owner.

The panel seam is now enforced by the layer-surface buffer boundary:

- `PanelMenuSurface` and panel-attached `OverlaySurface` carriers begin at the
  panel's lower edge through their layer-shell top margin. Their local seam is
  y=0, and opener, glyph anchor, card placement, input mask and live attachment
  refresh use that same carrier-local coordinate space.
- Notification Centre retains its centred, focusable overlay policy while its
  panel-opened carrier receives the inset. OSD and toast carriers use the same
  physical inset while retaining their quiet-surface keyboard policy. Floating
  and keybind routes still cover the output from y=0.
- `SoftMenu` keeps the `Menu` ListView viewport fixed and clipped at the seam;
  the fall translates the viewport's internal content carrier, so rows cannot
  carry their clip upward with them.
- Dense-glass collection intersects every section with the window and all
  ancestor clips before publishing a compositor region. Retirement preserves
  that fixed clip while the rounded region collapses.

Presentation opacity remains an animation and liveness contract. It is no
longer the safety boundary that decides whether a buffer can touch the panel.

## Temporal follow-up from the corrected nest

After the author confirmed that the physical panel seam was fixed, the second
source recording isolated a different defect. It contains 778 frames at 60 fps
and 1920x1080; its SHA-256 is
`c2aec9dee5120a8aa37a8e3f709434a3b4a4b7cfabccda791e710c2aa0548c5f`.

Two bottom-right OSD cycles reproduce the same ordering. The first shows bare
weak blur at frames 113–114, the first paint at frame 115 and the dense-material
change at frame 126. During departure, frames 340–341 retain the full blur
footprint after paint has almost disappeared, then frame 342 cuts clean. The
second cycle repeats the sequence at frames 383, 386 and 396, followed by bare
blur at frames 503–505 and a clean frame 506. The exact 13-frame delay between
the first material footprint and the dense publication rules out a one-time
mapping accident.

The same recording shows an overlay handoff. Launcher remains intact through
frame 516, frames 517–520 contain neither overlay, Clipboard starts painting at
frame 521, and its dense material changes after the geometry has settled at
frame 536. The blank handoff and late material change are independent symptoms
of the same split ownership.

The temporal owners are now unified:

- OSD bottom entry begins only when its field reveals, and its weak and dense
  regions are collected from the animated field geometry rather than a static
  resting footprint. Departure keeps paint and material coupled instead of
  leaving a bare compositor rectangle.
- `SoftMenuField` publishes material during reveal and scale changes instead of
  waiting `motionNormal + space3xl`; the latter mixed a motion token with a
  spatial token and created the observed 232 ms material snap.
- `OverlayController` is the single presentation gate for all five contextual
  overlays. Readiness is emitted only after non-empty glass has itself reached
  the next exposed frame swap, and the previous overlay follows the normal
  idempotent soft retirement instead of being destroyed at map time.
- Every toast placement uses one whole-block entry and departure. A re-entry
  cancels both row and block departure state, and a controller completion signal
  owns normal closure while the old 260 ms timer remains only as a watchdog.
- Dense and weak blur publication ignore hidden or retiring fields, including a
  pending probe that fires after retirement has begun.

## Result

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
celestina/scripts/complete-production.sh
```

- The architecture and version contracts pass.
- The complete registered Celestina verification passes all Rust suites, QML
  lint, the complete QML runner, all 23 CTest contracts and the eight-second
  release smoke. Focused regressions cover panel-menu carrier coordinates,
  live attachment refresh, Popup viewport clipping, Notification Centre focus
  and outside dismissal, OSD/toast inset scaling, and dense-glass clipping at
  scale 1 and at output scale 1.5 with shell scale 1.15.
- The verified 0.29.11 artifact was deployed to `~/.local` and reports current;
  the production script did not activate the main session.
- Nested Niri remains PID 80685 with its 1920x1080 scale-1 `winit` output.
  The final build-tree host restarted there as PID 464884, using
  `WAYLAND_DISPLAY=wayland-2` and
  `NIRI_SOCKET=/run/user/1000/niri.wayland-2.80685.sock`. Noctalia in the main
  session remained PID 1330.

## Limits

Both source recordings predate their respective corrections. Offscreen pixels,
layer-shell margin contracts, a successful production completion and a mapped
nested panel do not prove what the real compositor displayed after the temporal
change. The author confirmed the seam result, but `VAL-PANEL-1` remains pending
until the OSD and overlay timing is repeated live. Toast presentation also
requires a nested session that owns `org.freedesktop.Notifications`; another
server owns that name on the current shared bus.

## Follow-up

In one 60 fps recording on the restarted nested shell, trigger two
bottom-right OSD cycles and switch Launcher to Clipboard. Weak blur, dense
material and paint must appear, move and leave together; no bare-material frame,
late dense snap or empty overlay-handoff frame is acceptable. Repeat one toast
entry, replacement, re-entry and close when Celestina owns the notification
service.
