# Hematita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Hematita constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [The design](../docs/superpowers/specs/2026-09-21-hematita-design.md)
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `celestina-rs/crates/hematita-core` is the only owner of `/proc` and `/sys`
  parsing, rate computation between samples and the history ring. It takes
  text and answers typed values or typed errors; it opens no file and holds
  no policy.
- `src/` owns the sampling thread, the snapshot it publishes, the load
  thresholds and every Qt object; `qml/` presents. QML never reads a file,
  spawns a process or decides what "high" means.
- The Celestina shell is in standby by the author's order: never read, reuse
  or reference `celestina/` or `celestina-shell-core`, however similar.
- Absence is a state: a source that cannot be read leaves its section of the
  snapshot unavailable with a reason while the others keep publishing. Every
  row carries its own state and its own typed reason, and the page composes
  the sentence.
- Per-process network throughput is out of scope for every phase (the kernel
  does not expose it without root or eBPF); per-process disk IO exists only
  for the user's own processes.
- A signal is asked for in `processes.rs` and nowhere else, by one of two
  paths: the user's own process is signalled directly through `rustix`;
  somebody else's is asked for by `privilege.rs`, which runs
  `pkexec /usr/bin/kill` with a fixed argument shape on a worker thread.
  Both cross the same gate first — the PID must be one the latest snapshot
  lists, and `/proc` must still show the start time and owner it showed
  then — and neither ever touches PID 1 or this process.
- Sensor values and limits come from hwmon files alone; the chip's own
  `crit` decides the thermal load through the thresholds in `publish.rs`;
  no alert, no fan control.
- Privilege follows [ADR
  0010](../docs/decisions/0010-one-shot-privilege-through-polkit.md):
  one-shot, polkit-mediated, `pkexec` spelled only in `privilege.rs`, every
  outcome typed; no agent is Hematita's to provide.

## Local verification

- `hematita/scripts/build-production.sh`
- `hematita/scripts/verify-production.sh`
- `hematita/scripts/status-production.sh`

Verification exercises the canonical release artifact without touching the
installed binary. Closing a bug or milestone runs
`hematita/scripts/complete-production.sh`. Graph smoothness, idle cost,
keyboard and assistive-technology checks belong in `VALIDATION.md`.
