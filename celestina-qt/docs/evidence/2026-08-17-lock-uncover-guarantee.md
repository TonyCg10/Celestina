# The release cannot be held hostage by its own retreat, and the bundle ships

- **Date:** 2026-08-17
- **Scope:** Celestina units `LOCK-1-C` and the whole `LOCK-1` checkpoint
- **Artifact:** `LockUncover`, its wiring in `celestina/src/lock/main.cpp`, and
  celestina 0.30.0 as built, verified and deployed by
  `celestina/scripts/complete-production.sh`
- **Environment:** the nest closed and no Celestina process alive before the
  pipeline ran, so its smoke could start the real host without a second helper
  contending for the same DDC buses; the live session was not activated and
  Noctalia still owns it
- **Plan:** [lock depth transition](../plans/archive/2026-08-17-lock-depth-transition.md)
- **Validation:** `VAL-LOCK-1`

## Procedure

`celestina-lock-uncover-test` was written against `LockUncover` directly,
without a compositor: each case starts the sequence and breaks the retreat in a
different way. Afterward the plan's version was bumped to celestina 0.30.0 and
`scripts/complete-production.sh` was run exactly once, since its smoke starts
the real host and its provider adapter probes DDC on the live machine's i2c
buses.

## Result

### An authenticated verdict always releases, regardless of the retreat

Five cases, all passing:

- Nothing is uncovered before `begin()` is called, even after several times the
  ceiling — a refusal must be able to leave the session covered indefinitely.
- `retreat` fires immediately on `begin()`, and `uncover` only after the
  ceiling elapses: the session really does stay covered while the retreat has
  time to play.
- A retreat nobody is connected to still uncovers on schedule.
- A retreat handler that blocks past the ceiling still uncovers on schedule —
  the case a "release when the animation finishes" design gets wrong, since a
  stalled renderer would otherwise leave a correctly authenticated person
  outside their own session.
- Calling `begin()` three times emits `retreat` once and `uncover` once; the
  clock cannot be restarted or duplicated.

### The canonical production exit passes on the delivered bytes

`complete-production.sh` built celestina 0.30.0 once and verified those exact
bytes: Rust checks, QML lint, CTest 24/24 (including the five cases above and
every other suite test), and the eight-second offscreen release smoke of the
release host with the compiled style module. It then deployed the verified
bundle to `~/.local` and reported `current and verified` for all eleven
installed artifacts. The session was not activated.

## Limits

The retreat's on-screen appearance — whether the backdrop actually reaches true
scale and sharpness before the compositor uncovers, and whether that reveal is
free of a visible jump — was not observed here. Watching it needs an
authenticated verdict, which needs the author's real passphrase; no key was
injected into the nest to simulate one. That observation is `VAL-LOCK-1`, on a
real session.

This record makes no claim about `VAL-R6`, which remains unrun for its own
reason, and none about physical output behaviour, a second monitor, or
assistive technology — the nest used for `LOCK-1-A`/`LOCK-1-B` has one output,
and this record does not repeat those nest runs.
