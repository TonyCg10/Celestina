# Evidence: SMOKE-LOCK — measuring runs take turns, and a signal ends a smoke with its nest

- **Date:** 2026-11-24
- **Scope:** `celestina/scripts/nest-run.sh` (new: the nest lock, `nest`,
  the signal traps), its callers `smoke.sh`, `wallpaper-backdrop-probe.sh`
  and `lock-rehearsal.sh`, the notice of `dev-session.sh`, the tests of
  `celestina/tests/nest_run.rs`, the sentences of `README.md` and
  `AGENTS.md` about a measuring run, and this unit's ledger row. No code of
  the product changed.
- **Environment:** CachyOS, worktree `celestina-SMOKE-LOCK` cut from `main`
  at `3ecb7b5b` (celestina 1.15.1); toolchains 1.97.1 (pinned) and 1.100.0;
  util-linux `flock` and `setsid`, GNU `timeout`; `/bin/sh` here is bash in
  POSIX mode. Nested niri through `scripts/smoke.sh` only (own bus, a system
  bus address that is not there, fakes first on `PATH`, throwaway
  configuration and state homes). The author was told before the first
  nest. **Nothing ran in the author's session** but the nests' own windows
  and the lock file of this unit in the runtime directory.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

1. **Nothing kept two measuring runs apart.** `smoke.sh` judges frames at
   rest in about twenty nested niri sessions, each a window in the author's
   session, and every landing runs it. On 2026-11-24 several agent sessions
   and a landing ran smokes at the same time; several runs of this session had
   to be thrown away because another's nest opened beside them, and one of
   this session's ran beside a landing's own smoke.
2. **TERM did not stop a smoke.** Its trap was
   `trap 'rm -rf "$work"' EXIT INT TERM`: on TERM the script deleted its
   work directory and then started the next nested run without it. A shell
   also defers a trap until its foreground command ends, so even a trap that
   exits would have waited out the nest in progress.
   `wallpaper-backdrop-probe.sh` had the same trap.

## What changed

- **One lock** (`nest_lock_acquire`, in the new `scripts/nest-run.sh`):
  `$XDG_RUNTIME_DIR/celestina-nest.lock`, taken by `smoke.sh`,
  `wallpaper-backdrop-probe.sh` and `lock-rehearsal.sh` in their outermost
  invocation, before any nest. A second run waits
  `CELESTINA_NEST_LOCK_WAIT` seconds (default 1800), says once who holds the
  lock, and fails with status 69 naming the holder when the wait runs out.
  An interactive `dev-session.sh` nest takes no lock and only says that a
  run is measuring.
- **The lock lasts as long as any process of the run.** The script holds no
  descriptor on the lock (every nested niri and client would inherit it);
  a holder process started in a session of its own has it, ignores TERM, INT
  and HUP, and keeps it while the script, one of its attempts or a nest's
  process group recorded in the run's hold directory is alive. So neither a
  KILL of the script nor a signal or a KILL to its whole process group frees
  the lock while a nest is still measuring; an orphaned attempt starts no
  further nest; and the next run sweeps what a killed one left.
- **TERM and INT end the run.** Every nest starts through `nest`, which runs
  `timeout LIMIT env …` in the background and waits, so the trap runs while
  the nest is up: it sends TERM to the nest's process group (GNU `timeout`
  makes one), KILL after five seconds, runs the cleanup once and exits 143
  or 130. The attempts loop forwards the signal to the attempt.
- A caller bounds a smoke with TERM first and a limit that covers the wait
  plus the run (`timeout -k 60 2700 …`), never `timeout -s KILL`;
  `celestina/AGENTS.md` says so.

## Procedure

```sh
# in celestina/, on the tree of this unit (fe4a7e10)
cargo test --test nest_run
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings          # 1.97.1, pinned
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
for f in scripts/*.sh; do sh -n "$f"; done
# at the root
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
# with the release binaries of this tree in a target directory of the session's own:
#   two real smokes started 25 s apart, then TERM to each (the experiment below)
timeout -k 60 2700 sh scripts/smoke.sh --binary <release of this tree>   # three times
```

## Tests

`tests/nest_run.rs`, twenty tests in about a second, with scratch drivers
that source the real `nest-run.sh`, a scratch `XDG_RUNTIME_DIR` and no
compositor: a second run waits for the first and never overlaps it; the lock
is free within three seconds of a killed holder; a script started by a
holder does not wait for it, and a marker that names no live hold directory
is ignored; a killed outer script keeps the lock while its attempt and nest
run, and that attempt starts no further nest; a KILL and a TERM to the whole
process group keep the lock until the nest is gone; a refused nest says so
on the script's own stderr; a run without a private runtime directory is
refused; the directory of a killed waiter is swept; TERM and INT end a run
and its nest with 143 and 130 and one cleanup; the smoke takes the lock
before anything else; an interactive nest is told about a measuring run;
and two guards on the scripts' text (`dev-session.sh` is started only
through `nest`, no script keeps a signal trap of its own, only the outer
drivers take the lock). Each was seen failing first, against the code
before it or against a mutation of it.

