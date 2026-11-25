# Celestina smoke in phases — plan (U03, `SMOKE-PHASES`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `celestina/scripts/smoke.sh` runs the same checks from one file per phase, can run a chosen subset, and reruns only the phase whose rest-frame check was inconclusive.

**Architecture:** `smoke.sh` becomes a driver: arguments, the nest lock, the checks that need no nest, then one child process per phase. Today the script already reruns itself as a child for each of three attempts (`nest_child env CELESTINA_SMOKE_ATTEMPT=N sh "$0"`, exit 75 means "inconclusive, try again"); the driver keeps that exact mechanism and applies it per phase. Each phase is `celestina/scripts/smoke/<id>.sh`, sources `celestina/scripts/smoke/common.sh` for the helpers and shared fixtures, and works in a directory of its own.

**Tech Stack:** POSIX `sh`, `celestina/scripts/nest-run.sh` (lock, `nest`, `nest_child`, `nest_signals`), Rust integration tests that read the scripts as text (`celestina/tests/{nest_run,lock_scripts,duty_files}.rs`).

**Spec:** `docs/superpowers/specs/2026-11-24-suite-dev-speed-design.md`, section 4 "U03". Ledger: a new row `SMOKE-PHASES` in `celestina/docs/plans/active/2026-10-16-cel-2-parity.md`, prefix `celestina:`, where `SMOKE-LOCK` and `SMOKE-REST` are.

## Global Constraints

