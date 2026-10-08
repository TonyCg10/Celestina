# Cuprita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Cuprita constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [The design](../docs/superpowers/specs/2026-10-08-cuprita-design.md)
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `celestina-rs/crates/cuprita-core` is the only owner of the domain: the
  models, the three backend traits (network, Bluetooth, audio), their fakes and
  the pure logic. It has no Qt and no D-Bus in the trait layer.
- `src/` owns the Qt adaptation: the controllers, the list models, the worker
  threads and the real clients. `qml/` presents. QML never calls a bus, spawns
  a process or decides what a state means.
- No blocking IO on the Qt thread: every D-Bus or PipeWire call runs on a
  worker thread and reports back through cxx-qt `Threading` queues.
- Privilege: Cuprita never runs as root and never keeps a secret. A Wi-Fi
  password is passed inside the activation call to NetworkManager and dropped;
  a Bluetooth code goes straight to BlueZ through the pairing agent; polkit
  prompts in its own window.
- The Celestina shell is halted by the author's order: never read, reuse or
  reference `celestina/` or `celestina-shell-core`, however similar. Cuprita has
  no tray, indicator or daemon.

## Local verification

- `cuprita/scripts/build-production.sh`
- `cuprita/scripts/verify-production.sh`
- `cuprita/scripts/status-production.sh`

Verification exercises the canonical release artifact without touching the
installed binary. Closing a bug or milestone runs
`cuprita/scripts/complete-production.sh`. Live Wi-Fi, Bluetooth and audio
checks belong in `VALIDATION.md`.
