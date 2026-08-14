# The lock the compositor keeps, and the four defects on the way to it

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R6-B`
- **Artifact:** Celestina 0.22.0, `celestina-lock` and its shell integration
- **Environment:** nested niri 26.04 (`dev-session.sh`), one `winit` output at
  3840x2160 scale 1.5; Qt 6.11.1 with its private Wayland client API
- **Plan:** [first-party session lock](../plans/active/2026-08-14-first-party-session-lock.md)
- **Validation:** `VAL-R6`

The claim this unit had to earn is that the session stays locked when the lock
program does not. That is the whole reason ADR 0004 chose `ext-session-lock-v1`
over a layer surface with a keyboard grab, and it is the one thing here that
was proven by breaking it rather than by reading a trace.

## Procedure

`celestina-lock` was run against the nested session and its Wayland dialogue
captured with `WAYLAND_DEBUG`. The lock was then killed with `SIGKILL` while
the session was locked, and the screen photographed with `grim` afterwards.

## Result

### The protocol dialogue is the one the specification describes

    -> ext_session_lock_v1#33.get_lock_surface(new id #34, wl_surface#3, wl_output#21)
    -> ext_session_lock_surface_v1#34.ack_configure(13)
    <- ext_session_lock_v1#33.locked()

No protocol error, and `locked` on stdout for a caller to sequence behind. One
surface per output, including outputs that arrive while locked; three covers
on three screens, one on one.

### Killing the lock does not unlock the session

With the session locked, the process was killed outright. The compositor kept
the session locked and painted its own solid colour over the output — the
fallback the protocol requires when a lock surface dies before
`unlock_and_destroy`. The desktop was never exposed. That is the guarantee
this design exists to borrow, observed rather than assumed.

### Four defects, each found by measurement

- **The project declared `LANGUAGES CXX`**, so CMake silently ignored the
  generated `-protocol.c` and the interface symbols never linked. Silent
  because a `.c` file in a CXX-only project is not an error, just absent.
- **`QWaylandShellIntegration::initialize` is pure virtual** in 6.11; chaining
  to a base implementation does not link.
- **The commit that Qt makes as soon as a shell surface exists** violates the
  protocol: "Committing the surface before acking the first configure is a
  protocol error". The configure is now collected inside the surface's own
  constructor, on a private `wl_event_queue` so only this proxy's events are
  dispatched. Qt's own round trip was tried first and never returned — it
  re-enters Qt's machinery from inside window creation.
- **The `locked` event arrives during that constructor**, before the program
  has connected anything to hear it. An already-confirmed lock is announced
  directly rather than waited for.

## Limits

No passphrase, right or wrong, was entered here: `R6-A` owns the verification
boundary and `VAL-R6` owns the real attempt. What this unit proves is that
something covers every output and that only an authenticated verdict can
reach `unlock_and_destroy` — not that the author's own password opens their
own machine.

**The lock will not start under EGL on this nested session.** Mesa's EGL hangs
in a round trip of its own during `QQuickWindow` construction, before any of
this unit's code runs; the runs above use `QT_QUICK_BACKEND=software`. It was
isolated to the graphics stack by loading the integration with an empty
`initialize`, which hung identically, so it is neither the binding nor the
lock. Whether it reproduces on the author's real session, where the shell's
own EGL surfaces work, is unknown and is the first thing `VAL-R6` must
answer — a lock that cannot start is a lock that is not there.

Every test here also leaves the nest locked with no client, which is correct
and makes the *next* run hang in that same EGL round trip. A meaningful run is
the first one against a freshly started nest.
