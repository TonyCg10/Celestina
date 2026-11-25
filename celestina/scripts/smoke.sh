#!/bin/sh
set -eu

# Celestina smoke (design §8): the driver. The checks live in one file per
# phase under `scripts/smoke/` (`common.sh` holds what they share); this script
# reads the arguments, takes the nest lock, runs the checks that need no nest
# and then every phase. Each phase file's header says what it checks.
#
#   scripts/smoke.sh --binary PATH [--only ID[,ID...] | --order ID[,ID...]]
#   scripts/smoke.sh --list
#   scripts/smoke.sh --next-phase SECOND ID[,ID...]
#
# With niri installed every phase runs the shell in a nested niri on a session
# bus of its own (`dev-session.sh --once`), so no name of the desktop's bus is
# touched. `--list` prints the phase ids, one per line, and exits 0.
#
# The order. Most phases measure frames at rest in a stretch that must not
# cross a minute (the clock redraws), so before measuring they wait for a
# second of the minute in 2..H, their window (`while [ "$(date +%S)" -gt H ]`
# in the phase file; the first such wait is the phase's window). Before each
# phase the driver runs next the phase that would wait least at the second it
# is (`choose_next`): one whose window is open, the narrowest first, and a
# phase that never waits (no window) only when no window is open. The order
# therefore differs between runs; each phase prints `smoke: phase ID (second
# S)` as it starts, and the line before the last gives the whole order and the
# `--order` that repeats it. `--order` runs exactly the named phases in the
# named order. `--only` runs the named phases in the chosen order, and the last
# line is then `smoke: OK (only: ID,ID)` (or `smoke: OK (order: ID,ID)`)
# instead of `smoke: OK`. `--next-phase SECOND IDS` prints the phase the driver
# would run next at that second out of IDS, for the tests.
#
# Any other argument, an unknown, empty or repeated id, `--only` with
# `--order`, and a second outside 0..59 are usage errors (status 2);
# `--list`, `--next-phase` and the usage errors come before the lock.
#
# Before any phase, once per invocation: the artifact's other two binaries,
# `celestina-lock` and `celestina-lock-verify`, must sit beside the shell and
# refuse a bad request (CEL-2-H); the shell's `--check` must pass; and
# `cutover-config-probe.sh` runs on the two fixtures of
# `tests/fixtures/cutover/` (CEL-2-M): the niri config as the session ran it
# with Noctalia must fail it and the one with the design's §5.3 binds applied
# must pass it (no `~/.config` is read). Without niri the nested session is
# skipped with a notice and the smoke exits 0; without WAYLAND_DISPLAY or
# dbus-run-session it fails.
#
# Each phase runs as a child of this shell (`nest_child`) in an empty
# directory of its own, `$work/ID.N` for attempt N, and is told where things
# are through its environment (`SMOKE_PHASE_WORK`, `SMOKE_BIN`,
# `SMOKE_SCRIPTS`, `SMOKE_NIRI`, `CELESTINA_SMOKE_ATTEMPT`; see
# `smoke/common.sh`). A rest-frame check that fails while the host throttles
# the nest's window is inconclusive: the phase exits 75 and runs again in a
# new empty directory, up to three attempts per phase (`rest_fail` in
# `smoke/common.sh`; on the third it fails). Any other status of a phase ends
# the smoke with that status.
#
# Measuring runs take turns. The smoke takes the host-wide nest lock
# (`nest-run.sh`: `$XDG_RUNTIME_DIR/celestina-nest.lock`) before it
# does anything else and keeps it, every phase and attempt included, until it
# ends, so two smokes, or a smoke and `lock-rehearsal.sh`, never count frames
# at the same time. A second run waits `CELESTINA_NEST_LOCK_WAIT` seconds
# (default 1800) and then fails with status 69, naming the holder. TERM or INT
# ends the run together with the running phase and its nest (every nested run
# goes through `nest`, which keeps it in a process group of its own; the phase
# stops what it started itself, the fake Melibea of `phone` included), removes
# the work directory and exits 143 or 130. A caller that bounds the smoke uses
# TERM first and a limit that covers the wait plus the run (`timeout -k 60
# 2700 sh scripts/smoke.sh --binary PATH`), never `timeout -s KILL`.
#
# Exit statuses: 0 every selected phase passed (or niri is absent); 1 a check
# failed; 2 a usage error; 69 the lock stayed busy; 70 the lock's holder or
# this script vanished under a phase; 143 and 130 TERM and INT.
usage() {
    echo "usage: scripts/smoke.sh --binary PATH [--only ID[,ID...] | --order ID[,ID...]]" >&2
    echo "       scripts/smoke.sh --list" >&2
    echo "       scripts/smoke.sh --next-phase SECOND ID[,ID...]" >&2
    exit 2
}
# The phases; each is scripts/smoke/ID.sh. `--list` prints them in this order,
# which also settles the last tie in `choose_next`.
phases="notify launcher bar wallpaper energy glance clipboard connectivity osd phone media overview polkit cutover"
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ "$#" -eq 1 ] && [ "$1" = "--list" ]; then
    printf '%s\n' $phases
    exit 0
