# Celestina Desktop

Celestina Desktop is the shell/session component of the suite for a personal
Niri/Wayland environment. It owns a truthful per-output panel, keyboard
overlays and a versioned session command service while Noctalia responsibilities
are replaced in reversible phases.

## Current capabilities

- One 40 px layer-shell panel per output with output-local Niri workspaces,
  active window and confirmed focus requests.
- Clock, phone, CPU/RAM, media, audio/microphone, network, Bluetooth, power
  profile, per-monitor DDC brightness and screenshot request surfaces.
- StatusNotifierItem host, DBusMenu rendering and a watcher service that takes
  over only when no other watcher owns the session name.
- Keyboard launcher over desktop entries and a shell-owned clipboard-history
  overlay.
- `org.celestina.Shell1` plus the transient `celestina msg` client.
- `--pick-output` chooser used by `xdg-desktop-portal-wlr`.

Current implementation state and the next phase are in
[STATUS.md](STATUS.md). Manual Niri/hardware checks are deliberately separate in
[VALIDATION.md](VALIDATION.md).

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/celestina-shell-core/` | Pure framing, provider/command vocabulary and policy |
| `src/niri_adapter.rs` | Pinned Niri IPC reduction and compositor actions |
| `src/provider_adapter/` | Aggregate bounded non-Qt provider IO |
| `src/*.cpp`, `src/*.h` | Qt process, D-Bus and layer-surface adaptation |
| `qml/` | Panel, menu, launcher, clipboard and chooser presentation |
| `../celestina-style/` | Canonical visual tokens and controls imported from source |
| `tests/` | Rust, QtTest and Qt Quick Test coverage |

The host is C++20 with Qt 6.9+, LayerShellQt 6.6+ and KF6WindowSystem 6.19+.
The helpers are Rust 2021 and the Niri adapter pins `niri-ipc` to the compatible
session protocol version declared in `Cargo.toml`.

## Build and verify

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
```

Build creates the production bundle without mapping a panel. Verify consumes
that same bundle and never activates the shell. Status reports whether the host,
both Rust helpers and source-imported style inputs are still current.

Activating a shell changes the live session and is intentionally separate:

```sh
scripts/activate-production.sh
```

Run it only when that live mutation is intended. `scripts/run.sh` remains a
human compatibility wrapper; it is not the verification entry point.

## Session command client

The already-built host also acts as a transient client:

```sh
celestina msg get-state
celestina msg launcher-toggle
celestina msg clipboard-toggle
```

Client mode does not start a shell or claim the session service name. Panel mode
requires a live Wayland compositor with layer-shell support; `offscreen` is a
test mode and never evidence about Niri geometry, focus or blur.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent rules](AGENTS.md)
- [Open discussions](docs/discussions/README.md)
- [Accepted decisions](docs/decisions/README.md)
- [Historical replacement work orders](NOCTALIA-REPLACEMENT.md)
