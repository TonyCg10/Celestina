# Evidence: CEL-2-B-GAMMA — the night light paced by the compositor, and back after a lost connection

- **Date:** 2026-11-23
- **Scope:** `celestina/src/energy/nightlight.rs` (the sweep, the worker's
  life over its connections and their tests), the procedure and pass
  condition of VAL-CEL2-NIGHTLIGHT in `celestina/VALIDATION.md`, and this
  unit's ledger row. No dependency, setting, verb or version changed;
  `wire.rs` and the pure `celestina_shell::nightlight` are untouched.
- **Environment:** CachyOS, worktree `celestina-CEL-2-B-GAMMA` cut from
  `main` at `3ffac082` (celestina 1.13.0); toolchains 1.97.1 (the project's
  `rust-toolchain.toml`) and 1.99 (the GitHub runner's stable). No shell was
  started, no Wayland connection was opened and no gamma was set: the
  author's session was in use, and every test below runs on a fake
  compositor in fake time.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

The author switched the night-light duty on in the live 1.13.0 shell (niri
on DRM, three outputs: DP-1 at 165 Hz, DP-2 at 120 Hz, HDMI-A-1 at 60 Hz).
The warm tint showed for an instant and went, and the journal said:

```text
20:37:47.685 INFO celestina::energy: night-light duty on
20:37:48.985 WARN celestina::energy::nightlight: night light: the compositor did not answer a sync within 1000 ms
20:37:49.060 WARN celestina::energy: night light: not applied: the compositor did not answer a sync within 1000 ms
```

Afterwards `celestina nightlight off` then `on` answered
`night light: not applied: unavailable` until the shell was restarted. It
reproduced after a clean restart. Two defects:

1. **The sweep outran the compositor.** `Lights::sweep` sent the 19 frames
   of the 300 ms transition to every output (57 `set_gamma` tables on three
   outputs), flushing each frame, and confirmed all of them with one display
   sync of 1 s at the end. A table is not cheap for niri. The author and
   then the reviewer read it in the compositor's source (NIRI-MELIBEA
   `e4062688`): the request handler (`src/protocols/gamma_control.rs`) reads
   the whole table and calls `Tty::set_gamma` (`src/backend/tty.rs`) inside
   the request, on niri's main thread, and that sets the CRTC's `GAMMA_LUT`
   property, which on an atomic driver is a blocking commit: it waits for
   the output's previous commit and then for its own flip, about one or two
   refresh periods of that output per table. Nothing is coalesced, and
   destroying a controller costs one more such commit. One client's requests
   are dispatched in order, so the sync is answered only after every table
   before it is applied. 57 of them queued behind niri's main loop: more
   than 1.3 s, about 23 ms a table on average, and the one sync was not
   answered in time (duty on at .685, 300 ms of frames, the timeout 1 s
   later).
2. **Any lost connection was final.** A `Lost` out of `serve`, a timeout
   included, made the worker fail dark and then answer `unavailable` for
   good; it never connected again.

The nested smoke could not see either: the nest's winit backend refuses
gamma control.

## The fix

- **Backpressure.** A sweep now sends one frame to every output, waits for
  the display sync that confirms it (`Lights::confirm`, deadline
  `SYNC_TIMEOUT`) and handles what the compositor reported before it sends
  the next. At most one table per output is in flight, so the compositor's
  pace bounds the worker's. The per-frame `memfd`s are released by each
  confirmed sync, as `WaylandTransport::sync` already did for the last one.
- **Skipping.** The pure `next_frame(offsets, current, elapsed)` picks the
  newest later frame that is already due, else the next one; it never goes
  backwards and never skips the last frame, the target. `frame_at` keeps a
  plan shorter than the schedule ending on its own last frame. A sweep is
  therefore bounded by its 300 ms plus the syncs of the frames it plays.
- **Nothing an output already shows is sent.** The first frame of a
  transition is where it starts; `Lights::plans` leaves it out, so a sweep
  costs the compositor 18 commits per output at most, not 19, and the
  remaining frames keep their instants.
- **Recovery.** `run` became `supervise`, generic over the connect step.
  A lost connection still fails dark (every controller destroyed, no
  sweep); when the cause may pass (`Lost::transient`: a sync not answered in
  time, flush, read, poll, dispatch, flood, a withdrawn manager) and the
  light is still wanted, the worker reports `NotApplied(reason)`, opens a
  new connection after `retry_backoff` of the losses in a row (1 s doubling
  to 30 s) and sweeps to the warmth last asked for. The losses are forgotten
  once a connection settles (`Lights::settled`: every output that takes a
  controller shows the target, or the light is off and nothing is held, so
  one output that keeps refusing does not hold the count up) and when the
  light is switched off. Commands are answered during the wait
  (`wait_to_connect`): off is `Off` and nothing connects until the light is
  asked for again; a new warmth becomes the next connection's target without
  shortening the wait; shutdown and a dropped handle end the wait within one
  poll. A connection lost while the light is off reports `Off`, not a light
  nobody asked for; the worker's own warning still logs the loss. No
  compositor (`Connect`) and no `wlr-gamma-control` (`NoManager`) stay final
  and idle as `unavailable`, as before.
