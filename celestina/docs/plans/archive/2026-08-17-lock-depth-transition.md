# Lock depth transition — the session recedes instead of vanishing

- **Opened:** 2026-08-17
- **Closed:** 2026-08-17
- **Plan ID:** lock-depth-transition
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** LOCK-1
- **Author-validation checkpoint:** VAL-LOCK-1
- **Successor:** [LIVE-1 live session repairs](2026-08-17-live-session-repairs.md)

## Hypothesis

The lock is correct and unpleasant. It covers every output, delegates every
verification and refuses to unlock on any error, and it does all of that on a
flat opaque slab that shares nothing with the session it just covered. Locking
therefore reads as the session being destroyed rather than set aside.

The claim this checkpoint tests is that the lock does not need to show the
session to feel continuous with it. It needs to show the one thing the session
was already showing that is not anyone's content — the wallpaper — pushed back
and blurred, with the prompt floating on the shell's own glass above it. If
that is right, the lock stops being a different place and becomes the same
place, out of focus.

What makes the return in particular worth building: the backdrop can be
animated back to exactly the wallpaper's real geometry and sharpness before
`unlock_and_destroy` is called, so the compositor reveals the session on a
frame the backdrop already matches. The uncovering becomes continuous rather
than a cut.

## Tangible outcome

A locked output shows its own wallpaper, pushed back and heavily blurred, with
the clock and prompt fading in above it on real Celestina glass. Unlocking
fades the prompt out, returns the backdrop to its true scale and sharpness, and
only then uncovers the session. An output with no wallpaper, an undecodable
file or a shell that never sent one shows exactly the canvas it shows today.

## Scope

Three units, in dependency order.

- **LOCK-1-A — the lock learns its wallpaper.** `LockController` sends the
  running per-output wallpaper choice to `celestina-lock`, which paints it per
  cover with the same honesty `Wallpaper.qml` already applies: a file that will
  not decode falls back to the deliberate canvas rather than to a dark
  rectangle a person could mistake for a photograph.
- **LOCK-1-B — the depth transition.** The backdrop enters at true scale and
  sharpness and recedes to a reduced scale and an intense blur; the clock and
  prompt fade in above it. The prompt card stops declaring a backdrop it does
  not have and becomes a real `InSceneCapture` `GlassSurface` sampling the
  backdrop beneath it.
- **LOCK-1-C — the retreat precedes the uncovering.** An authenticated verdict
  fades the prompt out and returns the backdrop to true scale and sharpness
  before `unlock_and_destroy`, and a timer — never the animation — guarantees
  the release.

  The sequencing lives in `LockUncover` rather than in `main`, because the
  guarantee is the part worth testing and a lambda inside a `main` that needs a
  compositor cannot be. It emits `retreat` at once, emits `uncover` on a timer
  that answers to nothing else, and does both exactly once per process. No cover
  is ever asked whether it finished: a retreat nobody handles, a handler that
  hangs and a verdict delivered three times all still uncover the session once.

### Measured facts this plan is built on

Established on 2026-08-17 against the running nest and the deployed bundle,
rather than assumed:

- The nested Niri **does** implement `ext-session-lock-v1`. The deployed
  `celestina-lock` run against the nest printed its `locked` line and covered
  the nested output while the nested shell, Niri adapter and provider adapter
  all stayed alive. This closes the open question in `STATUS.md` about whether
  a nest running a shell and a lock together hits the one-EGL-client limit: it
  does not. `VAL-R6` remains unclaimed for its own reason — nobody has unlocked
  a real machine with it.
- Killing that lock left the nest covered until the nest itself was restarted,
  which is the protocol guarantee working, and is the reason the iteration loop
  for this checkpoint restarts the nest rather than dismissing a lock.
- Niri 26.04 publishes no session-lock animation. The compositor will not
  scale or blur the session on our behalf, so every frame of this transition
  belongs to the lock client.
- The client-side blur this needs already exists and is canonical:
  `GlassSurface` composes `ShaderEffectSource` with `MultiEffect` under the
  `glassBlur*`, `glassSaturation` and `glassSampleScale` tokens. This
  checkpoint reuses that material and does not introduce a second blur
  vocabulary.

### What building it taught, which the plan did not anticipate

- **Nothing on this surface may move before it has presented a frame.**
  `ext-session-lock-v1` makes committing a buffer before acknowledging the first
  configure a protocol error, and the compositor answers it by killing the
  client — leaving the session locked, blank and unusable. The acknowledgement
  is collected synchronously while the surface is built, but Qt Quick renders on
  its own thread, so merely starting an animation in `Component.onCompleted` was
  enough for that thread to commit first. It is a race, and it presented as an
  intermittently black lock. The cover now gates every movement on
  `onFrameSwapped`: a presented frame is the one signal that cannot lie about
  the acknowledgement having happened.
- **A safety net must not be able to cancel the design.** The guard that
  guarantees a prompt appears, and the recession of the backdrop, were one
  idempotent `settle()`. A 4K photograph took longer to decode than the guard
  took to fire, so the guard settled the surface and the picture arrived
  blurred but never set back. They are now two states: `overlayShown` answers
  "can this person type", `receded` answers "is there a picture to push away",
  and neither consumes the other.

