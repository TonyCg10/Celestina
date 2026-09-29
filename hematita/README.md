# Hematita

Celestina's resource monitor: what the machine is doing, and the means to act
on it. It replaces Mission Center.

## User contract

- Performance: CPU, memory, every whole disk, every Ethernet or Wi-Fi
  interface and the AMD GPU, each with sixty seconds of history.
- Processes and applications (H3), sensors (H4) and systemd services (H5)
  follow the [design](../docs/superpowers/specs/2026-09-21-hematita-design.md).
- Hematita reads `/proc` and `/sys` directly, every file bounded; it needs
  no daemon and no privilege to observe. Acting on another user's process or
  on a system unit goes through polkit, one call at a time (ADR 0010).
- Version 1.0 replaces Mission Center for the author: performance, processes,
  applications, sensors and services.
- Almacenamiento: mount points, browsing, a scan where you are, size list and
  treemap, duplicates verified by content, empty folders, and actions through
  Siderita's trash or a confirmed permanent deletion.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/hematita-core` | Parsers over `/proc` and `/sys` text, rate samplers, the history ring, and the storage analyser's domain; no Qt, and file IO only in `usage` (the walk, the duplicate check, the guarded deletion) |
| `src/` | The sampling thread and the bounded kernel-file reader, its immutable snapshot, load thresholds, CXX-Qt objects, single-instance activation |
| `qml/` | The window, the pill navigation strip and the pages |
| `../celestina-style` | Canonical visual tokens, controls and assets, linked |
| `org.celestina.Hematita.desktop` | Desktop discovery |

## Build and use

Hematita needs Rust and a Qt 6 development environment visible to CXX-Qt. The
canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

After completion, launch `hematita` or use the desktop entry.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