fi
# check_ids OPTION VALUE: the phase ids of VALUE, comma-separated, each a phase
# and named once, printed space-separated. Anything else is a usage error that
# names what is wrong (status 2; called in a command substitution, the caller
# exits with it).
check_ids() {
    case ",$2," in
        *,,*)
            echo "smoke: $1 '$2' names an empty phase id (scripts/smoke.sh --list prints them)" >&2
            exit 2
            ;;
    esac
    case $2 in
        *[!a-z,]*)
            echo "smoke: $1 '$2': a phase id is lowercase letters (scripts/smoke.sh --list prints them)" >&2
            exit 2
            ;;
    esac
    c_ids=
    for c_id in $(printf '%s\n' "$2" | tr ',' ' '); do
        case " $phases " in
            *" $c_id "*) ;;
            *)
                echo "smoke: $1: $c_id is not a phase (scripts/smoke.sh --list prints them)" >&2
                exit 2
                ;;
        esac
        case " $c_ids " in
            *" $c_id "*)
                echo "smoke: $1: $c_id is named twice" >&2
                exit 2
                ;;
        esac
        c_ids="$c_ids $c_id"
    done
    printf '%s\n' "$c_ids"
}
# phase_window ID: the H of the first minute wait in scripts/smoke/ID.sh
# (`while [ "$(date +%S)" -gt H ] || ...`), the last second at which the phase
# starts measuring; nothing for a phase that never waits for the minute.
phase_window() {
    sed -n 's/.*"\$(date +%S)" -gt \([0-9][0-9]*\) ].*/\1/p' "$here/smoke/$1.sh" | sed -n 1p
}
# window_wait SECOND H: the seconds a phase whose window ends at H waits from
# SECOND of the minute (it waits for a second in 2..H).
window_wait() {
    if [ "$1" -lt 2 ]; then
        echo $((2 - $1))
    elif [ "$1" -le "$2" ]; then
        echo 0
    else
        echo $((62 - $1))
    fi
}
# choose_next SECOND IDS: the phase of IDS (space-separated) to run at SECOND:
# the one that waits least for its window; on a tie the narrowest window, a
# phase with none counting as 60, so it only fills a gap; then the order of
# $phases.
choose_next() {
    cn_best=
    cn_key=
    for cn_id in $phases; do
        case " $2 " in
            *" $cn_id "*) ;;
            *) continue ;;
        esac
        cn_window=$(phase_window "$cn_id")
        if [ -n "$cn_window" ]; then
            cn_wait=$(window_wait "$1" "$cn_window")
        else
            cn_wait=0
            cn_window=60
        fi
        cn_this=$((cn_wait * 100 + cn_window))
        if [ -z "$cn_best" ] || [ "$cn_this" -lt "$cn_key" ]; then
            cn_best=$cn_id
            cn_key=$cn_this
        fi
    done
    printf '%s\n' "$cn_best"
}
# second_of TEXT: the second of the minute TEXT names, one or two digits
# (`date +%S` writes 05), or a usage error; a leading zero is dropped so the
# shell's arithmetic never reads 08 as octal.
second_of() {
    case $1 in
        '' | *[!0-9]* | ???*)
            echo "smoke: '$1' is not a second of the minute (0..59)" >&2
            exit 2
            ;;
    esac
    so_second=${1#0}
    so_second=${so_second:-0}
    [ "$so_second" -le 59 ] || {
        echo "smoke: '$1' is not a second of the minute (0..59)" >&2
        exit 2
    }
    echo "$so_second"
}
if [ "$#" -ge 1 ] && [ "$1" = "--next-phase" ]; then
    [ "$#" -eq 3 ] || usage
    np_second=$(second_of "$2") || exit 2
    np_ids=$(check_ids --next-phase "$3") || exit 2
    choose_next "$np_second" "$np_ids"
    exit 0
