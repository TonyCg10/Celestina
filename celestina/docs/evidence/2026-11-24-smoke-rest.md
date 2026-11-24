# Evidence: SMOKE-REST — the smoke's rest-frame budgets for the wallpaper run and the cutover run

- **Date:** 2026-11-24
- **Scope:** `celestina/scripts/smoke.sh` (the wallpaper run's budget, the
  cutover run's two windows and its nine-second rest, what a rest-frame
  failure prints), `celestina/crates/celestina-wayland/src/metrics.rs` (one
  journal line per rest frame) and this unit's ledger row. No behaviour of
  the product changed: the new line is written only with `--metrics`.
- **Environment:** CachyOS, worktree `celestina-SMOKE-REST` cut from `main`
  at `44b8cc2a` (celestina 1.14.0) and rebased onto `c83807be` (1.15.0,
  after CEL-2-M-LOCK-POLKIT) before the checks and the full smokes of the
  result below; the measurements of the two runs alone were taken before the
  rebase. Nested niri through `scripts/smoke.sh`
  and through copies of its wallpaper and cutover blocks kept outside the
  repository, which leave the journals in place (own bus, a system bus
  address that is not there, fakes first on `PATH`, throwaway configuration
  and state homes). The author was told before the first nest. **Nothing ran
  in the author's session** but the nests' own windows; during the resize
  measurements the host's niri was asked to change the width of the nest's
  own window, found by the nested niri's command line, and of no other.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

1. **The wallpaper run failed once at a landing** (2026-11-23, 21:25 UTC):
   `frames were drawn at rest with the wallpaper present (4, at most 2)`, and
   it passed when the landing was continued. Its metrics were

   ```text
   winit frames=2017 rest_frames=0 n=2014 ...     the panel
   winit frames=5 rest_frames=4 n=0 ...           the wallpaper
   ```

   so all four were on the wallpaper's own surface. The journal was deleted
   with the run and its printed tail held only the panel's frames.
2. **The cutover run's budget was thirteen** over its eighteen seconds, the
   sum of every earlier run's allowance, against a measured total of 0 to 2
   (`2026-11-23-cutover.md`): a shell that redrew every two seconds would
   draw nine and pass.

## Procedure

Each measurement used the release binary of the tree named with it, built as
`build-production.sh` builds it (`cargo build --release --locked --bin
celestina`, then `-p celestina-lock -p celestina-lock-verify`) and copied out
of the shared target directory at once.

```sh
# the wallpaper run alone, journal kept (binary of main, 44b8cc2a)
#   4 runs on a quiet machine, 6 beside twelve busy loops, 1 with one resize
# the cutover run alone, journal kept (binary of this tree)
#   9 runs with the script of main (five-second rest), 1 more with five resizes
#   5 runs with this tree's script (nine-second rest), 1 with four resizes
#   1 run with the after-switch budget set to 0, to see the check fail
niri msg action set-window-width --id <the nest's own window> -120    # a resize
cargo test --workspace --locked                                        # in celestina/
cargo clippy --workspace --all-targets --locked -- -D warnings          # 1.97.1, pinned
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
for f in scripts/*.sh; do sh -n "$f"; done
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
timeout -s KILL 1500 sh scripts/smoke.sh --binary <release of this tree>   # three times
```

## The wallpaper run: where the four frames came from

The run was repeated with the journal kept and the wallpaper's debug lines
on, on the release binary of `main`:

| Runs | Condition | Wallpaper surface |
|---|---|---|
| 4 | quiet machine | `frames=1 rest_frames=0` each |
| 6 | twelve busy loops on eight cores | `frames=1 rest_frames=0` each |
| 1 | the nest's window resized once by the host, four seconds in | `frames=3 rest_frames=2` |

A loaded machine alone adds nothing: the surface is created when the first
picture arrives, so that picture is its first frame, and the Island's frames
have told the real scale before then. One resize of the nest's window costs
exactly two frames, and the journal says what each is:

```text
INFO  handlers: output changed output=winit size=(548, 672) scale=2.0 refresh_mhz=60000
DEBUG handlers: configure surface=2 size=(548, 672) serial=5
DEBUG wallpaper: the device size differs; asked again output=winit wanted=Some((822, 1008))
DEBUG runtime: frame drawn frame=879 surface=2 next_callback=false     the surface at its new size, the old picture stretched
INFO  wallpaper: wallpaper uploaded output=winit width=822 height=1008
DEBUG runtime: frame drawn frame=985 surface=2 next_callback=false     the picture decoded for that size
```

