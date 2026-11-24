# nest-run.sh: what the scripts that run a nested niri share (POSIX sh, meant
# to be sourced: `. "$here/nest-run.sh"`). It defines functions and a few
# `nr_` variables and runs nothing. Two jobs.
#
# 1. One host-wide lock for measuring runs (`nest_lock_acquire WHO`).
#
# `smoke.sh` judges frame counts "at rest" in about twenty nests, each a window
# in the author's live session, and every landing runs it. Two measuring runs
# at once (several agent sessions did, on 2026-11-24) disturb each other's
# counts and prove nothing, so they take turns: `smoke.sh`,
# `wallpaper-backdrop-probe.sh` and `lock-rehearsal.sh` take the lock in their
# outermost invocation, before any nest and before any expensive step.
# `dev-session.sh` takes none (an interactive nest stays open for hours and
# must never block a landing); it only says so when a run is measuring
# (`nest_lock_notice`). `smoke-media.sh` is a client inside a nest: no lock.
#
#   - The lock is `$XDG_RUNTIME_DIR/celestina-nest.lock`. Without a private
#     runtime directory (set, a directory, owned by the user) the run refuses
#     with status 1: a fallback in the shared /tmp has predictable names that
#     another user could occupy. Beside it `.owner` holds one line about the holder:
#     the pid of the script, WHO (its name and what it was given) and the
#     start time. The holder removes it when it ends.
#   - The script never holds the lock on a descriptor of its own: every nested
#     niri, bus daemon and client it starts would inherit it, and one stray
#     survivor would keep the lock for ever. A background holder takes it
#     instead (`flock(1)` opens the file itself, so only the holder has it
#     open). The lock must last as long as ANY process of the run, not only
#     the script's pid (a `kill -9` of the script, or `timeout -s KILL` around
#     it, leaves the attempt and its nest measuring), so the holder keeps it
#     while the script's pid is alive OR a process recorded in the hold
#     directory is: `nest_child` records an attempt's pid (`att.PID`), `nest`
#     the pid of its `timeout` (`grp.PID`; that is the nest's process group,
#     and a group counts as alive while `kill -0 -- -PID` finds a member, so
#     a stray client of the nest keeps the lock too, which a pid check would
#     miss). Each removes its record when the process has ended. The holder
#     ignores TERM, INT and HUP and runs in a session of its own (`setsid`,
#     util-linux like `flock`), so neither a signal to the whole process group
#     of a run nor a KILL to it (`timeout -s KILL`, a harness's kill) frees
#     the lock while a recorded nest is still up; it ends through its own
#     loop, through `nest_lock_release`, or through a KILL aimed at the holder
#     itself, which nothing here can survive (nor the machine going down, nor
#     a client that left its nest's process group with `setsid`: it is
#     outside the registry). When
#     it ends it removes the owner file and the hold directory. A zombie
#     nobody has reaped counts as dead (`kill -0` would say alive), so
#     liveness of a pid is read from /proc: this is for Linux.
#   - The hold directory (`$XDG_RUNTIME_DIR/celestina-nest-hold.XXXXXX`) is
#     the run's registry and its identity: `CELESTINA_NEST_LOCK_HELD` carries
#     its path, exported to everything the run starts. A script started by a
#     holder (`smoke.sh` runs itself again for each attempt) must not wait for
#     its own parent, so `nest_lock_acquire` returns at once when the marker
#     names a directory whose `ready` file exists and whose holder (the pid in
#     `holder`) is alive; any other value (a leftover, a foreign `1`, a path
#     that is gone) is ignored and the lock is taken in the normal way.
#     `nest` and `nest_child` refuse to start anything (status 70, the script
#     ends through its cleanup) when the holder is gone or when the script that
#     took the lock (`owner`) is dead: an orphaned attempt stops after the nest
#     it is in instead of running the rest of the smoke unsupervised. The
#     marker rides the environment through `dev-session.sh`, `dbus-run-session`
#     and niri, but the only script that starts itself inside a nest,
#     `lock-rehearsal.sh`, takes the lock after the branch those inner
#     invocations run, so it does not depend on it. What a KILLed run can
#     still leave behind is its own work directory (`$work` of the smoke):
#     nobody is left to remove it. A stale hold directory (holder gone, or a
#     waiter killed before it had a holder) is removed by the next
#     `nest_lock_acquire`.
#   - The patience is `CELESTINA_NEST_LOCK_WAIT` seconds, default 1800 (a full
#     smoke takes four to five minutes and a landing runs one). `0` fails at
#     once. A value that is not a whole number is a usage error (status 2).
#     While waiting, one line on stderr says who holds the lock. When the wait
#     runs out the run fails naming the holder with status 69 (`smoke.sh`
#     already gives 75 the meaning "inconclusive, try again", and 1 and 2 are
#     failure and usage; 69 is EX_UNAVAILABLE). A missing `flock` fails with
#     status 1, a missing `setsid` too.
#   - `nest_lock_release` ends the holder at once (`stop` in the hold
#     directory, KILL if it has not ended after three seconds); the callers'
#     cleanup calls it so the lock is free the moment a run ends.
#   - A caller that bounds a measuring run with `timeout` must use TERM first
#     and a limit that covers the wait plus the run (a full smoke is four to
#     five minutes, up to three attempts): `timeout -k 60 2700 sh
#     scripts/smoke.sh --binary PATH`, or a lower CELESTINA_NEST_LOCK_WAIT.
#     Never `timeout -s KILL`: KILL cannot be handled, so the nest would
#     outlive it and its work directory stay (the registry above still keeps
#     the lock until that nest is gone). `flock`
#     does not serve waiters in arrival order, so a run that waits may be
#     passed over by a later one until its patience ends.
#
# 2. TERM and INT end a run together with its nest (`nest_signals`, `nest`).
#
# A POSIX shell defers a trap until its foreground command ends, so a smoke
# that waited on a nest in the foreground ran on after TERM. `nest` therefore
# runs the nest in the background and waits for it: the wait is interrupted by
# the signal and the trap runs while the nest is still up. The nest is
# `timeout LIMIT env ...`: GNU `timeout` calls `setpgid`, so it and everything
# below it (niri, the bus daemon, the clients) form one process group whose id
# is the pid of `timeout`, and `kill -s TERM -- -PID` reaches all of them. The
# trap waits for `timeout` to end and sends KILL to the group after five
# seconds. A background command of a non-interactive shell has no stdin
# (/dev/null) and ignores INT, so a script started that way cannot trap INT:
# an attempt run by `nest_child` is therefore told TERM, never INT. For the
# same reason a run that was itself started with INT ignored (a background job,
# `nohup`, some CI runners) cannot be ended by INT at all, only by TERM. The ignored
# INT and QUIT are inherited by everything inside the nest (niri, the shell,
# the wrappers); nothing in the suite sends INT inside a nest.


