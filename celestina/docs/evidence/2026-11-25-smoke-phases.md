# Evidence: the smoke in phases (SMOKE-PHASES)

- **Date:** 2026-11-25
- **Scope:** unit SMOKE-PHASES, `celestina/scripts/smoke.sh` at `2ed007e9`.
- **Environment:** CachyOS, kernel 7.3.5-2-cachyos, rustc 1.97.1
  (8bab26f4f 2026-07-14), niri 26.04 (8ed0da4). Nested niri on the smoke's own
  bus; nothing ran in the author's session.
- **Artifact:** `target/smoke/release/celestina` of the worktree
  `celestina-SMOKE-PHASES`, built with `cargo build --release --locked --bin
  celestina`, plus `-p celestina-lock -p celestina-lock-verify` and `--example
  fake-mpris-player`.

## Baseline

The untouched script, before any change of this unit.

- **Lines:** `wc -l scripts/smoke.sh` gives 2165.
- **Calls of `fail`:** 228.
- **Calls of `rest_fail`:** 12.
- **Run:** one full smoke, exit 0, last line `smoke: OK`, 233 s wall clock
  (`real 3m53.410s`).

The two counts skip comment lines and the definitions (`fail() {`,
`rest_fail() {`, which the patterns do not match). The same commands are rerun
over the new files in the section below.

```sh
grep -v '^[[:space:]]*#' scripts/smoke.sh | grep -c -E '(^|[^_[:alnum:]])fail '
grep -v '^[[:space:]]*#' scripts/smoke.sh | grep -c -E '(^|[^_[:alnum:]])rest_fail '
```

## Procedure

```sh
cargo build --release --locked --bin celestina
cargo build --release --locked -p celestina-lock -p celestina-lock-verify
cargo build --release --locked --example fake-mpris-player
time timeout -k 60 2700 sh scripts/smoke.sh --binary target/smoke/release/celestina
```

## Result

- **Exit:** 0 for the baseline run (see Baseline); 0 for the three full runs,
  the fourteen single runs and the forced retry after the split; 143 for the
  TERM run.
- **Observed:** the baseline above; after the split, the runs under "After
  the split".
- **Checks**, from `celestina/` (the shared session target; the smoke target
  `target/smoke` is separate): `cargo fmt --all --check` exit 0; `cargo clippy
  --workspace --all-targets --locked -- -D warnings` exit 0; `cargo +1.100.0
  clippy --workspace --all-targets --locked -- -D warnings` exit 0; `cargo test
  --workspace --locked` exit 0 (46 `test result: ok` lines, none failed). From
  the worktree root: `bash scripts/check-documentation-contract.sh`, `python3
  scripts/check-language-contract.py` and `bash
  scripts/check-architecture-contract.sh`, each exit 0.

## After the split

