# ADR 0007: Focus policy

- **Date:** 2026-10-14
- **Status:** accepted

## Context

The author's session runs with `focus-follows-mouse`. A layer surface that
takes keyboard focus at rest would steal it from the author's windows when the
pointer crosses the Island. See [the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §5.6.

## Decision

Surfaces at rest take no keyboard (`KeyboardInteractivity::None`). Opening an
Island page sets `OnDemand`; the launcher sets `Exclusive` while it is open.
Every open surface closes on Escape, a click outside its silhouette, keyboard
focus loss and, for popups, `popup_done`. The compositor's key bindings open
surfaces through the shell's own bus interface (`org.celestina.Shell1`).

## Consequences

- The shell never depends on `focus-follows-mouse` being on or off.
- Every surface must be operable from the keyboard once it is open.

## Revisit when

A surface needs typing at rest, or niri changes how layer-surface keyboard
interactivity interacts with `focus-follows-mouse`.