# A copy of the script's own stderr, for messages that must outlive the
# redirections of a `nest` call (see nr_check_held).
if ( : >&2 ) 2> /dev/null; then
    exec 9>&2
fi

nr_pid=
nr_group=no
nr_rec=
nr_cleanup=
nr_cleaned=no
nr_lock_dir=
nr_lock_pid=

# nr_alive PID: the process exists and is not a zombie waiting to be reaped.
nr_alive() {
    [ -n "$1" ] || return 1
    [ -r "/proc/$1/status" ] || return 1
    case $(sed -n 's/^State:[[:space:]]*\(.\).*/\1/p' "/proc/$1/status" 2>/dev/null) in
        '' | Z) return 1 ;;
    esac
    return 0
}

# nr_file_pid FILE: the pid a small file holds, or nothing.
nr_file_pid() {
    nr_fp=
    [ -r "$1" ] && IFS= read -r nr_fp < "$1" || true
    case $nr_fp in
        *[!0-9]*) nr_fp= ;;
    esac
    printf '%s\n' "$nr_fp"
}

# nr_paths: where the lock lives (nr_dir, nr_lock).
nr_paths() {
    [ -n "${XDG_RUNTIME_DIR:-}" ] && [ -d "$XDG_RUNTIME_DIR" ] && [ -O "$XDG_RUNTIME_DIR" ] || return 1
    nr_dir=$XDG_RUNTIME_DIR
    nr_lock=$nr_dir/celestina-nest.lock
}

# nr_live_hold: a hold directory whose holder is alive, or fail.
nr_live_hold() {
    for nr_d in "$nr_dir"/celestina-nest-hold.*; do
        [ -d "$nr_d" ] && [ ! -L "$nr_d" ] || continue
        [ -e "$nr_d/ready" ] || continue
        if nr_alive "$(nr_file_pid "$nr_d/holder")"; then
            printf '%s\n' "$nr_d"
            return 0
        fi
    done
    return 1
}

