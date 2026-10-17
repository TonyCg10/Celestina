# ADR 0011: Duties are switched on one by one

- **Date:** 2026-10-16
- **Status:** accepted

## Context

While Noctalia runs it keeps every duty the author has not switched off. A
unit that takes over a duty must be validated against the real session without
stopping Noctalia, and two owners of the same duty must never run at once. See
the design's §4.1.

## Decision

Each duty the shell may take over is a key of one table in the shell's own
`shell.json`:

```text
duties: bar | idle | caffeine | nightlight | wallpaper | osd | media |
        clipboard | polkit | lock | phone | ...
```

The value is `off` (the default while Noctalia runs) or `on`. A duty with an
observable owner (a bus name, a layer namespace) also reports its holder, and
the shell refuses to take a duty held by another process, as ADR 0008 does for
well-known names. A duty without an owner signal is only the author's switch.
The shell reads the file at start and follows its changes through its one
`shell.json` watcher (`follow_shell` in `src/providers/settings.rs`, which
every duty shares); a switch is a live change, never a restart. The
author turns the matching Noctalia duty off with Noctalia's own verbs. The last
unit, CEL-2-M, retires the `off` values of the shipped duties: after the
cutover a duty is simply on.

## Consequences

- Every unit ships off and is validated live by the author without a shell
  restart or a Noctalia stop.
- The settings file carries the duty table, so it must stay compatible: a
  missing key means `off`, and a file that predates a duty keeps working.
- Until CEL-2-M, the code paths of an off duty remain in the tree beside the
  on path, which is an accepted temporary cost.

## Revisit when

A duty is found that cannot be switched without restarting the shell, or the
author asks for a duty to be taken over without a switch at all (CEL-2-M).