### Verification loop for this checkpoint

The nest is the iteration harness, and two of its hazards are recorded because
both produced false results before they were understood.

- Kill the nest **by pid only**, resolved through `celestina-dev-session.kdl`.
  The nest and the author's live session run the same `niri` binary, so
  `pkill -x niri` matches both — it did, and it took the author's real session
  down mid-checkpoint.
- `pkill -f <pattern>` also matches the invoking shell's own command line when
  the pattern appears there, killing the cleanup command itself and leaving
  stale lock clients alive. Two lock clients on one compositor produce
  `refusing lock as already locked` plus a protocol error from the loser, which
  is indistinguishable from a product defect until the compositor log is read.
  Measure only after asserting a single client.

## Exclusions

- **No capture of the session.** `ext-session-lock-v1` stops the compositor
  showing the session, and Wayland gives no client another client's buffers.
  The receding backdrop is the wallpaper and never the panel, a window or a
  composited picture of the desktop. This is the same limit already recorded
  against window previews in `WMAP-1`.
- **No new unlock path.** `unlock_and_destroy` remains reachable only from an
  `Authenticated` verdict. The transition delays that call; it can never
  originate it, skip it, or be the reason it happens.
- **Nothing new on the locked surface.** Time, prompt and failure state remain
  the whole of it. A wallpaper is the session's own look, visible to anyone
  standing in front of the screen a moment earlier; it is not content, and no
  notification, media, clipboard or window information joins it.
- **No second channel into the lock.** The lock gains exactly one inbound fact
  — which image belongs to its output — and no provider connection, D-Bus name
  or settings reader. It still chooses no wallpaper of its own.
- **No PAM, verification or inhibitor change.** `LOCK-1-C` moves when
  `release()` is called and nothing about what authorizes it.
- **No compositor, Niri or wallpaper-selection work.** The shell decides which
  file belongs to which output exactly as it does today.

## Build order

`LOCK-1-A` first: until a cover can paint a wallpaper at all there is nothing
for a transition to move, and the fallback path has to be proven before it is
animated. `LOCK-1-B` then animates it and re-materializes the prompt card.
`LOCK-1-C` last, because the retreat is the same animation played backwards
and should not be written twice.

The hand-off in `LOCK-1-A` is the unit's real risk and is bounded first. The
wallpaper travels over the lock's stdin as one bounded line, not over `argv`,
because `/proc/<pid>/cmdline` is world-readable and a filename can be personal.
It is read without blocking: a lock that waited for the shell to describe its
backdrop would have made a decoration into a precondition for covering the
screen. If the line never arrives, arrives late or does not parse, the cover is
already up and simply keeps the canvas.

## Implementation exit

- `LOCK-1-A`: a cover paints the file it was given, and a cover given nothing,
  a missing path or an undecodable file paints the canvas. The confirmation
  line is not delayed by the hand-off — proved by a `LockController` test that
  withholds the wallpaper entirely and still observes `locked`.
- `LOCK-1-B`: the backdrop reaches its receded scale and blurred state, the
  prompt is legible against it at both nest scales, and `reducedMotion` drops
  the travel without dropping legibility.
- `LOCK-1-C`: an `Authenticated` verdict always releases. A test holds the
  animation open and observes `release()` fire on the timer regardless, and a
  non-authenticated verdict releases nothing.
- Whole-checkpoint: `scripts/complete-production.sh` builds once, runs the Rust
  checks, QML lint, CTest and the offscreen release smoke against those bytes,
  and deploys without activating the session.
- Perceptual confirmation on a real output is `VAL-LOCK-1` and does not keep
  this checkpoint open.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LOCK-1-A | `celestina:` | done | [inventory](../../inventories/2026-08-17-lock-depth-transition/LOCK-1-A.numstat.tsv) | Hand the lock its per-output wallpaper over a bounded, non-blocking stdin line, falling back to the canvas on anything unreadable; owns `lockcontroller.{h,cpp}`, `lockbackdrop.{h,cpp}` and the shell-side wiring | 9 files, +619/-0 | [evidence](../../evidence/2026-08-17-lock-backdrop-handoff.md) | `VAL-LOCK-1` |
| LOCK-1-B | `celestina:` | done | [inventory](../../inventories/2026-08-17-lock-depth-transition/LOCK-1-B.numstat.tsv) | Recede and blur the backdrop, arrive the overlay above it, and give the prompt card a real in-scene glass backdrop; owns the whole of `LockScreen.qml` | 4 files, +712/-13 | [evidence](../../evidence/2026-08-17-lock-depth-transition.md) | `VAL-LOCK-1` |
| LOCK-1-C | `celestina:` | done | [inventory](../../inventories/2026-08-17-lock-depth-transition/LOCK-1-C.numstat.tsv) | Sequence the retreat before `unlock_and_destroy` with the release guaranteed by a timer, own the process wiring in `lock/main.cpp`, the build and version bump, and the closing production exit | 15 files, +681/-12 | [evidence](../../evidence/2026-08-17-lock-uncover-guarantee.md) | `VAL-LOCK-1` |
