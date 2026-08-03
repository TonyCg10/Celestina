# celestina — Niri shell local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It defines the
shell's Rust/C++/Qt/QML boundary; it does not authorize activating surfaces,
editing Niri, hiding Noctalia, or installing tools.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- Run `python3 ../scripts/agent-context.py celestina` to discover the registered
  active plan.
- [Open discussions](docs/discussions/README.md) and
  [accepted decisions](docs/decisions/README.md)

Apply expert Rust, C++20, Qt 6/QML, and CXX-Qt judgment at every boundary,
ownership edge, thread, signal, and build registration point.

## Shell boundaries

- `celestina-shell-core` owns framing, vocabulary, reduction, and pure policy.
  It does not depend on Qt or know surfaces.
- `src/niri_adapter.rs` is the only owner of `niri-ipc` types. It publishes
  narrow snapshots and confirmations; it never leaks the full Niri model to Qt.
- `src/provider_adapter/` is the aggregated long-lived non-Qt IO helper. A
  feature adds a module to that runtime, not one process per widget or a second
  parallel channel.
- The helper/host protocol is bounded, line-delimited, backward-compatible JSON.
  The host revalidates, clears state when a generation is lost, and transports
  `u64` IDs as decimal strings. `accepted` is not confirmation; a later snapshot
  containing the requested state confirms it.
- Manual C++ owns only LayerShellQt/KWindowEffects, QtDBus, `QProcess`,
  marshaling, and lifecycle gaps CXX-Qt cannot cover. Every new seam names that
  limitation; domain and policy do not move to C++.
- QML receives adapted state. It does not open sockets, execute processes, or
  decide recovery/protocol behavior. Register every QML file in CMake.

## Session channel

- The panel host owns `org.celestina.Shell` and exports
  `org.celestina.Shell1` at `/org/celestina/Shell1`. Only panel mode owns that
  name.
- `msg` and `--pick-output` are transient clients; they never claim the name or
  start a shell.
- Preserve `GetState`, `Command`, `Changed`, and `CommandResult`. Extend `a{sv}`
  additively and version every payload.
- Every session key binding enters through this channel. A helper's internal
  pipe is not a second public API.
- Bus loss degrades the channel instead of blocking or terminating the panel.

## Surfaces, tray, and style

- Create one layer-shell surface per `QScreen`, with explicit namespace,
  anchors, exclusive zone, and keyboard policy. Hotplug changes only the
  affected surface.
- Blur is best-effort: use a finite region, reapply on geometry changes, and
  publish a readable fallback. Offscreen tests prove no compositor decision.
- Panel, menus, and overlays describe their intersection through
  `LayerSurfaceSpec`; new surfaces do not copy setup code.
- `TrayWatcher` hosts items and `TrayWatcherService` claims
  `org.kde.StatusNotifierWatcher` only when no owner exists. Calls to foreign
  applications are asynchronous and every property may be absent.
- Foreign tray icons are the only local exception to the closed icon catalogue;
  their names/pixels retain another app's identity. This does not extend to
  first-party icons.
- Import canonical `../celestina-style` through equivalent CMake/runtime URI
  aliases. Never copy it or assume an installed module.
- `--pick-output` prints exactly `Monitor: <output>` to stdout on acceptance;
  logs use stderr and cancellation invents no selection.

## Production and verification

- `scripts/build-production.sh`: CMake Release host and both Rust helpers,
  without mapping surfaces.
- `scripts/verify-production.sh`: guards, Rust checks, QML lint, CTest, and smoke
  against the same built bundle, without activation.
- `scripts/status-production.sh`: host/helper/style currency and digests.
- `scripts/complete-production.sh`: build once, verify, and update the normal
  on-disk bundle without replacing the live session.
- `scripts/activate-production.sh`: the only entry that starts the real surface;
  never run it during verification.

`scripts/run.sh` is human compatibility, not canonical evidence. Do not run it,
hide another bar, or mutate the live session without an explicit request.
Build/CTest/qmllint prove compilation or isolated behavior. Geometry, hotplug,
blur, focus, tray takeover, physical monitors, lock, and AT-SPI belong only in
`VALIDATION.md` when the author requests that pass.