## Result

**Two real smokes and two signals.** A smoke X was started, a second one Y
25 s later, both with the release binary of this tree; twenty seconds on, X
was 14 runs in and Y had printed one line and started nothing:

```text
nest-run: waiting for the nest lock, held by: 2236124 smoke.sh --binary <release>/celestina Tue Nov 24 01:36:26 PM EST 2026 (at most 1800s, CELESTINA_NEST_LOCK_WAIT)
```

| Step | Observed |
|---|---|
| TERM to X, in the middle of a nested run | exit status 143 after 1.2 s; a second later the owner file names Y |
| Y, thirty seconds later | 13 runs in, its own nest up |
| TERM to Y | exit status 143 after 1.0 s |
| afterwards | no nested niri, bus or client of either run alive; no hold directory; no owner file; the lock free (`flock -n` takes it); neither run's work directory left in the temporary directory |

| Command | Exit | Output |
|---|---|---|
| `cargo test --test nest_run` | 0 | 20 passed |
| `cargo test --workspace --locked` | 0 | 2294 passed, 0 failed, 2 ignored, over 46 test binaries |
| Clippy, pinned 1.97.1, `--workspace --all-targets` | 0 | clean |
| Clippy 1.100.0, `--workspace --all-targets` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `sh -n` on every script of `celestina/scripts/` | 0 | clean |
| `bash scripts/check-architecture-contract.sh` | 0 | OK |
| `python3 scripts/check-language-contract.py` | 0 | OK |
| `bash scripts/check-documentation-contract.sh` | 0 | OK |
| `timeout -k 60 2700 sh scripts/smoke.sh --binary <release of this tree>`, three times | 0, 0, 0 | `smoke: OK` each, one attempt each, every run of the script, none waiting for the lock |

The three full smokes ran every nest through `nest` under the lock and
measured what `main` measures: wallpaper 0 at rest; cutover `the wallpaper 0,
the rest 0 before the switch and 1 after it` in each. Afterwards the runtime
directory held the lock file alone.

**A test that depended on who started it.** The first full run of the tests
failed one: `int_ends_the_run_and_its_nest` timed out. That run was started
from a background job, where INT is ignored; the disposition is inherited
through `exec`, and a POSIX shell cannot trap a signal that was ignored when
it started, so the test's driver never saw INT. A landing may start its
tests the same way. Every driver of the tests now starts through
`env --default-signal=INT,QUIT,HUP`, as the SIGINT test of
`celestina-rs/crates/selenita-core` already does, and the whole suite above
passed started from such a job. The head of `nest-run.sh` says what this
means for a real run: one started with INT ignored can be ended by TERM
only.

A fourth full smoke also ended `smoke: OK` and is not counted: a nest of
another session, whose worktree does not have the lock yet, ran beside it
for a few seconds, so it was repeated.

## Reviews

A model other than the implementer, high reasoning, twice (2026-11-24),
with experiments of its own against the real `nest-run.sh`.

- **First review:** two Important findings. The lock lived only as long as
  the outer script's pid, so a `kill -9` of it, or `timeout -s KILL` around
  it, freed the lock while the attempt and its nest went on measuring; and
  the default wait of 1800 s was longer than the 1500 s KILL guard sessions
  put around a smoke. Fixed: the hold directory became the run's registry
  and the documents say how to bound a smoke. Nine Minor findings, fixed.
- **Second review:** the first findings fixed but for one case, a KILL to
  the whole process group, which took the holder with it; one Important
  finding for that, fixed by starting the holder in a session of its own,
  and eight Minor ones, fixed (a refusal that was written to a log the
  cleanup deletes, a release that could end the cleanup, the rehearsal's
  nests outside the registry, leftovers of a killed waiter, a fallback in
  the shared `/tmp`). That last round was read whole by the coordinating
  session and exercised by the experiment below, not by a third review.

## Limits

- A KILL aimed at the holder process itself, the machine going down, or a
  client that leaves its nest's process group with `setsid` are outside what
  the lock can keep; the head of `nest-run.sh` says so.
- A smoke killed with KILL leaves its work directory in the temporary
  directory: nobody is left to remove it.
- The lock binds the scripts of this repository from this unit on. A
  session whose worktree was cut before it runs a `smoke.sh` without the
  lock until it rebases.
- `/bin/sh` on this host is bash in POSIX mode; `dash` is not installed, so
  the scripts were read for POSIX and run under bash only.
- `lock-rehearsal.sh` and `wallpaper-backdrop-probe.sh` were not run for
  real here: their lock and their records are covered by the tests on the
  text and by the same functions the smoke exercises.
- No agent ran any live behaviour.

## Landing

- **Base revision:** `9e236bbbd8d6d5a006d4823a7160860ff6a30fb0`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:17ebd1b56b668a552fa08a48981832bfb00ae283a52026b1a87e38359cd0a5a5, verification_fingerprint sha256:8667d89778833e7e352026551c22816f05ccd565fad0d5f62cc9af70e220ded6
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
