# Siderita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Siderita constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [Content activation](../docs/contracts/content-activation.md)
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `src/` adapts domain to Qt/CXX-Qt and the desktop; `qml/` presents; `cpp/`
  covers only a named CXX-Qt limitation. Pure file domain belongs in
  `celestina-rs/crates/siderita-*`.
- `Main.qml`, `PickerWindow.qml`, and `qml/views/` coordinate. Extract a region
  with its own state/lifecycle behind typed properties and signals; it never
  reaches parent IDs.
- Added, moved, or removed QML stays in parity across the single `QML_FILES`
  list, QRC, and `rerun-if-changed`. Consume shared style through canonical
  relative links and explicit registration, never a copy.
- IO, processes, and D-Bus never block the Qt thread. Workers publish only
  current snapshots and cancel/join deterministically.

## Content activation

- Space on editable text opens embedded Grafita; double-click or Enter opens
  standalone Grafita. `grafita-core` decides by bytes and encoding, never file
  extension.
- Space on image/video/audio opens minimal Fluorita; double-click or Enter opens
  standalone Fluorita on that item. Navigation consumes static artwork and does
  not construct the engine.
- Directories retain navigation; unsupported types retain Quick Look or the
  desktop handler. Never swap the two actions.
- Embedded modals own local Qt/QML state, block the folder and lower shortcuts,
  contain/restore focus, and publish only core/engine-confirmed state.

## Local verification

- `siderita/scripts/build-production.sh`
- `siderita/scripts/verify-production.sh`
- `siderita/scripts/status-production.sh`
- `siderita/scripts/qml-tests.sh` when floating-surface event delivery changes

Isolated verification does not install the portal or alter `portals.conf`.
Closing a bug or milestone runs `complete-production.sh`, updating the author's
normal binary without recompilation. Wayland, blur, live portal routing,
physical keyboard, and AT-SPI belong in `VALIDATION.md`. Review ownership,
thread affinity, Qt models, QML registration, and both integrated consumers.
