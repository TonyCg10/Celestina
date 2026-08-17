# Night-light gamma transition

- **Date:** 2026-08-17
- **Scope:** Celestina unit `R8-P-Q` night-light follow-up
- **Environment:** Qt 6.11.1; Rust Wayland protocol tests; nested Niri `winit`
  output without gamma-control; registered production verification
- **Artifact:** Celestina 0.29.12 delivery batch, built, verified and deployed
  to the normal test prefix without activating the main session
- **Unit:** `R8-P-Q` night-light follow-up
- **Reported defect:** activating or deactivating night light produced a brief
  whole-output warm colour flash that was much easier to perceive physically
  than in an ordinary screen recording.

## Procedure

The old provider treated night light like the caffeine inhibitor: process
liveness was the state. It spawned `wlsunset -T 2701 -t 2700` and published
`active` as soon as the child existed. `wlsunset` acquired gamma control and
applied its 2700 K table in one commit; killing the child on deactivation made
the compositor restore identity in one commit. There was no temporal owner
between those two endpoints, and a child that failed after spawning could be
reported active until the two-second liveness poll noticed it.

The control-centre switch had a separate one-frame reversal. Qt changed its
local `checked` value optimistically, QML rebound it to the still-old provider
frame, and the confirmed provider frame changed it again. The resulting
`false -> true -> false -> true` sequence restarted both thumb and track
animations even when gamma itself was unavailable.

Night light now has one Wayland-owning worker in the aggregate provider. Pure
core code calculates the exact former 2700 K white point and a 19-sample,
300 ms smoothstep transition. The adapter creates one exclusive
`wlr-gamma-control` object per output, publishes and persists the active state
only after the last warm table is confirmed, reaches and confirms identity
before releasing the objects, and handles hotplug and asynchronous `failed`
events on the same thread. Every frame owns a new native-endian memfd retained
until the confirming round trip; reusing one would let shared descriptor
offset/content race the compositor's read.

No protocol wait is an unbounded `EventQueue::roundtrip`. Discovery, final
gamma confirmation and object release use `wl_display.sync` callbacks pumped
at 25 ms intervals under explicit deadlines. Each interactive request also
owns an atomic pending/committed/cancelled permit. If its caller times out, the
worker cannot publish or persist that target later and restores the last
confirmed state; if commit wins first, the caller cannot misreport that valid
commit as a timeout.

The switch remains provider-owned. Pointer, keyboard and assistive activation
send one request without changing `checked`; the confirmed frame moves it once.
Caffeine remains the process-backed `systemd-inhibit` hold.

## Result

- `cargo test --manifest-path celestina/Cargo.toml --bin
  celestina-provider-adapter --offline`: 90 passed, 0 failed, including request
  cancellation, commit/timeout ordering, operation deadlines and shutdown.
- `cargo test --manifest-path celestina-rs/Cargo.toml -p
  celestina-shell-core --offline`: 329 passed, 0 failed, including exact 2700 K
  whitepoint, both monotonic directions, exact endpoints and native LUT bounds.
- `cargo test --manifest-path celestina/Cargo.toml --test held_shutdown`: 1
  passed, 0 failed; the remaining process-backed hold still releases on helper
  shutdown.
- Provider-adapter and shell-core clippy with `-D warnings`: passed.
- Rust formatting check: passed.
- Focused `tst_controlcentre.qml`: 8 passed, 0 failed; activation sends one
  request with no optimistic `checkedChanged`, and provider confirmation causes
  exactly one change.
- `cmake --build celestina/build --target celestina celestina_qmllint -j2`:
  passed. QML lint retained only the three pre-existing diagnostics in
  `BrightnessLevel.qml`, `CalendarMenu.qml` and `SoftCard.qml`.
- `bash scripts/check-architecture-contract.sh`: passed.
- `celestina/scripts/complete-production.sh`: passed outside the restricted
  sandbox. The registered build, all Rust suites, all 23 CTest targets,
  qmllint, both production smokes, deployment to `~/.local`, and final artifact
  status completed successfully for Celestina 0.29.12.
- The authorized nested restart replaced host PID 689217 with build-tree host
  PID 757796 and provider-adapter PID 757987 on `WAYLAND_DISPLAY=wayland-2`.
  The host mapped the 1920x1080 `winit` output without a QML construction
  error, the provider reported the absent gamma-control global truthfully, and
  no `wlsunset` process remained.

## Limits

`VAL-NIGHT-1` requires a real Niri TTY output and an external camera or
colorimeter. Output gamma is applied after the ordinary captured scene, so the
screen recording that revealed the interaction is not sufficient proof of the
physical ramp. The development nest's winit backend does not advertise gamma
control; its useful regression is the truthful refusal path: no warm flash, no
optimistic switch movement and no persisted active state.
