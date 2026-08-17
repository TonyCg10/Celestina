# R8 final corrective batch

- **Date:** 2026-08-17
- **Scope:** Celestina unit `R8-P-Q`
- **Environment:** Qt 6.11.1; Rust and QML regressions; registered production
  build and verification; nested Niri on `wayland-2`
- **Artifact:** Celestina 0.29.12 delivery batch, built, verified and deployed
  to the normal test prefix without activating the main session
- **Unit:** `R8-P-Q`
- **Version:** Celestina 0.29.12
- **Detailed records:** [pinned tray attachment](2026-08-16-pinned-tray-menu-attachment.md), [control-centre reading transition](2026-08-17-control-centre-reading-transition.md), and [night-light gamma transition](2026-08-17-night-light-gamma-transition.md)

## Procedure

The final R8 batch closes three defects found through author review of the same
panel interaction system. A pinned tray application's foreign menu now carries
the real opener, glyph and application title through the standard attached
surface contract; its membrane and rows enter as one block, and stable pinned
icons fade independently to the inventory opener's left. Provider-owned labels
in Control Centre cross-fade instead of replacing every glyph in one frame.
Night light no longer treats an external `wlsunset` process as state: one
Wayland-owning worker applies the exact former 2700 K endpoint through a
19-sample, 300 ms smoothstep transition, publishes only a confirmed endpoint,
and restores the last confirmed state when a bounded request is cancelled.

The night-light switch is likewise provider-owned. Activating it sends one
request without optimistically moving the thumb; the later provider frame
causes the only checked-state change. Every gamma-control sync is deadline
bounded, no timed-out request can commit late, and the worker restores identity
within a bounded shutdown path.

## Result

- `bash scripts/check-architecture-contract.sh`: passed.
- `python3 scripts/version_tool.py check`: passed for all six owners.
- `cargo test --manifest-path celestina/Cargo.toml --bin celestina-provider-adapter --offline`: 90 passed, 0 failed.
- `cargo test --manifest-path celestina-rs/Cargo.toml -p celestina-shell-core --offline`: 329 passed, 0 failed.
- Focused `tst_controlcentre.qml`: 8 passed, 0 failed.
- `celestina/scripts/complete-production.sh`: passed outside the restricted
  sandbox. All Rust suites, all 23 CTest targets, qmllint, both production
  smokes, deployment to `~/.local`, and final artifact status completed
  successfully.
- The restarted nested host is PID 757796 with provider adapter PID 757987 on
  `WAYLAND_DISPLAY=wayland-2`; it mapped the 1920x1080 `winit` output, refused
  the unavailable gamma-control protocol truthfully, and left no `wlsunset`
  process.

## Limits

`VAL-PANEL-1` retains the perceptual tray/menu and Control Centre checks.
`VAL-NIGHT-1` requires a real Niri TTY output and an external camera or
colorimeter because the nested `winit` backend does not advertise gamma
control and ordinary screen capture does not contain the physical output LUT.
