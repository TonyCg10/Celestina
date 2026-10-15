# ADR 0009: One process, one loop, worker threads

- **Date:** 2026-10-14
- **Status:** accepted

## Context

The halted shell split its providers into separate processes joined by a line
protocol. In one Rust process the providers can publish typed messages
directly, and blocking D-Bus, DDC and IPC calls need not share the loop. See
[the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §5.1.

## Decision

The main thread runs `calloop` with the Wayland source, timers and one
channel of typed messages; everything that touches a surface, the painter or
the UI tree runs there. Each provider with blocking IO runs on its own named
worker thread behind the salvaged `Worker` guard (shared shutdown flag, join
on drop, a panic reported by name) and talks to the loop through bounded
channels. There is no async executor: zbus runs blocking on workers.

## Consequences

- Bursty sources are coalesced before they reach the loop.
- Shutdown joins every worker with a bounded wait and releases claimed names.

## Revisit when

A provider cannot be expressed as a bounded blocking worker, or the number of
workers costs more than an executor would.