# nr_owner: the owner line of the holder, or fail. When the script is gone but
# its attempt or nest still runs (the lock is still held) the line says so.
nr_owner() {
    nr_paths || return 1
    nr_file=$nr_lock.owner
    [ -r "$nr_file" ] || return 1
    nr_line=
    IFS= read -r nr_line < "$nr_file" || [ -n "$nr_line" ] || return 1
    nr_owner_pid=${nr_line%% *}
    case $nr_owner_pid in
        '' | *[!0-9]*) return 1 ;;
    esac
    if nr_alive "$nr_owner_pid"; then
        printf '%s\n' "$nr_line"
        return 0
    fi
    nr_gone=$(nr_live_hold) || return 1
    printf '%s (script gone; its attempt or nest still runs, hold directory %s)\n' "$nr_line" "$nr_gone"
}

# nest_lock_notice: one notice on stderr when a measuring run holds the lock.
# For an interactive nest, which takes no lock.
nest_lock_notice() {
    nr_held=$(nr_owner) || return 0
    echo ">> a measuring run is in progress ($nr_held); this nest's window may disturb its frame counts" >&2
}

# nr_marker_valid: CELESTINA_NEST_LOCK_HELD names a live hold directory.
nr_marker_valid() {
    case ${CELESTINA_NEST_LOCK_HELD:-} in
        /*) ;;
        *) return 1 ;;
    esac
    [ -e "$CELESTINA_NEST_LOCK_HELD/ready" ] || return 1
    nr_alive "$(nr_file_pid "$CELESTINA_NEST_LOCK_HELD/holder")"
}

# nr_check_held: under a lock, the holder and the script that took it must
# still be alive, or nothing new may start.
nr_check_held() {
    [ -n "${CELESTINA_NEST_LOCK_HELD:-}" ] || return 0
    nr_marker_valid && nr_alive "$(nr_file_pid "$CELESTINA_NEST_LOCK_HELD/owner")" && return 0
    nr_msg="nest-run: the nest lock is gone (its holder or the script that took it has ended); not starting another nest"
    # The caller's redirections of a `nest` call apply here, and a smoke's
    # point at a log its cleanup deletes: the message goes to the copy of the
    # script's own stderr taken when this file was sourced.
    { echo "$nr_msg" >&9; } 2> /dev/null || echo "$nr_msg" >&2
    exit 70
}

# nest_check: for a caller that starts its own nest (lock-rehearsal.sh).
nest_check() {
    nr_check_held
}

# nest_register PID, nest_unregister PID: record a nest group the caller
# started itself (PID leads its process group) in the registry, and drop it.
nest_register() {
    if [ -n "${CELESTINA_NEST_LOCK_HELD:-}" ] && [ -d "$CELESTINA_NEST_LOCK_HELD" ]; then
        printf '%s\n' "$1" > "$CELESTINA_NEST_LOCK_HELD/grp.$1" 2> /dev/null || true
    fi
}

nest_unregister() {
    [ -z "${CELESTINA_NEST_LOCK_HELD:-}" ] || rm -f "$CELESTINA_NEST_LOCK_HELD/grp.$1"
}

# nr_launch NAME COMMAND...: start the command in the background (nr_pid) and,
# under a lock, register it in the hold directory as NAME.PID; the child
# writes its own record before it execs, so there is no instant in which a
# nest runs unregistered.
nr_launch() {
    nr_name=$1
    shift
    if [ -n "${CELESTINA_NEST_LOCK_HELD:-}" ] && [ -d "$CELESTINA_NEST_LOCK_HELD" ]; then
        sh -c 'printf "%s\n" "$$" > "$1/$2.$$" 2> /dev/null || :; shift 2; exec "$@"' \
            nest-launch "$CELESTINA_NEST_LOCK_HELD" "$nr_name" "$@" &
        nr_pid=$!
        nr_rec=$CELESTINA_NEST_LOCK_HELD/$nr_name.$nr_pid
    else
        "$@" &
        nr_pid=$!
        nr_rec=
    fi
}

nr_unrecord() {
    [ -z "$nr_rec" ] || rm -f "$nr_rec"
    nr_rec=
}

# nr_abort_wait CODE: a signal arrived while waiting for the lock: stop asking
# for it (a holder that already has it sees its directory go) and leave.
nr_abort_wait() {
    kill -s TERM "$nr_holder" 2> /dev/null || true
    rm -rf "$nr_hold"
    exit "$1"
}

# nr_sweep: remove hold directories of runs that can no longer exist: the
# holder is gone, or there never was one and the waiter that made it is dead
# (killed while waiting). Symlinks and directories of other users are left.
nr_sweep() {
    for nr_d in "$nr_dir"/celestina-nest-hold.*; do
        [ -d "$nr_d" ] && [ ! -L "$nr_d" ] && [ -O "$nr_d" ] || continue
        if [ -e "$nr_d/holder" ]; then
            nr_alive "$(nr_file_pid "$nr_d/holder")" || rm -rf "$nr_d"
        else
            nr_alive "$(nr_file_pid "$nr_d/owner")" || rm -rf "$nr_d"
        fi
    done
}

# nest_lock_acquire WHO: take the lock or exit.
nest_lock_acquire() {
    if nr_marker_valid; then
        return 0
    fi
    unset CELESTINA_NEST_LOCK_HELD
    nr_who=$1
    nr_wait=${CELESTINA_NEST_LOCK_WAIT:-1800}
    case $nr_wait in
        *[!0-9]*)
            echo "nest-run: CELESTINA_NEST_LOCK_WAIT is a whole number of seconds, not '$nr_wait'" >&2
            exit 2
            ;;
    esac
    if ! command -v flock > /dev/null 2>&1; then
        echo "nest-run: flock (util-linux) is required to take the nest lock" >&2
        exit 1
    fi
    if ! command -v setsid > /dev/null 2>&1; then
        echo "nest-run: setsid (util-linux) is required to take the nest lock" >&2
        exit 1
    fi
    if ! nr_paths; then
        echo "nest-run: XDG_RUNTIME_DIR must name a directory of your own (the nest lock lives there)" >&2
        exit 1
    fi
    nr_sweep
    nr_hold=$(mktemp -d "$nr_dir/celestina-nest-hold.XXXXXX") || {
        echo "nest-run: cannot make a directory in $nr_dir" >&2
        exit 1
    }
    printf '%s\n' "$$" > "$nr_hold/owner"
    nr_line="$$ $nr_who $(date)"
    # The holder: flock(1) takes the lock (exit 200 when the wait runs out),
    # then the shell, which ignores TERM, INT and HUP, writes its pid, the
    # owner line and `ready`, and stays while the script, an attempt or a nest
    # group recorded in its directory lives, until `stop` appears or the
    # directory goes. Then it removes the owner file and the directory.
    setsid -w flock -E 200 -w "$nr_wait" "$nr_lock" sh -c '
        trap "" TERM INT HUP
        [ -d "$3" ] || exit 0
        alive() {
            case $(sed -n "s/^State:[[:space:]]*\(.\).*/\1/p" "/proc/$1/status" 2>/dev/null) in
                "" | Z) return 1 ;;
            esac
        }
        busy() {
            alive "$4" && return 0
            for r in "$3"/att.*; do
                [ -e "$r" ] || continue
                alive "${r##*.}" && return 0
            done
            for r in "$3"/grp.*; do
                [ -e "$r" ] || continue
                kill -0 -- "-${r##*.}" 2>/dev/null && return 0
                alive "${r##*.}" && return 0
            done
            return 1
        }
        alive "$4" || exit 0
        printf "%s\n" "$$" > "$3/holder.part" && mv "$3/holder.part" "$3/holder"
        printf "%s\n" "$1" > "$2" || exit 1
        : > "$3/ready"
        while [ ! -e "$3/stop" ] && [ -d "$3" ] && busy "$@"; do sleep 1; done
        rm -f "$2"
        rm -rf "$3"
    ' nest-lock-holder "$nr_line" "$nr_lock.owner" "$nr_hold" "$$" \
        > /dev/null 2> "$nr_hold/err" < /dev/null &
    nr_holder=$!
    trap 'nr_abort_wait 143' TERM
    trap 'nr_abort_wait 130' INT
    nr_polls=0
    while [ ! -e "$nr_hold/ready" ]; do
        if ! nr_alive "$nr_holder"; then
            nr_rc=0
            wait "$nr_holder" || nr_rc=$?
            if [ "$nr_rc" -eq 200 ]; then
                nr_held=$(nr_owner) || nr_held=unknown
                if [ "$nr_wait" -eq 0 ]; then
                    echo "nest-run: the nest lock is held by: $nr_held (CELESTINA_NEST_LOCK_WAIT=0: not waiting)" >&2
                else
                    echo "nest-run: the nest lock is held by: $nr_held; gave up after ${nr_wait}s (CELESTINA_NEST_LOCK_WAIT)" >&2
                fi
                rm -rf "$nr_hold"
                exit 69
            fi
            if [ ! -e "$nr_hold/ready" ]; then
                echo "nest-run: could not take the nest lock $nr_lock (flock status $nr_rc)" >&2
                cat "$nr_hold/err" >&2 2> /dev/null || true
                rm -rf "$nr_hold"
                exit 1
            fi
            break
        fi
        nr_polls=$((nr_polls + 1))
        if [ "$nr_polls" -eq 5 ]; then
            nr_held=$(nr_owner) || nr_held=unknown
            echo "nest-run: waiting for the nest lock, held by: $nr_held (at most ${nr_wait}s, CELESTINA_NEST_LOCK_WAIT)" >&2
        fi
        sleep 0.1
    done
    trap - TERM INT
    nr_lock_dir=$nr_hold
    nr_lock_pid=$(nr_file_pid "$nr_hold/holder")
    CELESTINA_NEST_LOCK_HELD=$nr_hold
    export CELESTINA_NEST_LOCK_HELD
}

