# Selenita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Selenita's constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [The design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md),
  §1–§3 and §5
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `celestina-rs/crates/selenita-core` is the only owner of the domain:
  capture targets and geometry, output file names, the history, the niri IPC
  client, the argv of `grim`, `slurp` and `wl-copy`, and the recording
  pipeline and its state machine. It has no Qt.
- `src/` owns the Qt adaptation: the controller, the activation adapter, the
  appearance follower and, from SEL-1-A/B, the capture and recording workers.
  `qml/` presents and never decides what a target, a file or a history entry
  means.
- One main window, and beside it the corner preview of the latest result
  (SEL-2-A), a frameless window niri places by its title. Activation goes
  through `celestina_core::activation`: `Activate` raises, `Open` is
  ignored, and a file argument never triggers anything; `Adopt` on
  `org.celestina.Selenita1` hands back a file another application wrote.
  The appearance comes from `celestina-settings`. No private copy of
  either.
- No blocking IO on the Qt thread: child processes, the portal, files and the
  history run on workers and report back through cxx-qt `Threading` queues.
- `SELENITA_FAKE=1` routes every external tool (niri, `grim`, `slurp`,
  `wl-copy`, the portal, GStreamer) to fakes; tests and the smoke run with it.
  A real capture or recording on the session is never run by an agent.
- The Celestina shell is halted by the author's order: never read, reuse or
  reference `celestina/` or `celestina-shell-core`.

## Local verification

- `selenita/scripts/build-production.sh`
- `selenita/scripts/verify-production.sh`
- `selenita/scripts/status-production.sh`

Verification exercises the canonical release artifact without touching the
installed binary. Closing a bug or milestone runs
`selenita/scripts/complete-production.sh`. Real captures and recordings on
the session belong in `VALIDATION.md`.
