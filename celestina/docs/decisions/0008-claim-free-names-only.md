# ADR 0008: Claim well-known names only when free

- **Date:** 2026-10-14
- **Status:** accepted

## Context

Noctalia owns `org.freedesktop.Notifications` and
`org.kde.StatusNotifierWatcher` in the author's session and keeps running
while this shell grows. See [the design](../../../docs/superpowers/specs/2026-10-14-celestina-rust-shell-design.md), §2 and §5.6.

## Decision

The shell requests every well-known bus name with `DoNotQueue`, after its
objects are served. `NameTaken` disables that provider for the session and
records the owner for the page to show. There is no retry: the author stops
Noctalia and restarts the shell when ready.

## Consequences

- The Island and the launcher work beside Noctalia; the Notifications and Tray
  pages say who holds the name.
- The single-instance claim of `org.celestina.Shell` follows the same rule.

## Revisit when

Noctalia leaves the session for good, or a provider needs to take over a name
at run time.