# nest_lock_release: end the holder now (when this script took the lock).
nest_lock_release() {
    [ -n "$nr_lock_dir" ] || return 0
    touch "$nr_lock_dir/stop" 2> /dev/null || true
    nr_ticks=0
    while nr_alive "$nr_lock_pid" && [ "$nr_ticks" -lt 30 ]; do
        sleep 0.1
        nr_ticks=$((nr_ticks + 1))
    done
    if nr_alive "$nr_lock_pid"; then
        kill -s KILL "$nr_lock_pid" 2> /dev/null || true
    fi
    wait "$nr_holder" 2> /dev/null || true
    rm -rf "$nr_lock_dir"
    nr_lock_dir=
    nr_lock_pid=
    unset CELESTINA_NEST_LOCK_HELD
}

# nr_stop: end the background command `nest` or `nest_child` is waiting for:
# TERM (to the whole group for a nest), KILL when it has not ended after the
# grace (five seconds for a nest, fifteen for an attempt that must stop its
# own nest first), then reap it.
nr_stop() {
    [ -n "$nr_pid" ] || return 0
    if [ "$nr_group" = yes ]; then
        nr_target=-$nr_pid
        nr_grace=50
    else
        nr_target=$nr_pid
        nr_grace=150
    fi
    kill -s TERM -- "$nr_target" 2> /dev/null || true
    nr_ticks=0
    while nr_alive "$nr_pid" && [ "$nr_ticks" -lt "$nr_grace" ]; do
        sleep 0.1
        nr_ticks=$((nr_ticks + 1))
    done
    if nr_alive "$nr_pid"; then
        kill -s KILL -- "$nr_target" 2> /dev/null || true
    fi
    wait "$nr_pid" 2> /dev/null || true
    nr_unrecord
    nr_pid=
}

