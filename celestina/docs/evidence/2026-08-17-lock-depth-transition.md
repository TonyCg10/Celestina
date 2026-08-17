# The locked session recedes behind its own wallpaper

- **Date:** 2026-08-17
- **Scope:** Celestina unit `LOCK-1-B`
- **Artifact:** `celestina/src/lock/LockScreen.qml`
- **Environment:** the nest on DP-2 at scale 1, one lock client asserted alive
  before every measurement; the live session was not touched
- **Plan:** [lock depth transition](../plans/active/2026-08-17-lock-depth-transition.md)
- **Validation:** `VAL-LOCK-1`

## Procedure

Each of the three backdrop cases a cover can be in — a decodable wallpaper, no
wallpaper published, and a path that will not decode — was run against a
freshly restarted nest, with exactly one lock client confirmed alive before the
screenshot. Two clients on one compositor produce `refusing lock as already
locked` plus a protocol error from the loser, which is indistinguishable from a
product defect until the compositor log is read; several early measurements
were discarded once this was understood.

## Result

### The three cases behave as the plan requires, with no compositor complaint

| Case | Screen | Compositor complaints |
|---|---|---|
| Wallpaper present | Backdrop covers edge to edge, heavily defocused, clock and prompt on translucent glass above it | none |
| No wallpaper published | The deliberate canvas, clock and prompt on the opaque readable fallback | none |
| Path that will not decode | Identical to no wallpaper | none |

The undecodable case is byte-identical in size to the absent case, which is the
point: an unreadable file is the same as no file, and neither produces a dark
rectangle a person could mistake for a photograph. An earlier composition
scaled the backdrop *down* to suggest depth, which exposed the canvas at the
output's edges as a visible black margin; the surface now enters slightly
overscanned and settles at true scale, so the same gesture of something moving
away never uncovers a single pixel of canvas.

### Two defects this unit found in itself

Both were found by running the thing rather than by reading it.

**Nothing on this surface may move before it has presented a frame.**
`ext-session-lock-v1` makes committing a buffer before acknowledging the first
configure a protocol error, and the compositor answers it by killing the client
— leaving the session locked, blank and unusable. The acknowledgement is
collected synchronously while the surface is built, but Qt Quick renders on its
own thread, so merely starting an animation in `Component.onCompleted` was
enough for that thread to commit first. It is a race, and it presented as an
intermittently black lock. Every movement is now gated on `onFrameSwapped`: a
presented frame cannot lie about the acknowledgement having happened.

**A safety net must not be able to cancel the design.** The guard that
guarantees a prompt appears, and the recession of the backdrop, were one
idempotent function. A 4K photograph took longer to decode than the guard took
to fire, so the guard settled the surface and the picture arrived blurred but
never set back. They are now two states — `overlayShown` and `receded` — that
cannot consume one another.

## Limits

This record covers what a cover looks like once locked, on one nested output at
one scale. It does not cover the retreat that plays before unlocking — that is
[the uncover-guarantee evidence](2026-08-17-lock-uncover-guarantee.md) — and it
makes no claim about a real session, a second monitor, or assistive technology.

The author's live session was interrupted once during this work by a `pkill`
that matched the nest and the real compositor alike, since both run the same
`niri` binary. Nothing was lost from the repository and nothing about the
delivered bundle depends on it, but the nest must only ever be killed by pid.
