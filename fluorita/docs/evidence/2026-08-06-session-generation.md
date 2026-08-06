# Evidence: 2026-08-06 a render handle belongs to one session

- **Date:** 2026-08-06
- **Scope:** `F6-E`; plan
  [immersive-content](../plans/active/2026-08-04-immersive-content.md); finding
  `H3` of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md)
- **Environment:** source correction with compilation, lint and unit tests. No
  production build, no deployment, no window opened on a live session
- **Artifact:** none; no production build ran

## What was wrong

The worker publishes its render handle asynchronously: `run_session` queues a
closure onto the Qt thread once the backend has one. Nothing said which session
that handle belonged to.

`F6-B` then added a shortcut to `close()` — when no handle has arrived, settle
the surface and return — precisely so a track, which needs no surface, could not
leave the player half-closed. That shortcut runs inside the publication window.
It joins the worker and destroys the mpv instance, and the closure queued a
moment earlier still lands afterwards and writes the address of an
`mpv_handle` that no longer exists.

What remains is worse than the stale address: a player holding a non-zero handle
with no worker. `decide_open` reads the handle, routes the next activation to
`CloseFirst`, and `close()` returns immediately because there is no worker —
so the parked activation is never replayed and never cleared. Every later
activation is a silent no-op for the rest of the process. Pressing Escape
straight after starting a video is enough to reach it.

## What changed

- `src/player.rs` — `PlayerRust` carries a `generation`, incremented for each
  session it starts and handed to `run_session`. The queued publication compares
  it before writing, so a handle from a session the player has already left is
  dropped. This is the rule `grafita-core` already applies to a completed save,
  and for the same reason: an answer that arrives late has to prove it is still
  the answer to the current question.
- `src/player.rs` — `close()` no longer returns silently when there is no
  worker. If an activation was parked while the previous session was closing, it
  starts it; the state that used to strand it cannot outlive the call.

## Procedure

```sh
cargo fmt --all --check                                  # in fluorita/
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

## Result

| Command | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | passes, no diagnostics |
| `cargo test --all-targets --locked` | 48 passed, 0 failed |

Two tests were added. One states the publication guard as the rule it is,
including that the counter wraps rather than overflowing and that a wrap is
still a different session. The other pins the routing that made the stranded
state permanent: an activation during a close parks, and a player still holding
a handle routes through the close that now has to honour what was parked.

## Limits

Both tests state rules over the decision functions; neither drives a real mpv
session through the window, which would need a running backend and a surface.
The failure this closes is a race, and a race is not something a unit test
reproduces — what it can do is fix the invariant that makes the race harmless,
which is what these assert. Whether the player survives an Escape pressed
immediately after activation belongs to `VAL-FLU-TEARDOWN` in
[`../../VALIDATION.md`](../../VALIDATION.md), which already walks that path.
