# ADR 0003: Do not build a running-app dock

- **Date:** 2026-08-14
- **Status:** accepted

## Context

SHELL-D4, formerly tracked in the discussion queue and retired by this
decision, asked whether the lived shell still needs a running-application dock
after Noctalia is removed.
The dock is visible product scope — window tracking, launch/focus semantics,
output behaviour — that should not be rebuilt merely because the old shell
had one, and R8 could not close its dock slice without an author decision.

## Decision

Celestina does not have a running-app dock. The launcher and the workspace
strip are the shell's complete window-reach surface; no dock slice is opened
under R8 or any later checkpoint.

## Consequences

- The R8 dock slice is closed with no implementation: R8 completes without
  it.
- Any future request for dock-like behaviour is a new discussion against the
  shell as it exists then, not a reopening of this one.

## Revisit when

Nothing is scheduled to revisit this. It stays decided unless the author
raises a new discussion.