`scripts/smoke.sh` is now the driver; the checks are in `scripts/smoke/`:
`common.sh` and one file per phase. Each phase file was built from a byte
copy of the old script (`target/smoke.sh.before`) by line range, with only
the edits the plan names (the calls `make_shell_run`, `make_wallpaper_fixture`,
`make_osd_fakes` and `need_python3`, and each phase's head).

- **Calls of `fail`:** 228, as in the baseline.
- **Calls of `rest_fail`:** 12, as in the baseline.

```sh
cat scripts/smoke.sh scripts/smoke/*.sh | grep -v '^[[:space:]]*#' | grep -c -E '(^|[^_[:alnum:]])fail '
cat scripts/smoke.sh scripts/smoke/*.sh | grep -v '^[[:space:]]*#' | grep -c -E '(^|[^_[:alnum:]])rest_fail '
```

- **Lines** (`wc -l`), 2460 in all against 2165 before:

| File | Lines |
|---|---|
| `scripts/smoke.sh` | 274 |
| `scripts/smoke/common.sh` | 253 |
| `scripts/smoke/notify.sh` | 76 |
| `scripts/smoke/launcher.sh` | 65 |
| `scripts/smoke/bar.sh` | 53 |
| `scripts/smoke/wallpaper.sh` | 92 |
| `scripts/smoke/energy.sh` | 83 |
| `scripts/smoke/glance.sh` | 106 |
| `scripts/smoke/clipboard.sh` | 251 |
| `scripts/smoke/connectivity.sh` | 139 |
| `scripts/smoke/osd.sh` | 151 |
| `scripts/smoke/phone.sh` | 191 |
| `scripts/smoke/media.sh` | 116 |
| `scripts/smoke/overview.sh` | 217 |
| `scripts/smoke/polkit.sh` | 52 |
| `scripts/smoke/cutover.sh` | 341 |

- **Tests:** `cargo test --locked --test nest_run --test lock_scripts --test
  duty_files` passes: 25, 11 and 8 tests. `sh -n` passes on all 16 files.
  `shellcheck` is not installed on this host.

Runs on the host date shown by `date` (Tue Nov 24 2026, EST; the nest's
UTC log stamps read 2026-11-25), release binary of the worktree at
`dbb19179`, one at a time in one chain (a status file at the end), each bound
with `timeout -k 60 2700` except the TERM run. No other smoke, lock rehearsal
or landing was running.

| Run | Exit | Last line | Wall clock |
|---|---|---|---|
| full smoke, run 1 | 0 | `smoke: OK` | 245 s |
| full smoke, run 2 | 0 | `smoke: OK` | 240 s |
| full smoke, run 3 | 0 | `smoke: OK` | 241 s |
| `--only notify` | 0 | `smoke: OK (only: notify)` | 5 s |
| `--only launcher` | 0 | `smoke: OK (only: launcher)` | 7 s |
| `--only bar` | 0 | `smoke: OK (only: bar)` | 5 s |
| `--only wallpaper` | 0 | `smoke: OK (only: wallpaper)` | 21 s |
| `--only energy` | 0 | `smoke: OK (only: energy)` | 7 s |
| `--only glance` | 0 | `smoke: OK (only: glance)` | 29 s |
| `--only clipboard` | 0 | `smoke: OK (only: clipboard)` | 44 s |
| `--only connectivity` | 0 | `smoke: OK (only: connectivity)` | 12 s |
| `--only osd` | 0 | `smoke: OK (only: osd)` | 26 s |
| `--only phone` | 0 | `smoke: OK (only: phone)` | 21 s |
| `--only media` | 0 | `smoke: OK (only: media)` | 18 s |
| `--only overview` | 0 | `smoke: OK (only: overview)` | 21 s |
| `--only polkit` | 0 | `smoke: OK (only: polkit)` | 6 s |
| `--only cutover` | 0 | `smoke: OK (only: cutover)` | 19 s |
| forced retry of `launcher` | 0 | `smoke: OK (only: launcher)` | 7 s |
| TERM during `--only phone` | 143 | (killed) | 1020 ms from TERM to exit |

Before and after, the full run:

| | Wall clock |
|---|---|
| Before the split (baseline) | 233 s |
| After the split, run 1 | 245 s |
| After the split, run 2 | 240 s |
| After the split, run 3 | 241 s |

The three runs after the split are 7 to 12 s slower than the baseline run
(3 to 5 %). The cause of the difference was not measured.

After the review's fixes (below) one more full run and one more forced retry
ran on the final scripts: the full run exit 0, last line `smoke: OK`, 262 s,
no `starting attempt` in its log; the forced retry of `launcher` exit 0, 7 s,
with the same three lines as above. The table's runs were made one commit
earlier, before those fixes.

The wall clock of a single phase includes the driver's checks before it and
the phase's wait for a safe second of the minute. `media` and `overview` ran
(no skip notice in either log): the example and `kitty` are present. No log of
a full run or of a single run holds `starting attempt`; only the forced run
does.

The forced retry ran
`CELESTINA_SMOKE_FORCE_RETRY=launcher timeout -k 60 2700 sh scripts/smoke.sh
--binary PATH --only launcher`. Its output holds `smoke: forced inconclusive
attempt (test hook)`, then `smoke: launcher: the rest-frame check was
inconclusive; starting attempt 2`, then one launcher run (`launcher opened and
closed`).

The TERM run started `sh scripts/smoke.sh --binary PATH --only phone` in the
background (no `timeout` wrapper, so `$!` was the driver's pid). The fake
Melibea process existed 1 s after the start; 3 s later `ps` showed the fake
Melibea, `dev-session.sh`, the nested niri and the phone shell, all under
`/tmp/celestina-smoke.*/phone.1`, and TERM went to the driver. It exited 143,
1020 ms after TERM. Two seconds after it, `pgrep -af
'[f]ake-melibea|[d]ev-session'` printed nothing and no
`${TMPDIR:-/tmp}/celestina-smoke.*` directory existed (there was none before
the run either).

## Review

Three rounds at most (a sensitive path). Round 1 read the whole diff and
found four things. Two were fixed: a third status 75 from a phase now fails
with the old loop's message and status 1 instead of ending the smoke with 75
and no line, and `every_listed_phase_has_its_file` now requires each phase
file to say `SMOKE_PHASE=<its id>`. Two were deferred to Follow-up. Round 2
read those two fixes only and found nothing.

## Lines of the old script

The old script's code lines that are in none of the new files. Blank lines
and comment lines are dropped, leading whitespace is stripped, and the two
sides are compared as sets of lines, from `celestina/scripts`:

```sh
norm() { grep -v '^[[:space:]]*$' | grep -v '^[[:space:]]*#' | sed 's/^[[:space:]]*//' | sort -u; }
norm < ../../target/smoke.sh.before > old.set
cat smoke.sh smoke/*.sh | norm > new.set
comm -23 old.set new.set    # this section: 12 lines
comm -13 old.set new.set    # next section: 88 lines
```

The old argument check (old lines 116–121), replaced by the driver's
parsing:

```text
[ "$#" -eq 2 ] && [ "$1" = "--binary" ] || usage
bin=$2
echo "usage: scripts/smoke.sh --binary PATH" >&2
```

The attempts loop (old lines 136–150), replaced by `run_phase`:

```text
[ "$attempt_status" -eq 75 ] || exit "$attempt_status"
attempt=$((attempt + 1))
attempt=1
attempt_status=0
echo "smoke: the nest was throttled by the host in 3 attempts; show its window" >&2
echo "smoke: the nest was throttled by the host; starting attempt $attempt of 3" >&2
if [ -z "${CELESTINA_SMOKE_ATTEMPT:-}" ]; then
nest_child env CELESTINA_SMOKE_ATTEMPT=$attempt sh "$0" "$@" || attempt_status=$?
while :; do
```

No other line is missing: every line that moved into a wrapper of
`common.sh` (`make_shell_run`, `make_wallpaper_fixture`, `make_osd_fakes`,
`need_python3`, `phase_cleanup`) is there byte for byte.

## Lines of the new files

The new files' code lines that are not in the old script, by file (a line in
several files is listed under each; 88 distinct lines). The driver's
parsing, selection and `run_phase`; `common.sh`'s refusal, test hook,
variable setup, lock and traps, and the heads of its function wrappers; and
in each phase its head (`SMOKE_PHASE=ID` and the line that sources
`common.sh`; the shebang and `set -eu` match old lines) and its calls of the
common functions.

`smoke.sh`:

```text
*)
*,,*)
*" $id "*)
*" $id "*) ;;
[ "$phase_status" -eq 75 ] || exit "$phase_status"
[ "$phase_status" -ne 0 ] || return 0
*) usage ;;
[ -z "$expect" ] && [ "$bin_given" = yes ] || usage
*[!a-z,]*)
bin=
--binary) [ "$bin_given" = no ] || usage; bin_given=yes; expect=binary ;;
binary) bin=$arg; expect=; continue ;;
bin_given=no
case " $phases " in
case " $selected " in
case " ${selected:-$phases} " in
case $arg in
case $expect in
case ",$only," in
case $only in
echo "       scripts/smoke.sh --list" >&2
echo "smoke: $1: the rest-frame check was inconclusive; starting attempt $((attempt + 1))" >&2
echo "smoke: $1: the nest was throttled by the host in 3 attempts; show its window" >&2
echo "smoke: OK (only: $ran)"
echo "smoke: --only: $id is named twice" >&2
echo "smoke: --only: $id is not a phase (scripts/smoke.sh --list prints them)" >&2
echo "smoke: --only '$only': a phase id is lowercase letters (scripts/smoke.sh --list prints them)" >&2
echo "smoke: --only '$only' names an empty phase id (scripts/smoke.sh --list prints them)" >&2
echo "usage: scripts/smoke.sh --binary PATH [--only ID[,ID...]]" >&2
expect=
for arg do
for attempt in 1 2 3; do
for id in $phases; do
for id in $(printf '%s\n' "$only" | tr ',' ' '); do
if [ "$#" -eq 1 ] && [ "$1" = "--list" ]; then
if [ "$only_given" = yes ]; then
mkdir "$work/$1.$attempt"
nest_child env CELESTINA_SMOKE_ATTEMPT=$attempt SMOKE_PHASE=$1 SMOKE_PHASE_WORK="$work/$1.$attempt" \
only=
--only) [ "$only_given" = no ] || usage; only_given=yes; expect=only ;;
only) only=$arg; expect=; continue ;;
only_given=no
phases="notify launcher bar wallpaper energy glance clipboard connectivity osd phone media overview polkit cutover"
phase_status=$?
phase_status=0
printf '%s\n' $phases
ran=
ran=$ran${ran:+,}$id
run_phase() {
run_phase "$id"
selected=
selected="$selected $id"
SMOKE_BIN="$bin" SMOKE_SCRIPTS="$here" SMOKE_NIRI="$niri_binary" sh "$here/smoke/$1.sh" ||
```

`smoke/common.sh`:

```text
bin=$SMOKE_BIN
echo "smoke: $SMOKE_PHASE: run it through smoke.sh --only $SMOKE_PHASE" >&2
echo "smoke: forced inconclusive attempt (test hook)" >&2
here=$SMOKE_SCRIPTS
if [ "${CELESTINA_SMOKE_FORCE_RETRY:-}" = "$SMOKE_PHASE" ] && [ "${CELESTINA_SMOKE_ATTEMPT:-}" = 1 ]; then
if [ -z "${SMOKE_PHASE_WORK:-}" ]; then
make_osd_fakes() {
make_shell_run() {
make_wallpaper_fixture() {
need_python3() {
nest_lock_acquire "smoke.sh phase $SMOKE_PHASE"
nest_signals phase_cleanup
niri_binary=$SMOKE_NIRI
phase_cleanup() {
run=
work=$SMOKE_PHASE_WORK
```

`smoke/notify.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
make_shell_run
SMOKE_PHASE=notify
```

`smoke/launcher.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
SMOKE_PHASE=launcher
```

`smoke/bar.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
make_shell_run
SMOKE_PHASE=bar
```

`smoke/wallpaper.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
make_shell_run
make_wallpaper_fixture
need_python3
SMOKE_PHASE=wallpaper
```

`smoke/energy.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
SMOKE_PHASE=energy
```

`smoke/glance.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
SMOKE_PHASE=glance
```

`smoke/clipboard.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
make_wallpaper_fixture
need_python3
SMOKE_PHASE=clipboard
```

`smoke/connectivity.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
SMOKE_PHASE=connectivity
```

`smoke/osd.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
make_osd_fakes
SMOKE_PHASE=osd
```

`smoke/phone.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
need_python3
SMOKE_PHASE=phone
```

`smoke/media.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
need_python3
SMOKE_PHASE=media
```

`smoke/overview.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
need_python3
SMOKE_PHASE=overview
```

`smoke/polkit.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
SMOKE_PHASE=polkit
```

`smoke/cutover.sh`:

```text
. "${SMOKE_SCRIPTS:-$(dirname -- "$0")/..}/smoke/common.sh"
make_osd_fakes
make_wallpaper_fixture
need_python3
SMOKE_PHASE=cutover
```

## Limits

- INT to the driver was not run; only TERM was.
- The real skip paths of `media` and `overview` did not run (neither skipped:
  the example and `kitty` are present).
- No unforced retry happened: attempt 2 or 3 after a real inconclusive
  rest-frame check was never observed; only the forced attempt 2 ran.
- One host, one baseline run and three runs after the split; the timings are
  not a benchmark.
- The counts are of call sites by pattern, not a parse of the shell.
- A third status 75 from a phase (the driver's own failure message) was not
  provoked: `rest_fail` fails on attempt 3, so no check produces it.
- The plan was not copied to `docs/superpowers/plans/`: that path is outside
  the `celestina:` commit scope.

## Follow-up

A suite unit adds `celestina/scripts/smoke` to the shell's
`verification_inputs` in `docs/projects.toml`. Until then only `smoke.sh`
itself is fingerprinted, so a change to a phase file alone does not make the
shell's verification stale. The same unit may register `nest-run.sh`,
`dev-session.sh`, `smoke-notify.sh`, `smoke-media.sh`, `fake-melibea.py` and
the two probes, and carry the copy of the plan.

The same suite unit widens the sensitive-path list of
`docs/standards/verification.md`: its `smoke*.sh` does not match
`scripts/smoke/<id>.sh`, so a unit that changes only a phase file is not
counted as sensitive.

`need_python3` runs at the top of `media` and `overview`, before their skip
notice, and its message is the wallpaper gate's. On a host without python3,
`--only media` fails with a line about the wallpaper gate. The full smoke is
unchanged: it failed at the wallpaper gate before the split too.

## Landing

- **Base revision:** `2ed007e9bee6f9951d5620abbf650d88d705c05b`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:e39e41fd026c2ab518a284d55ce375e6dc6bbf5a124309f6201eb4be278ae64e, verification_fingerprint sha256:681d40008f29ccd3b1bad5b82f8476a8830d54c777432d66a08aff68adf391d2
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