- **One clock.** A `Clock` (`Monotonic` in the worker) gives the sweep its
  elapsed time, the pacing its sleep and the backoffs their instants. The
  state machine reads it where a thing happens instead of carrying the
  instant its poll began, so an output refused late in a long sweep waits
  its whole backoff. The `Transport` trait is unchanged.
- **A line for the live row.** Every finished sweep says at debug level
  `night light: sweep played N of M frames, slowest confirm K ms`, measured
  on the same clock; `sweep` returns those numbers (`Swept`) and the tests
  read them there.

Two choices the request left open, both recorded at `Lost::transient`:
a first sync that is not answered in time (a transient cause before any
connection worked) is tried again like any other, and a withdrawn manager is
tried once more, the next connection settling it as `NoManager` if it is
still absent.

## Tests

All in `energy::nightlight::tests`, on the `Fake` transport and a fake
clock (`Ticker`); none opens a display or a bus. The fake compositor's sync
costs a fixed time plus a time per table sent since the last one, and is a
`Lost::Timeout` when that passes the deadline.

The pure `next_frame`:

- `a_compositor_that_keeps_the_pace_is_given_every_frame`
- `a_slower_compositor_skips_to_the_newest_frame_already_due`
- `past_the_whole_transition_the_next_frame_is_the_target`
- `the_last_frame_has_no_next_and_a_schedule_of_one_ends_at_once`
- `every_next_frame_is_later_due_or_adjacent_and_every_pace_ends_on_the_target`
  (every frame against every millisecond to 400 ms, and every steady pace to
  1200 ms a frame)

The sweep:

- `a_compositor_that_blocks_on_every_table_never_has_more_than_one_per_output_to_apply`
  (the direct regression test of the first defect: 25 ms a table on three
  outputs, where one sync behind the whole sweep would cost 1350 ms; the
  sweep never has more than three tables in flight, its slowest confirm is
  75 ms, it plays 5 of 18 frames and ends `Applied` on the target)
- `a_sweep_the_compositor_confirms_in_time_plays_every_frame_but_the_one_already_shown`
  (2 ms a sync: 18 tables per output, the first not neutral)
- `a_sweep_the_compositor_paces_slower_skips_frames_and_ends_on_the_target`
  (40 ms a sync: frames 1, 3, 5, 8, 10, 13, 15, 17, 18 on every output)
- `a_compositor_slower_than_the_whole_transition_gets_one_frame_and_the_target`
- `a_plan_shorter_than_the_schedule_still_ends_on_its_last_frame`
- `a_refusal_late_in_a_long_sweep_waits_its_whole_backoff`
- `a_connection_is_settled_when_only_refusing_outputs_are_short_of_the_target`
- `an_output_unplugged_mid_sweep_gets_no_further_frame_and_the_others_reach_the_target`
- `a_controller_refused_mid_sweep_is_destroyed_not_left_half_way` (existing,
  with one assertion added: the refused output gets no table after the
  refusal)

The worker over its connections:

- `a_sync_lost_mid_sweep_fails_dark_and_the_light_comes_back_by_itself` (the
  regression test of the second defect: `NotApplied`, every controller
  destroyed half way, then `Applied` one second later on a new connection at
  the same warmth, with no new command)
- `two_losses_in_a_row_wait_one_second_then_two_and_an_applied_light_forgets_them`
- `an_output_that_keeps_refusing_does_not_make_later_losses_wait_longer`
- `switching_off_forgets_the_lost_connections`
- `a_connection_lost_while_the_light_is_off_reports_off_and_connects_when_asked`
- `off_during_the_wait_is_off_and_nothing_is_connected`
- `a_new_warmth_during_the_wait_is_what_the_next_connection_sweeps_to`
- `on_again_after_off_connects_at_once_when_the_backoff_is_long_over`
- `off_asked_during_a_sweep_that_loses_the_connection_is_answered_and_nothing_connects`
- `shutdown_during_the_wait_ends_it_within_one_poll`
- `a_dropped_handle_during_the_wait_ends_it_within_one_poll`
- `a_first_sync_not_answered_in_time_is_tried_again`
- `a_withdrawn_manager_is_settled_by_the_next_connection`
- `only_no_compositor_and_no_gamma_manager_are_final`
- `no_compositor_or_no_gamma_manager_idles_as_unavailable_and_never_connects_again`

The 18 tests the module had all pass. None encoded the old "idles for good"
rule, so none was rewritten. What changed in them: the two that call `serve`
pass it the new `Standing` argument; the instant arguments of `reconcile`
and `handle` are gone, and the test helper moves the fake clock to the
instant it is given instead; and four of them count one table fewer per
sweep and output (18 for 19, 36 for 38, 19 for 20), because the frame an
output already shows is no longer sent, two with an assertion added that
says so.

## Procedure

