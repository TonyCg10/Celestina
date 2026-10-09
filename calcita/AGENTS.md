# Calcita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Calcita's constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [The design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md),
  §1–§4
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `celestina-rs/crates/calcita-core` is the only owner of the domain: recent
  documents, the zoom ladder, reading positions and the page field's grammar.
  It has no Qt.
- `src/` owns the Qt adaptation: the controller, the activation adapter and
  the appearance follower. `qml/` presents. QtPdf renders from QML; the
  controller never touches page rendering, and QML never decides what a path
  or a recent entry means.
- One document per window. Activation goes through
  `celestina_core::activation` (`Open` with paths byte-exact); the appearance
  comes from `celestina-settings`. No private copy of either.
- No blocking IO on the Qt thread: file and store work runs on a worker and
  reports back through cxx-qt `Threading` queues.
- Calcita reads documents; it never writes to them and needs no privilege.
- The Celestina shell is halted by the author's order: never read, reuse or
  reference `celestina/` or `celestina-shell-core`.

## Local verification

- `calcita/scripts/build-production.sh`
- `calcita/scripts/verify-production.sh`
- `calcita/scripts/status-production.sh`

Verification exercises the canonical release artifact without touching the
installed binary. Closing a bug or milestone runs
`calcita/scripts/complete-production.sh`. Reading real documents on the
session belongs in `VALIDATION.md`.