- Repository text is English; the author is addressed in Spanish.
- Never edit the canonical checkout. Open the unit from it: `scripts/worktree.sh open celestina SMOKE-PHASES`. First edit: add the ledger row (`active`, evidence link `[evidence](../../evidence/2026-11-25-smoke-phases.md)`, intended change "Add phases to the smoke: one file per phase, a selection and a retry of one phase") and copy this plan to `docs/superpowers/plans/2026-11-25-celestina-smoke-phases-plan.md`. The plan file changes only by that new row.
- Session commits: `celestina-maintenance: <Add|Fix|Remove|Update> …`.
- `smoke*.sh` is a sensitive path (`docs/standards/verification.md`): an ordinary unit, at most three review rounds.
- The session never runs `scripts/land-unit.py`, `git push`, a merge into `main` or a `*-production.sh`.
- Smoke runs: never against the author's session (the smoke only ever starts a nested niri on a bus of its own; do not change that). Never two at once: the smoke takes the host lock itself; before starting one, `pgrep -af 'smoke.sh --binary|lock-rehearsal.sh|land-unit.py'` must show no foreign run. Bound each run with `timeout -k 60 2700 sh scripts/smoke.sh …`, never `timeout -s KILL`.
- The binary under test is built in `<worktree>/target/smoke` (`CARGO_TARGET_DIR`), all from `celestina/`: `cargo build --release --locked --bin celestina`, then `-p celestina-lock -p celestina-lock-verify`, then `--example fake-mpris-player`. A full smoke takes about four minutes and a background shell call is capped at ten: start a chain with `setsid nohup sh chain.sh > chain.log 2>&1 &` and wait on a status file the chain writes.
- **Checks move unchanged.** No assertion, budget, timeout, sleep or log pattern changes. The only permitted edits to moved lines are the ones this plan names.
- Do not touch `docs/projects.toml`: a `celestina:` unit cannot carry it. Registering `celestina/scripts/smoke` as a verification input is a follow-up suite unit (last section).
- Before the hand-over, from `celestina/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the same clippy with `cargo +1.100.0` (the CI runner's toolchain), `cargo test --workspace --locked`.
- Line numbers below are `celestina/scripts/smoke.sh` at `2ed007e9`.

## Review Focus

1. **A check lost or changed in the move.** Task 3 records two set differences between the old file and the new files; the reviewer reads both lists whole.
2. **A phase that only passed because an earlier one ran.** `clipboard` and `cutover` use the wallpaper picture; `cutover` uses the fake audio tools; several use `python3` unchecked. Each phase must pass alone (Task 4).
3. **A retry that reuses a dirty directory.** A second attempt must start in an empty phase directory: stale `*.status` files, the appended `curl` log, an existing FIFO or clipboard history would turn a failure into a pass or a pass into a failure.
4. **Signals.** TERM or INT to the driver must end the running phase and its nest within seconds and leave nothing behind (the fake Melibea server is started without `nest`).
5. **Tests that read `smoke.sh` by name.** Moving code out of it must not silently drop their coverage: every guard that scanned the old file scans the driver and every phase file.

## The map of today's file

**Preamble (1–330):** header comment 1–115; `usage` and the two-argument check 116–121; `here` from `$0` 122; `. "$here/nest-run.sh"` 123; `nest_lock_acquire "smoke.sh $*"` 125; the attempts loop 136–150; binary check 151–155 (message "is not an executable"); the lock's two binaries and their exit codes 163–184; `"$bin" --check` 186–189; `cutover-config-probe.sh` on the two fixtures 191–210; no niri → notice and `exit 0` 212–215; no `WAYLAND_DISPLAY` 216–220; no `dbus-run-session` 221–224; wait for second 2..48 of the minute 228–230; `probe_args` 232–237; `CELESTINA_HOST_BUS` 239–240; `work=$(mktemp -d …)` 242; `fake_pid=`, `cleanup`, `nest_signals cleanup` 246–252; quote guards 256–280 (`niri_binary`); `duties()` 292–302; `xdg_default`, `xdg_state`, exports and the dead system bus 303–314; `shell_run` wrapper 319–324.

**Helpers defined after the first nest (332–418):** `tails`, `fail`, `nest_throttled`, `rest_context`, `output_changes`, `rest_fail`, `sum`. `tails` reads the global `$run`.

**Phases:**

| id | Lines | Needs beyond the common helpers |
|---|---|---|
| `notify` | 228–230, 232–240, 319–445 | `probe_args`, `CELESTINA_HOST_BUS`, `shell_run`, default config |
| `launcher` | 447–499 | default config |
| `bar` | 500–539 | `shell_run` |
| `wallpaper` | 540–643 | `python3`, `shell_run`; creates the picture `default.png` |
| `energy` | 645–711 | — |
| `glance` | 713–804 (`glance_run`, two runs) | — |
| `clipboard` | 806–1036 (`want`) | the wallpaper picture (826–827) |
| `connectivity` | 1037–1160 (cargo build of `fake_system_bus`, two runs) | — |
| `osd` | 1162–1329 (`osd_wrapper`, `osd_run`, two runs); creates the fake `wpctl`/`pactl`/`ddcutil`/`brightnessctl` | — |
| `phone` | 1331–1509 (`stop_fake`, `phone_prelude`, three runs, `fake-melibea.py` started without `nest`) | `niri_binary`, `fake_pid` |
| `media` | 1511–1613 (`media_run`; skipped with a notice without the example or a bus tool) | `CELESTINA_HOST_BUS`, `python3` |
| `overview` | 1614–1814 (`overview_run`, `column_of`; skipped without `kitty`) | `python3` |
| `polkit` | 1817–1856 | — |
| `cutover` | 1857–2164 (`cutover_run`) | the wallpaper picture's folder (1928), the fake audio tools (1955), `niri_binary`, `python3` |

`echo "smoke: OK"` is line 2165. `rest_fail` call sites: 430, 532, 635, 781, 1033, 1126, 1442, 1600, 1811, 2155, 2157, 2159.

---

### Task 1: Baseline

**Files:**
- Create: `docs/evidence/2026-11-25-smoke-phases.md` under `celestina/` (`celestina/docs/evidence/…`, from `docs/templates/evidence.md`)

- [ ] **Step 1:** Open the worktree, add the ledger row, copy the plan. Build the three artifacts into `<worktree>/target/smoke`.
- [ ] **Step 2:** Record in the evidence, from the untouched script: `grep -c` of lines calling `fail ` and `rest_fail ` outside comments and definitions; `wc -l`; and one full run, `time timeout -k 60 2700 sh scripts/smoke.sh --binary <target>/release/celestina`, with its exit, its last line and the wall-clock time.
- [ ] **Step 3:** Save a copy of the untouched script outside the repository tree (`<worktree>/target/smoke.sh.before`) for Task 3's comparison. Commit the row, the plan and the evidence stub: `celestina-maintenance: Add the smoke phases unit and its baseline`.

### Task 2: Tests for the new layout (red first)

**Files:**
- Modify: `celestina/tests/nest_run.rs` (the text guards at 763–842 and the lock-order test at 610–635)
- Modify: `celestina/tests/lock_scripts.rs` (`SCRIPTS`, 30–42)
- Modify: `celestina/tests/duty_files.rs` (the `smoke.sh` lookups at 321–333 and 384–390)

**Interfaces:**
- Produces a helper in each test file that needs it: `smoke_sources() -> Vec<(String, String)>`: `scripts/smoke.sh` plus every `*.sh` in `scripts/smoke/`, sorted, as (relative path, text). The directory must exist and hold at least 15 files (`common.sh` and 14 phases).
- Guards, restated over `smoke_sources()`:
  - every statement naming `dev-session.sh` starts with `nest` and carries no `timeout`; no `trap` names INT or TERM; none of `| nest `, `(nest `, `$(nest `, `` `nest `` appears: in every file;
  - at least one `dev-session.sh` statement exists across the phase files, and **none** in `smoke.sh`;
  - `smoke.sh` contains the statement `nest_signals cleanup` and a `nest_lock_acquire ` statement before its first `nest_child `; every phase file contains a `nest_lock_acquire ` statement before its first `nest `; `smoke-media.sh` still has none;
  - `lock_scripts.rs`: `SCRIPTS` is the old list plus every file of `scripts/smoke/`, read from the directory, so a new phase is covered without editing the list; `celestina-lock-verify` is still required in `smoke.sh`;
  - `duty_files.rs`: the `for d_name in … ; do` line equal to `Duties` is required in `smoke/common.sh` (exactly one file defines `duties()`).
- New tests in `nest_run.rs`, hermetic (no compositor, no binary):
  - `the_smoke_lists_its_phases`: `sh scripts/smoke.sh --list` exits 0 and prints exactly, one per line: `notify launcher bar wallpaper energy glance clipboard connectivity osd phone media overview polkit cutover`. It must not take the lock (run it while a scratch holder has the lock and `CELESTINA_NEST_LOCK_WAIT=0`: still 0).
  - `the_smoke_refuses_an_unknown_phase`: `--binary /nonexistent/celestina --only nope` exits 2, names `nope`, and does not take the lock.
  - `the_smoke_refuses_an_empty_or_repeated_selection`: `--only ""` and `--only bar,bar` exit 2.
  - `every_listed_phase_has_its_file`: each id of `--list` has `scripts/smoke/<id>.sh`, and the directory holds no other file than those and `common.sh`.
  - `a_phase_refuses_to_run_outside_the_driver`: `sh scripts/smoke/bar.sh` with no `SMOKE_PHASE_WORK` exits 2 and says `run it through smoke.sh --only bar`.
- The existing `the_smoke_takes_the_lock_before_anything_else` keeps its two expectations (69 with the holder named; 1 with "is not an executable").

- [ ] **Step 1:** Write the changes above.
- [ ] **Step 2:** Run `cd celestina && cargo test --locked --test nest_run --test lock_scripts --test duty_files`. Expected: the new and restated tests FAIL (no `scripts/smoke/` directory), the untouched ones pass.
- [ ] **Step 3:** Commit: `celestina-maintenance: Add the tests of the smoke's phase layout`.

### Task 3: The split

**Files:**
- Modify: `celestina/scripts/smoke.sh` (becomes the driver)
- Create: `celestina/scripts/smoke/common.sh`
- Create: `celestina/scripts/smoke/{notify,launcher,bar,wallpaper,energy,glance,clipboard,connectivity,osd,phone,media,overview,polkit,cutover}.sh`
- Modify: `celestina/scripts/nest-run.sh` (comments only: lines 7–15, 50, 71, 81 describe the per-script attempts)

**Interfaces:**

*The driver, `smoke.sh`:*
- Usage: `scripts/smoke.sh --binary PATH [--only ID[,ID…]]` and `scripts/smoke.sh --list`. Anything else: `usage`, exit 2. `--list` prints the ids in file order and exits before the lock. An unknown, empty or repeated id in `--only` is a usage error naming it, before the lock.
- Order, unchanged from today where it exists: `here`; source `nest-run.sh`; `nest_lock_acquire "smoke.sh $*"`; `nest_signals nest_lock_release` until `work` exists; the binary check with its message; the lock's two binaries; `--check`; the cutover probe; the three environment checks (`exit 0` with the notice when niri is missing, as today, before any phase); `work=$(mktemp -d …)`; `cleanup` (removes `$work`, releases the lock) and the statement `nest_signals cleanup`; the quote guards; `niri_binary`.
- These checks run once per invocation, not once per attempt as today.
- `run_phase ID`: for `attempt` in 1 2 3: make the empty directory `$work/ID.$attempt`; run `nest_child env CELESTINA_SMOKE_ATTEMPT=$attempt SMOKE_PHASE=ID SMOKE_PHASE_WORK="$work/ID.$attempt" SMOKE_BIN="$bin" SMOKE_SCRIPTS="$here" SMOKE_NIRI="$niri_binary" sh "$here/smoke/ID.sh"`; status 0 → return; status 75 and `attempt` under 3 → print `smoke: ID: the rest-frame check was inconclusive; starting attempt N` and continue; any other status → the driver exits with it. (As today, a third inconclusive attempt fails inside `rest_fail`, so the driver never sees 75 from attempt 3.)
- The driver runs the selected phases in file order and then prints `smoke: OK`. With `--only`, the last line is `smoke: OK (only: ID,ID)`.
- The driver's header comment keeps the contract (usage, lock, signals, exit statuses, the three attempts now per phase) and loses the per-phase descriptions, which move to each phase file's header.

*`smoke/common.sh`, sourced by every phase:*
- Refuses when `SMOKE_PHASE_WORK` is unset: `smoke: SMOKE_PHASE: run it through smoke.sh --only ID`, exit 2 (the phase passes its own id).
- Sets `bin=$SMOKE_BIN`, `here=$SMOKE_SCRIPTS`, `work=$SMOKE_PHASE_WORK`, `niri_binary=$SMOKE_NIRI`, `run=` and `fake_pid=`; sources `"$here/nest-run.sh"`; calls `nest_lock_acquire "smoke.sh phase $SMOKE_PHASE"` (it returns at once: the driver holds the lock, as an attempt's child does today); defines `phase_cleanup` (calls `stop_fake` when `fake_pid` is set; removes nothing, the driver owns `$work`) and runs `nest_signals phase_cleanup`.
- Holds, moved unchanged: `duties` (292–302), the default config and state homes and the three exports (303–314, now inside the phase's `work`), `CELESTINA_HOST_BUS` (239–240), `tails`, `fail`, `nest_throttled`, `rest_context`, `output_changes`, `rest_fail`, `sum` (332–418), `stop_fake` (1343–1345).
- New functions that wrap moved lines, each idempotent and writing under `$work`:
  - `need_python3`: the check of 566–568, called by every phase that runs `python3`;
  - `make_shell_run`: lines 319–324, sets `shell_run`;
  - `make_wallpaper_fixture`: lines 569–593, sets `fixture`;
  - `make_osd_fakes`: lines 1184–1215, sets `osd_fakes`;
  - `wait_minute LOW HIGH`: the wait of 228–230 generalised only if the phases' own waits are textually the same loop with other bounds; otherwise each phase keeps its own lines.
- Test hook, documented in the file as test-only: when `CELESTINA_SMOKE_FORCE_RETRY` equals `$SMOKE_PHASE` and `CELESTINA_SMOKE_ATTEMPT` is 1, print `smoke: forced inconclusive attempt (test hook)` and `exit 75` before anything else.

*Each phase file:* `#!/bin/sh`, `set -eu`, a header comment with that phase's paragraph of today's header, `SMOKE_PHASE=<id>`, `. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"`, then the phase's lines from the map, unchanged, with these edits only:
- a call to the common function replaces the lines that moved into it (`make_shell_run` in `notify`, `bar`, `wallpaper`; `make_wallpaper_fixture` in `wallpaper`, `clipboard`, `cutover`; `make_osd_fakes` in `osd`, `cutover`; `need_python3` in `wallpaper`, `phone`, `media`, `overview`, `cutover`);
- `osd`: `set -- $osd_counts` (1303) stays as it is; it is now local to the phase's process;
- a phase that today skips with a notice (`media`, `overview`) prints the same notice and exits 0.
- Every `nest` statement stays a plain statement, as `nest_run.rs` requires.

- [ ] **Step 1:** Write `common.sh`, the fourteen phase files and the driver. Move with an editor-level cut and paste by line range; do not retype.
- [ ] **Step 2:** Update the comments of `nest-run.sh`.
- [ ] **Step 3:** Record the two comparisons in the evidence, from `celestina/scripts`, ignoring blank lines, comment lines and leading whitespace:
  - lines of `<worktree>/target/smoke.sh.before` that appear in none of the new files (expected: the old argument check, the attempts loop 136–150 and the header comment; list every line);
  - lines of the new files that are not in the old one (expected: the driver's parsing and `run_phase`, `common.sh`'s refusal, variable setup, hook and function wrappers, each phase's five-line head, the calls to the common functions; list every line).
  Also record `grep -c` of `fail ` and `rest_fail ` call sites across the new files: it must equal Task 1's counts.
- [ ] **Step 4:** Run `sh -n` on every file, `cd celestina && cargo test --locked --test nest_run --test lock_scripts --test duty_files`. Expected: all pass.
- [ ] **Step 5:** Commit: `celestina-maintenance: Add the smoke's driver, its common library and one file per phase`.

### Task 4: Proof on real runs, documents, hand-over

**Files:**
- Modify: `celestina/AGENTS.md` (lines 48–59: the smoke's command forms), `celestina/README.md` (lines 290–297; its `--exit-after 3` is stale, the script uses 4)
- Modify: `celestina/docs/evidence/2026-11-25-smoke-phases.md`

- [ ] **Step 1:** Run and record, each with `time` and its last line (one chain, in the background, results in files):
  - the full smoke three times in a row: exit 0 and `smoke: OK` each time;
  - `--only <id>` for each of the fourteen ids: exit 0 each (a skip notice for `media` or `overview` is recorded as a skip, not a pass);
  - `CELESTINA_SMOKE_FORCE_RETRY=launcher … --only launcher`: exit 0, the output shows the forced attempt, the "starting attempt 2" line and one real run;
  - TERM to the driver in the middle of `--only phone`: it exits 143 within a few seconds, `pgrep -af 'fake-melibea|dev-session'` shows nothing of it left, and its work directory is gone.
- [ ] **Step 2:** Update the two documents with the new command forms: the full run, `--only`, `--list`, and that a session runs the phases of the area it touched while the landing always runs everything.
- [ ] **Step 3:** From `celestina/`: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings`; `cargo test --workspace --locked`. From the worktree root: `bash scripts/check-documentation-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-architecture-contract.sh`. Expected: each exits 0. Delete `<worktree>/target/smoke` afterwards.
- [ ] **Step 4:** Finish the evidence (before and after wall-clock of the full run, the fourteen single runs, the two line lists, **Limits:** what was not run). Commit: `celestina-maintenance: Add the proof runs and documents of the smoke phases`.
- [ ] **Step 5:** Report to the coordinating session and stop: branch, commits, the timings, the two line lists' sizes, what was not done or not checked, and the author's command. The landing re-verifies the shell (release build, clippy, tests, the full smoke): several minutes.

```sh
cd /home/toni/CODIGO/CELESTINA
time python3 scripts/land-unit.py unit/celestina/SMOKE-PHASES --kind maintenance --summary "Add phases to the smoke: one file per phase, a selection and a retry of one phase"
```

## Follow-up, not part of this unit

A suite unit (a new row in the `dev-speed` ledger) adds `celestina/scripts/smoke` to the shell's `verification_inputs` in `docs/projects.toml`, so that a change to a phase file makes the shell's verification stale. Until it lands, only `smoke.sh` itself is fingerprinted. The same unit can register the helpers that are not inputs today either (`nest-run.sh`, `dev-session.sh`, `smoke-notify.sh`, `smoke-media.sh`, `fake-melibea.py`, the two probes), if the author wants that hole closed.

## Chunks and models (heuristic estimate)

| Chunk | Model | Reasoning | Why |
|---|---|---|---|
| U03.1 · Baseline (Task 1) | Sonnet 5.5 | low | Build and one timed run |
| U03.2 · Tests of the layout (Task 2) | Sonnet 5.5 | medium | Rust text guards with pinned expectations |
| U03.3 · The split (Task 3) | Opus 5.5 | high | 2 165 lines of shell, signals and shared state; one agent must hold all of it |
| U03.4 · Proof runs and documents (Task 4) | Sonnet 5.5 | medium | Long runs in the background, tables, two documents |
| U03.R · Final review | Fable 5.2 | high | Different from the implementers; `/code-review`; up to three rounds, sensitive path |

Tasks run in order in one worktree.