```sh
# in celestina/
cargo test --lib energy::nightlight
cargo test
cargo clippy --all-targets -- -D warnings          # 1.97.1, the pinned one
cargo +1.99 clippy --all-targets -- -D warnings
cargo fmt --all -- --check
# from the root
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

Nine mutations of the fixed file were then tried one at a time, each
restored from a saved copy before the next: the old sweep (every frame
flushed, one sync behind them all); every loss final (the old rule); the
elapsed time ignored (no skipping); the frame already shown sent again; the
losses forgotten only by `Applied`; the losses kept across off; a loss with
the light off reported as not applied; a refusal stamped with the instant
its reconciliation began; and a command taken off the queue during a sweep
dropped with the connection.

## Result

| Command | Exit | Output |
|---|---|---|
| `cargo test --lib energy::nightlight` | 0 | 46 passed, 0 failed (18 before this unit) |
| `cargo test` | 0 | the library's 1505 tests and every integration test passed, 0 failed, 1 ignored (an `#[ignore]` of `tests/wallpaper_decode.rs`, which this unit does not touch) |
| `cargo clippy --all-targets -- -D warnings` (1.97.1) | 0 | clean |
| `cargo +1.99 clippy --all-targets -- -D warnings` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `bash scripts/check-architecture-contract.sh` | 0 | `Architecture contract: OK` |
| `python3 scripts/check-language-contract.py` | 0 | `Language contract: OK (128 legacy file(s) ratcheted)` |
| `bash scripts/check-documentation-contract.sh` | 0 | `Documentation contract: OK` (it also prints the known `SID-G7-D` erratum lines) |

Each mutation failed the tests written for it and nothing hung. The old
sweep failed 9 tests, the direct one with `Lost(Wire(Timeout))`, which is
the failure of the live run; every loss final failed 14, the second
defect's regression test among them; no skipping 5; the frame already shown
13; the losses forgotten only by `Applied` 1; the losses kept across off 1;
a loss while off as not applied 5; the stale refusal instant 1; the command
dropped 1. The file was restored byte for byte before the runs above.

## Review

An independent review (Opus, a model other than the implementer's, read
only) found nothing critical and confirmed the cause from niri's source as
recorded above. Its one finding about the landing: this is a product
defect's correction and lands as `bug`; the session commit says
`maintenance` only because the session hook refuses `bug`. Fixed in this
unit after it: the frame an output already shows is not sent (M-1); a
refusal's backoff counts from the instant it happened (M-2); the losses are
forgotten when a connection settles or the light goes off, not only on
`Applied` (M-3); a loss with the light off reports `Off` (M-4); this
record's cause and limits (M-6); the validation row's untestable clause and
the sweep line for it (M-7); and the fake's cost per table with the direct
regression test (M-8). Recorded below, not changed: the shutdown bound
(M-5). Left as it is: a sweep whose outputs have all gone mid-way keeps
confirming its remaining frames with nothing sent, which is bounded and
cheap (M-9).

## Limits

- Nothing ran on a display. The real cost on the author's outputs (how long
  niri takes to apply one frame's three tables, and so how many of the 18
  frames a live sweep shows) is unverified until VAL-CEL2-NIGHTLIGHT, which
  now asks for the sweep line; the tests prove the worker's behaviour for a
  given compositor pace, not that pace.
- That a table is a blocking commit of one or two refresh periods is read
  from niri's source; the kernel's and drm-rs's side of it (a property set
  on an atomic driver blocks until its flip) is upstream knowledge that
  neither the unit nor the review checked locally.
- The nested smoke cannot exercise gamma (the nest's winit backend refuses
  it), so it was not run and would not have covered this change.
- A compositor that cannot confirm even one frame within 1 s keeps failing,
  and each attempt costs it more: a stall of over a second on its main
  thread, with every client waiting, tried again every 30 s at most until
  the light is switched off. Nothing warm shows in that case, since no frame
  was confirmed. The review found no steady state of niri that gets there;
  the transient ones (a modeset at resume, hotplug or a mode change) cost one
  fail-dark of every output and one new sweep a second later.
- Shutdown. On the author's outputs the restoring sweep is a few frames of
  30 to 100 ms, three controllers destroyed and one sync: about 0.5 s, under
  the 2 s the shell waits for a worker (`JOIN_WAIT`, `energy/mod.rs`). With
  a compositor that answers every sync just under 1 s the worst case is
  about 4 s (the confirm in flight, two frames of the restoring sweep, the
  release sync), against about 3.3 s before this unit, and so beyond that
  wait: the worker is then left behind and the process exit closes its
  socket, on which the compositor restores every output by protocol. One
  sync still waits up to 1 s and is not interrupted by shutdown, as before.
- `cargo test --workspace` and the production scripts were not run; the
  landing's `verify-production.sh` runs them.
- Nothing was stashed, pushed or landed.

## Landing

- **Base revision:** `3b36e8f84d362b76f06fc1a439061e9936284e49`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:cc33de3b21a5ad8bd9ea0e04e191c04fc59d1df7a430ee931aacade0e0bd279c, verification_fingerprint sha256:ecee1404efbeee86461a6e0bb598e2a25f0631a893343f1b9f44df268b0131d4
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