# nr_wait_child: wait for the background command; return its status.
nr_wait_child() {
    nr_status=0
    wait "$nr_pid" || nr_status=$?
    nr_unrecord
    nr_pid=
    return "$nr_status"
}

# nest LIMIT [VAR=VALUE...] COMMAND [ARG...]: `timeout LIMIT env ...` in the
# background (a process group of its own), waited for; returns its status.
nest() {
    nr_check_held
    nr_limit=$1
    shift
    nr_launch grp timeout "$nr_limit" env "$@"
    nr_group=yes
    nr_wait_child
}

# nest_child COMMAND [ARG...]: a command that is not itself a nest (the smoke's
# attempts), in the background; TERM or INT sends it TERM, and the caller then
# exits 143 or 130 after it has ended.
nest_child() {
    nr_check_held
    nr_launch att "$@"
    nr_group=no
    nr_wait_child
}

nr_on_signal() {
    nr_stop
    exit "$1"
}

nr_on_exit() {
    nr_exit=$?
    trap '' INT TERM
    nr_stop
    if [ "$nr_cleaned" = no ]; then
        nr_cleaned=yes
        "$nr_cleanup" || true
    fi
    exit "$nr_exit"
}

# nest_signals CLEANUP: EXIT runs the function CLEANUP once; TERM and INT stop
# the running nest and exit 143 and 130 (which runs CLEANUP through EXIT).
nest_signals() {
    nr_cleanup=$1
    trap nr_on_exit EXIT
    trap 'nr_on_signal 143' TERM
    trap 'nr_on_signal 130' INT
}
