# ADR 0003: No dock

- **Date:** 2026-10-14
- **Status:** accepted

## Context

The halted shell decided against a running-application dock in its
[ADR 0003](../../../celestina-qt/docs/decisions/0003-no-running-app-dock.md).
The author confirmed the same choice for this shell in brainstorming. See
[the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §2.

## Decision

This shell has no dock of any kind. The launcher and the Island's Activity
page are the complete window-reach surface.

## Consequences

- No delivery opens a dock slice, parkable or fixed.
- A request for dock-like behaviour is a new discussion against the shell as
  it exists then.

## Revisit when

The author raises a new discussion; nothing is scheduled to revisit it.