Both are frames somebody needed. Four on the wallpaper's surface with none
on the panel's would be two such resizes. The smoke starts its nests with
`--monitor none`, so the host places the window and may resize it: in that
landing's own log the launcher run's nest was 942×961 device pixels and the
wallpaper run's, twenty seconds later, 1893×1983, on another monitor of a
session the author was using. **That the host resized the nest twice during
the failed run is inferred from these measurements, not observed:** its
journal no longer exists.

## The cutover run: what each window owes

The runtime now writes `metrics: rest frame surface=N` when it counts one,
so the journal says when each was drawn. The cutover run alone, repeated
with the journal kept:

| Runs | Script | Condition | Before the switch | After the switch |
|---|---|---|---|---|
| 9 | `main`, five-second rest | none | 0 | 1, the panel, 12.02 to 12.03 s after start |
| 1 | `main`, five-second rest | five resizes, two before the switch and three after | 4, the wallpaper (two per resize) | 1, the same panel frame |
| 5 | this tree before the review, nine-second rest | none | 0 | 1, the same panel frame |
| 1 | this tree, nine-second rest | four resizes before the switch | 8, the wallpaper (at most 10); the other surfaces 0 (at most 2) | 1 (at most 2) |

The frame after the switch is the same in every run. The system monitor's
first reading reaches the bar about two seconds in and is shown at once; the
next one is held back by the bar's ten-second cadence, which schedules a
redraw for twelve seconds in. The switch stops the monitor at five (now
nine) seconds and withdraws the reading, but the scheduled redraw still
fires:

```text
06:13:25.162  layer surface created surface=1 ... namespace="celestina-panel"
06:13:25.190  system monitor on
smoke-cutover-rewrite
06:13:30.595  system monitor off
06:13:37.191  metrics: rest frame surface=1 output=winit rest_frames=1
```