fi
bin=
bin_given=no
only=
only_given=no
order=
order_given=no
expect=
for arg do
    case $expect in
        binary) bin=$arg; expect=; continue ;;
        only) only=$arg; expect=; continue ;;
        order) order=$arg; expect=; continue ;;
    esac
    case $arg in
        --binary) [ "$bin_given" = no ] || usage; bin_given=yes; expect=binary ;;
        --only) [ "$only_given" = no ] || usage; only_given=yes; expect=only ;;
        --order) [ "$order_given" = no ] || usage; order_given=yes; expect=order ;;
        *) usage ;;
    esac
done
[ -z "$expect" ] && [ "$bin_given" = yes ] || usage
[ "$only_given" = no ] || [ "$order_given" = no ] || usage
# The selection: ids of phases, each once, none empty. Refused before the lock.
selected=
if [ "$only_given" = yes ]; then
    selected=$(check_ids --only "$only") || exit 2
fi
ordered=
if [ "$order_given" = yes ]; then
    ordered=$(check_ids --order "$order") || exit 2
fi
# shellcheck source=scripts/nest-run.sh
. "$here/nest-run.sh"
nest_lock_acquire "smoke.sh $*"
# Until the work directory exists, the lock is all there is to undo on the
# way out (EXIT; TERM and INT exit 143 and 130).
nest_signals nest_lock_release
[ -x "$bin" ] || { echo "smoke: $bin is not an executable" >&2; exit 1; }
case "$bin" in
    /*) ;;
    *) bin=$(pwd)/$bin ;;
esac

# The artifact is three binaries (CEL-2-H): the shell finds the lock and its
# verifier beside itself, so a smoke of a lone shell proves nothing about the
# lock it would spawn. Assertions only: neither is started against a
# compositor here. A bad argument answers 2 from the lock and 6 from the
# verifier before either touches Wayland, PAM or stdin, which shows the right
# programs are there and speak their contracts.
bin_dir=$(dirname -- "$bin")
for lock_binary in celestina-lock celestina-lock-verify; do
    [ -x "$bin_dir/$lock_binary" ] || {
        echo "smoke: $bin_dir/$lock_binary is missing or not an executable" >&2
        exit 1
    }
done
set +e
"$bin_dir/celestina-lock" --not-an-option >/dev/null 2>&1
lock_status=$?
"$bin_dir/celestina-lock-verify" --protocol 0 --service production </dev/null >/dev/null 2>&1
verify_status=$?
set -e
[ "$lock_status" -eq 2 ] || {
    echo "smoke: celestina-lock answered $lock_status to a bad option, not 2" >&2
    exit 1
}
[ "$verify_status" -eq 6 ] || {
    echo "smoke: celestina-lock-verify answered $verify_status to a bad protocol, not 6" >&2
    exit 1
}
echo "smoke: celestina-lock and celestina-lock-verify are beside the shell and refuse a bad request"

if ! "$bin" --check; then
    echo "smoke: $bin --check failed" >&2
    exit 1
fi

# The cutover's config probe (CEL-2-M) on its two fixtures: a copy of a niri
# config that still starts Noctalia must fail it (exit 1) and one with the
# design's §5.3 binds applied must pass it. The probe runs `niri validate` only
# when a niri is installed and never reads `~/.config`; nothing here needs the
# nest.
cutover_fixtures=$here/../tests/fixtures/cutover
before_status=0
sh "$here/cutover-config-probe.sh" "$cutover_fixtures/config-before.kdl" >/dev/null 2>&1 || before_status=$?
after_status=0
after_words=$(sh "$here/cutover-config-probe.sh" "$cutover_fixtures/config-after.kdl" 2>&1) || after_status=$?
[ "$before_status" -eq 1 ] || {
    echo "smoke: cutover-config-probe.sh answered $before_status to the config before the cutover, not 1" >&2
    exit 1
}
[ "$after_status" -eq 0 ] || {
    echo "smoke: cutover-config-probe.sh refused the config after the cutover:" >&2
    printf '%s\n' "$after_words" >&2
    exit 1
}
echo "smoke: cutover-config-probe.sh fails the config before the cutover and passes the one after"

if ! command -v niri >/dev/null 2>&1; then
    echo "smoke: niri is absent; the nested session was not run"
    exit 0
fi
if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "smoke: niri is installed but WAYLAND_DISPLAY is empty: the nested" >&2
    echo "  niri needs a host compositor to open its window in" >&2
    exit 1
fi
if ! command -v dbus-run-session >/dev/null 2>&1; then
    echo "smoke: dbus-run-session is absent; the nest needs a bus of its own" >&2
    exit 1
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/celestina-smoke.XXXXXX")
# The one cleanup, run once on every way out (EXIT; TERM and INT exit 143 and
# 130 after stopping the running phase, which first stops its nest and what
# it started itself): the lock, the work directory.
cleanup() {
    nest_lock_release
    rm -rf "$work"
}
nest_signals cleanup
# The phases' generated wrappers quote paths with single quotes, so a path
# that holds one cannot be written safely: refuse it rather than build a
# script that means something else.
case "$work" in
    *\'*)
        echo "smoke: the temporary directory $work contains a single quote;" >&2
        echo "  set TMPDIR to a path without one" >&2
        exit 1
        ;;
esac
# The niri the nest runs, resolved as dev-session.sh resolves it, so a probe
# that talks to the nest (`overlay` in the phone phase, the cutover's layers)
# uses the same binary and the same protocol as the nest and never the PATH's.
# The phases get it as SMOKE_NIRI.
niri_binary=${CELESTINA_NEST_NIRI:-}
if [ -z "$niri_binary" ]; then
    patched=${CELESTINA_NIRI_PREFIX:-$HOME/.local/lib/celestina}/niri
    if [ -x "$patched" ]; then
        niri_binary=$patched
    else
        niri_binary=niri
    fi
fi
case "$niri_binary" in
    *\'*)
        echo "smoke: the niri path $niri_binary contains a single quote" >&2
        exit 1
        ;;
esac
# run_phase ID: scripts/smoke/ID.sh as a child of this shell (`nest_child`:
# TERM or INT here ends it, and it ends its own nest first), in a new empty
# directory for each attempt. Status 0 is a pass. Status 75 is an
# inconclusive rest-frame check (`rest_fail`): the phase runs again, up to
# three attempts; on the third `rest_fail` fails instead, and a 75 that came
# from anything else fails here with status 1. Any other status ends the smoke
# with that status.
run_phase() {
    for attempt in 1 2 3; do
        mkdir "$work/$1.$attempt"
        phase_status=0
        nest_child env CELESTINA_SMOKE_ATTEMPT=$attempt SMOKE_PHASE=$1 SMOKE_PHASE_WORK="$work/$1.$attempt" \
            SMOKE_BIN="$bin" SMOKE_SCRIPTS="$here" SMOKE_NIRI="$niri_binary" sh "$here/smoke/$1.sh" ||
            phase_status=$?
        [ "$phase_status" -ne 0 ] || return 0
        [ "$phase_status" -eq 75 ] || exit "$phase_status"
        if [ "$attempt" -ge 3 ]; then
            echo "smoke: $1: the nest was throttled by the host in 3 attempts; show its window" >&2
            exit 1
        fi
        echo "smoke: $1: the rest-frame check was inconclusive; starting attempt $((attempt + 1))" >&2
    done
}
# The run: with --order, the named phases in that order; otherwise every
# selected phase, the next one chosen at the second it is (`choose_next`).
ran=
if [ "$order_given" = yes ]; then
    for id in $ordered; do
        echo "smoke: phase $id"
        run_phase "$id"
        ran=$ran${ran:+,}$id
    done
else
    remaining=${selected:-$phases}
    while [ -n "$(echo $remaining)" ]; do
        second=$(date +%S)
        second=${second#0}
        second=${second:-0}
        id=$(choose_next "$second" "$remaining")
        echo "smoke: phase $id (second $second)"
        run_phase "$id"
        ran=$ran${ran:+,}$id
        rest=
        for left in $remaining; do
            [ "$left" = "$id" ] || rest="$rest $left"
        done
        remaining=$rest
    done
fi
echo "smoke: the phases ran in the order $ran (scripts/smoke.sh --order $ran repeats it)"
if [ "$only_given" = yes ]; then
    echo "smoke: OK (only: $ran)"
elif [ "$order_given" = yes ]; then
    echo "smoke: OK (order: $ran)"
else
    echo "smoke: OK"
fi
