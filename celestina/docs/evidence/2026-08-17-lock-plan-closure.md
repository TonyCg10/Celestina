# LOCK-1 closes on its implementation exit, not on being looked at

- **Date:** 2026-08-17
- **Scope:** Celestina unit `LIVE-1-Z`
- **Artifact:** the archived
  [LOCK-1 plan](../plans/archive/2026-08-17-lock-depth-transition.md) and the
  roadmap and status entries that pointed at it
- **Environment:** repository only; nothing was built, run or deployed for this
  record
- **Plan:** [live session repairs](../plans/archive/2026-08-17-live-session-repairs.md)
- **Validation:** `VAL-LOCK-1`

## Procedure

`LOCK-1` delivered its three units and passed its implementation exit as
celestina 0.30.0. The guard permits one active plan per owner, and `LIVE-1`
now needs that slot, so the closed plan is moved to the archive and this
record says what it did and did not establish.

## Result

The locked session recedes behind its own wallpaper instead of vanishing into
an opaque slab: the backdrop enters overscanned and settles at true scale, a
blurred copy cross-fades in, and the prompt arrives on real Celestina glass
above it. Unlocking plays that backwards and only then calls
`unlock_and_destroy`, with the release guaranteed by a timer rather than by
the animation completing — five cases in `celestina-lock-uncover` stall,
ignore and repeat the retreat and require exactly one uncovering from each.

Two defects the work found in itself are worth keeping: nothing on a lock
surface may move before its first presented frame, or Qt's render thread can
commit before `ext-session-lock-v1`'s first configure is acknowledged and the
compositor kills the client; and a safety net must not be able to cancel the
design it protects, which is what happened when one flag answered both "may a
person type" and "is there a picture to set back".

## Limits

`VAL-LOCK-1` never ran. Whether the retreat actually reads as continuous on a
real output — whether the reveal is free of a visible jump — needs an
authenticated verdict, which needs the author's own passphrase. The lock has
not been exercised on the live session at all since this work landed.

Closing the plan releases the checkpoint slot; it does not claim the design is
finished, and any further work on the lock's appearance reopens under a new
checkpoint.