The first reading, the pieces arriving and the switch itself each start a
burst of animated frames (the journal shows them: about 700 frames in 200 ms
at the nest's unthrottled pace), so none of them is a rest frame. After the
switch a resize costs nothing: the wallpaper's surface is gone and the panel
animates to its new width.

With the after-switch budget set to 0 the run fails as it should, and prints
what the next reader needs:

```text
--- what preceded each rest frame (cutover.err)
... INFO celestina::energy: caffeine: released
... INFO celestina::clipboard: clipboard: the history file was deleted
... INFO celestina_wayland::metrics: metrics: rest frame surface=1 output=winit rest_frames=1
smoke: frames were drawn at rest after every duty went off (1, at most 0: the system monitor's held-back copy 1, the switch 1, notice 0): winit frames=2696 rest_frames=1 ...
```

**Why nine seconds.** Rest frames count from one second after a surface's
first frame. With a five-second rest the window before the switch is four
seconds long: a shell that redrew every two seconds while its duties are on
would draw two frames in it and pass any budget that allows the bar's and
the wallpaper's three. At nine seconds the window is eight seconds long and
that shell draws four; the window after the switch is about eight and a
half seconds long and it draws four there against a budget of two. The run
still exits at eighteen seconds.

## What changed

- `metrics.rs`: `SurfaceStats::on_frame` says whether the frame was a rest
  frame and `Metrics::on_frame` writes `metrics: rest frame` with the
  surface, the output and the running count. The rule stays in one place;
  the smoke reads the line instead of deriving rest frames from
  `frame drawn` by itself.
- The wallpaper run allows two frames for the surface's real scale, as
  before but with the reason the code has today, and two more for each
  `output changed` line after `wallpaper surface created`.
- The cutover run rests nine seconds before the switch instead of five and
  holds three counts to their own budgets. The wallpaper's surface, whenever
  it draws: two, plus two for each output change between `wallpaper surface
  created` and `wallpaper duty off`. Every other surface before the switch:
  two (the bar's late first events; the system monitor's held-back copy when
  the switch is three seconds late). Every other surface after it: two (that
  copy; the switch itself). The lock's missing-PAM notice, on a host without
  the service, adds one on each side. The three counts must add up to the
  metrics' total. The wrapper also says so when the shell has exited before
  the layers after the switch are listed, which nine seconds of rest leave
  five seconds for.
- `rest_fail` prints the journal's lines before each rest frame, without the
  frames, so the next failure says what asked for them.

## Result

On the tree rebased onto `c83807be` (commit `48d03715`):

| Command | Exit | Output |
|---|---|---|
| `cargo test --workspace --locked` | 0 | 2272 passed, 0 failed, 2 ignored, over 45 test binaries |
| Clippy, pinned 1.97.1, `--workspace --all-targets` | 0 | clean |
| Clippy 1.100.0, `--workspace --all-targets` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `sh -n` on every script of `celestina/scripts/` | 0 | clean |
| `bash scripts/check-architecture-contract.sh` | 0 | OK |
| `python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | OK |
| `timeout -s KILL 1500 sh scripts/smoke.sh --binary <release of this tree>`, three times | 0, 0, 0 | `smoke: OK` each, one attempt each, every run of the script |

Rest frames of the three full smokes (budget in parentheses):

| Run | 1 | 2 | 3 |
|---|---|---|---|
| first run, with the notification (2) | 1 | 1 | 1 |
| bar (1) | 0 | 0 | 0 |
| **wallpaper (2, no output change in any)** | 0 | 0 | 0 |
| glance (2) | 1 | 1 | 1 |
| glance with weather (2) | 1 | 0 | 1 |
| clipboard (one per duty switch) | 0 | 0 | 0 |
| connectivity (1) | 0 | 0 | 0 |
| phone and bubbles (0) | 0 | 0 | 0 |
| media, paused player (2) | 1 | 1 | 1 |
| **cutover: the wallpaper (2)** | 0 | 0 | 0 |
| **cutover: the rest before the switch (2)** | 0 | 0 | 0 |
| **cutover: the rest after the switch (2)** | 1 | 1 | 1 |

The cutover run's line, the same in the three but for the frame counts:

```text
smoke: cutover: every duty on then off: winit frames=2601 rest_frames=1 ... (rest_frames=1: the wallpaper 0, at most 2 with 0 output changes; the rest 0 before the switch, at most 2, and 1 after it, at most 2; over 2 surfaces)
smoke: cutover: rss_mib=101.2          (101.4 and 103.0 in the other two)
```

and the run with four resizes, on the same binary and script:

```text
smoke: cutover: every duty on then off: ... (rest_frames=9: the wallpaper 8, at most 10 with 4 output changes; the rest 0 before the switch, at most 2, and 1 after it, at most 2; over 2 surfaces)
```

A fourth full smoke, the second in time, also ended `smoke: OK` and is not
counted: another session's smoke ran beside it for a few seconds, which the
runner of these smokes watches for, so it was repeated. The release binaries
were built in a target directory of this session's own, since `smoke.sh`
builds its fake system bus beside the binary it is given and the sessions'
shared target directory was being rebuilt by another session.

## Review

A model other than the author of the change, high reasoning (2026-11-24),
read the diff, the script and the product code behind each claim: spec met;
quality approved after one Important finding, with four Minor ones.

- **Important:** the first version split everything at the wrapper's line and
  counted resizes only before it. The shell follows a rewritten `shell.json`
  after a 300 ms pause, so the wallpaper's surface outlives the line by about
  half a second, and a host resize there would land two wallpaper frames
  after the line against a budget that had no room for them: the very kind
  of failure this unit removes. The wallpaper's frames are now counted by
  surface, over the surface's own life, and only the other surfaces by side.
- **Minor, fixed:** the cutover counted resizes by a looser recipe than the
  wallpaper run (both now call `output_changes`, with an end); on a host
  without the PAM service the notice's allowance let a two-second redraw
  through before the switch (it is now one frame on each side); four
  sentences of the comments and of this record said more than the code or
  the measurements did; and the shorter time after the switch now fails with
  its own words when the shell has gone.
- The reviewer read, and found sound: that the wrapper's line and the
  shell's journal share one file in write order; that the journal's count and
  the metrics' total cannot differ (no surface is ever dropped from the
  summary, popups are in neither); and the Rust change.

The counts after the fix were checked against the kept journals without a
new nest (five resizes, two of them in the wallpaper's life: wallpaper 4,
before 0, after 1; the plain runs: 0, 0, 1) and then by the runs of the
result below.

## Limits

- The cause of the failed run of 2026-11-23 is inferred, as said above.
- This host has `/etc/pam.d/celestina-lock`, so the cutover run's
  missing-PAM branch and its notice allowance (one frame on each side of
  the switch) were not exercised; they are reasoned from the first run's
  rule and from the server's ten-second expiry.
- The wallpaper run's two frames for the real scale were never drawn in any
  run here; the allowance is kept from the reasoning in the script.
- **A finding, not fixed here:** the system monitor's held-back copy is a
  redraw scheduled ten seconds after the first reading
  (`island::cadence::Wake::After`, scheduled in `src/app.rs`), and stopping
  the monitor does not cancel it. In the cutover run the panel therefore
  draws one frame twelve seconds in with no reading left to show: one frame
  nobody needed, once. The after-switch budget names it; removing it is a
  change of the product for a unit of its own.
- No agent ran any live behaviour.

## Landing

- **Base revision:** `b1c12fc638e231e0d8c21fde8a9fbb667b2bb5f7`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:246976d95bab99bc4913fb63a07e38b9a0885483af6f975e39cad81dcc00cf86, verification_fingerprint sha256:f209774587e100874897a8cac3c0ec7750942c3c38bc0f9f8b85b770cf26dc03
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
