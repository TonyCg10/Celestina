# Grafita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Grafita constraints; it cannot relax the root or grant authority.

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

- `celestina-rs/crates/grafita-core` is the only owner of text classification,
  document state, positions, selection, edits, undo/redo, savepoint, conflict,
  safe IO, and open/edit/save/close session behavior.
- `src/` adapts that session to CXX-Qt and owns workers; `qml/` composes the
  standalone app. Siderita owns a separate adapter/composition; neither imports
  the other's QML or reimplements core rules.
- Extension and MIME assist discovery/highlighting but never decide whether a
  file is text. Canonical classification uses bytes and encoding.
- Grafita opens documents, not projects. Project trees, build runners,
  debuggers, LSP, terminals, and plugin platforms are out of scope.

## Documents, concurrency, and save

- Bytes, encoding, and line endings belong to the document. Never normalize
  silently or offer a non-reversible flow as editable.
- Probe, read, `stat`, and save never block the GUI thread. Open results carry a
  generation and save results a revision; stale responses neither replace nor
  clear current state.
- Preserve the core's zero-loss save contract: sibling temporary, reproducible
  metadata, synchronization, identity revalidation, and atomic rename. A
  refusal before rename preserves the original.
- Workers/callbacks respect ownership and Qt affinity, remain bounded, and join
  deterministically on close.

## Two surfaces

- In Siderita, Space on editable text opens embedded Grafita; double-click or
  Enter opens standalone Grafita. Never swap them.
- The embedded modal edits and saves but does not adopt app tabs/chrome. Dirty
  close offers Save, Discard, and Cancel; it blocks below, contains focus, and
  restores it on close.
- A visual recipe enters `celestina-style` only under the root sharing contract;
  never copy it between surfaces.

## Local verification

- `grafita/scripts/build-production.sh`
- `grafita/scripts/verify-production.sh`
- `grafita/scripts/status-production.sh`

Verification exercises the canonical release artifact without touching the
installed binary. Closing a bug or milestone runs `complete-production.sh` to
deploy those same bytes without recompilation. Perceptual, physical keyboard,
IME, AT-SPI, and compositor checks belong in `VALIDATION.md`. Review byte
preservation, ownership, thread affinity, Qt models, QML registration, and both
core consumers.
