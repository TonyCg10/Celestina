#!/bin/sh
# lock-rehearsal.sh: steps R-1 to R-10 of the session lock's rehearsal
# (CEL-2-H design §9.2), in a nested niri and nowhere else.
#
#   lock-rehearsal.sh --bin-dir DIR --fake-logind PATH --out DIR
#                     [--unlock-probe PATH] [--pointer-tool PATH]
#                     [--wallpaper-folder DIR] [--monitor NAME|none]
#                     [--check] [R-1 ... R-10 | R-9a R-9b R-9c]
#
#   --bin-dir        directory holding the release `celestina`,
#                    `celestina-lock` and `celestina-lock-verify` (the shell
#                    finds the other two beside itself, as installed)
#   --fake-logind    the `fake_logind` example (examples/fake_logind.rs)
#   --unlock-probe   the `unlock_probe` binary built with `--features
#                    fake-verifier` (R-8 only; never shipped)
#   --pointer-tool   a program that, run with the nest's WAYLAND_DISPLAY,
#                    connects and creates a virtual pointer at once, then
#                    waits for one line on its standard input, sends a few
#                    motions and exits (R-9b: input that ends the idle period
#                    must stop the chain; a virtual keyboard does not count as
#                    activity for the compositor, a pointer does)
#   --wallpaper-folder  R-10 only: a directory holding the pictures the nest's
#                    wallpaper duty paints (and so the lock's thumbnail). Without
#                    it R-10 generates a bright 960x540 PNG with python3
#   --monitor        R-10 only: the host output the nest's window is placed
#                    fullscreen on, as `dev-session.sh --monitor` does (default
#                    DP-1, the 4K output; `none` leaves the window where the
#                    host puts it and R-10 then records the size it measured).
#                    R-10 always runs the nest at scale 1.5. Every other step
#                    keeps `--monitor none` and never touches the host's IPC
#   --out            where the evidence goes (captures, logs, summary)
#   --check          run the refusals and the preparation, print the plan and
#                    stop: nothing is started
#
# Each step runs in a nest of its own (`dev-session.sh --own-bus --once`) on a
# private session bus, with the system bus pointed at the fake logind of
# examples/fake_logind.rs on a private bus of its own, so no real inhibitor is
# taken and nothing is ever suspended. The only PAM service a rehearsal can
# reach is `celestina-lock-rehearsal`, which must NOT exist: PAM then falls to
# `other` and denies, outside `faillock`. The author's password is never typed
# and the production service is never reached with a wrong one.
#
# R-10 (the backdrop and the clock, H-b) locks a nest whose shell runs the
# wallpaper duty over a real picture, so the bundle carries a thumbnail; it
# captures the dimmed backdrop with the clock and checks that the clock
# redraws at the minute. The lock exposes no frame time (no `--metrics`, no
# log line), so R-10 does NOT report painter p50/p99: it records, as a NOTE,
# the wall-clock time from a key press (`wtype` against the nest) to the first
# `grim` capture that differs, an upper bound that includes `wtype`, the
# nest's compositor and the capture itself. See `step_r10`.
#
# Refused at the start, with nothing started:
#   - /etc/pam.d/celestina-lock-rehearsal exists (read-only test);
#   - any $XDG_RUNTIME_DIR/celestina/lock-*.state names a live process (a lock
#     of the live session; the nest shares the runtime directory);
#   - the host has no WAYLAND_DISPLAY to open the nest's window in.
# Inside the nest every action first checks that the display is not the
# host's and that its listener is the nest's own niri.
#
# NO process is ever stopped by name. The shell, the locks and `swaylock` are
# stopped by the pid their own launcher recorded (the lock's pid file, `$!`),
# and a nest that does not end by itself is stopped by the pid read from its
# own NIRI_SOCKET after checking that this process was started with this
# run's generated .kdl.

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
self=$here/$(basename -- "$0")

now_ms() { date +%s%3N; }

# ── inside the nest ──────────────────────────────────────────────────────────
# `dev-session.sh` starts this same file twice there: as the "binary"
# (`--nest-shell`, which keeps the nest alive and runs the shell as asked) and
# as the "probe" (no argument, which drives the step).

if [ "${LOCK_REHEARSAL_INSIDE:-}" = 1 ]; then
    set -u
    work=$LOCK_REHEARSAL_WORK
    ctl=$work/ctl
    bin_dir=$LOCK_REHEARSAL_BIN_DIR
    steps_log=$work/steps.log
    err=$work/run.err
    nest_log=$work/nest.log
    prefix=$LOCK_REHEARSAL_PREFIX
    logind_log=$work/logind.log

    say() {
        printf '%s %s\n' "$(now_ms)" "$*" >> "$steps_log"
    }

    # The nest, or nothing: the display is not the host's, its listener is the
    # niri named by NIRI_SOCKET, and that niri was started with this run's
    # generated configuration.
    guard() {
        case "${WAYLAND_DISPLAY:-}" in
            ''|"$LOCK_REHEARSAL_HOST_DISPLAY") say "ABORT: display is '${WAYLAND_DISPLAY:-}'"; return 1 ;;
        esac
        sock=$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY
        want=$(printf '%s' "${NIRI_SOCKET:-}" | sed -n 's/.*niri\.[^.]*\.\([0-9]*\)\.sock$/\1/p')
        have=$(ss -xlp 2>/dev/null | grep -F " $sock " | sed -n 's/.*pid=\([0-9]*\),.*/\1/p' | head -n 1)
        if [ -z "$want" ] || [ "$want" != "$have" ]; then
            say "ABORT: listener of $WAYLAND_DISPLAY is '$have', NIRI_SOCKET says '$want'"
            return 1
        fi
        if ! tr '\0' ' ' < "/proc/$want/cmdline" 2>/dev/null | grep -qF -- "$work/run.kdl"; then
            say "ABORT: niri $want was not started with this run's configuration"
            return 1
        fi
        return 0
    }

    if [ "${1:-}" = "--nest-shell" ]; then
        # The "binary" of the nest. Without the shell it only holds the nest
        # open; with it, it runs the shell and starts it again on request
        # (R-5). The nest ends when this returns.
        guard || exit 3
        while :; do
            if [ "${LOCK_REHEARSAL_SHELL:-0}" = 1 ]; then
                "$bin_dir/celestina"
                say "shell exited status $?"
                : > "$ctl/shell-exited"
                until [ -e "$ctl/restart" ] || [ -e "$ctl/finish" ]; do sleep 0.2; done
            else
                until [ -e "$ctl/finish" ]; do sleep 0.2; done
            fi
            [ ! -e "$ctl/finish" ] || break
            rm -f "$ctl/restart" "$ctl/shell-exited"
            say "shell started again"
        done
        exit 0
    fi

    # ── the probe: helpers ───────────────────────────────────────────────────

    runtime_celestina=$XDG_RUNTIME_DIR/celestina
    state_file=$runtime_celestina/lock-$WAYLAND_DISPLAY.state
    pids=$ctl/pids

    # Records a pid this run started, so the host can stop it if the probe
    # dies before it does.
    track() { printf '%s %s\n' "$1" "$2" >> "$pids"; }

    # The nest's own clients: a clean environment, the nest's display.
    nestenv() {
        env -i WAYLAND_DISPLAY="$WAYLAND_DISPLAY" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" PATH=/usr/bin "$@"
    }

    # The same, for a background process: `exec` makes the subshell the
    # program, so `$!` is the program's own pid, the one that is recorded and
    # later stopped.
    nest_bg() {
        exec env -i WAYLAND_DISPLAY="$WAYLAND_DISPLAY" XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" PATH=/usr/bin "$@"
    }

    shot() {
        guard || return 1
        if nestenv grim "$prefix-$1.png" 2>> "$steps_log"; then
            say "capture $prefix-$1.png"
        else
            say "capture $1 FAILED"
        fi
    }

    result() { # result STEP PASS|FAIL|NOTE text
        say "RESULT $1 $2 $3"
        printf 'RESULT %s %s %s\n' "$1" "$2" "$3"
    }

    count() { grep -cF -- "$2" "$1" 2>/dev/null || true; }
    # Whole lines (a status line `locked` is not part of `unlocked`).
    # Suspend calls the fake logind served (not its CanSuspend probes).
    suspend_calls() { grep -cE '^[0-9]+ Suspend ' "$logind_log" 2>/dev/null || true; }
    count_line() { grep -cxF -- "$2" "$1" 2>/dev/null || true; }
    # niri's "locking session" is not part of its "unlocking session".
    count_locking() { grep -cE '[^n]locking session' "$1" 2>/dev/null || true; }

    # wait_count FILE PATTERN N SECONDS: until FILE has N lines with PATTERN.
    wait_count() {
        i=0
        limit=$(($4 * 20))
        while [ "$i" -lt "$limit" ]; do
            [ "$(count "$1" "$2")" -ge "$3" ] && return 0
            sleep 0.05
            i=$((i + 1))
        done
        return 1
    }

    # The epoch milliseconds of the Nth line of the shell's journal that has
    # PATTERN (tracing prints an RFC 3339 stamp first).
    ts_of() { # ts_of PATTERN N
        line=$(grep -F -- "$1" "$err" | sed -n "${2}p")
        [ -n "$line" ] || return 1
        date -u -d "${line%% *}" +%s%3N
    }

    # The Nth line number of the shell's journal with PATTERN.
    line_of() { # line_of PATTERN N
        grep -nF -- "$1" "$err" | sed -n "${2}p" | cut -d: -f1
    }

    send() { # a verb to the nest's shell, retried while it starts
        tries=0
        until guard && "$bin_dir/celestina" "$@"; do
            tries=$((tries + 1))
            [ "$tries" -lt 40 ] || { say "verb '$*' failed"; return 1; }
            sleep 0.25
        done
        say "verb '$*' sent"
    }

    wait_shell() { # until the shell says the lock duty is on
        wait_count "$err" 'lock: the duty is on' 1 40 || { say "the shell never started the lock duty"; return 1; }
    }

    lock_pid() { # the pid in the lock's own pid file (design §8 step 2)
        [ -f "$state_file" ] || return 1
        sed -n 's/^pid \([0-9][0-9]*\)$/\1/p' "$state_file"
    }

    alive() { [ -n "${1:-}" ] && [ -d "/proc/$1" ]; }

    # Whether PID is still the program it was recorded as. The shell runs the
    # lock from a pinned descriptor (`/proc/self/fd/N`) with the program's
    # name as argv[0], so the kernel's name for it is the descriptor's
    # number: argv[0] is checked too.
    is_program() { # is_program PID NAME
        alive "$1" || return 1
        comm=$(cat "/proc/$1/comm" 2>/dev/null || true)
        argv0=$(tr '\0' '\n' < "/proc/$1/cmdline" 2>/dev/null | head -n 1 || true)
        if [ "$comm" != "$2" ] && [ "${argv0##*/}" != "$2" ]; then
            say "pid $1 is '$comm' ($argv0), not '$2'"
            return 1
        fi
        return 0
    }

    # Stops one process by the pid its launcher recorded, after checking that
    # the pid is still the program it was recorded as.
    kill_pid() { # kill_pid PID NAME [SIGNAL]
        alive "$1" || return 0
        is_program "$1" "$2" || { say "NOT killing $1"; return 1; }
        kill "-${3:-KILL}" "$1" 2>/dev/null
        say "kill -${3:-KILL} $1 ($2)"
    }

    # No lock of any other display may be alive (the nest shares the runtime
    # directory with the live session).
    other_locks() {
        for f in "$runtime_celestina"/lock-*.state; do
            [ -f "$f" ] || continue
            [ "$f" != "$state_file" ] || continue
            p=$(sed -n 's/^pid \([0-9][0-9]*\)$/\1/p' "$f")
            if alive "$p"; then
                say "ABORT: $f names live process $p"
                return 1
            fi
        done
        return 0
    }

    # Locks through the shell's own verb and waits for `locked`. Sets
    # `verb_at` and `locked_at` (epoch ms) and `locks_before`.
    lock_through_shell() {
        wait_shell || return 1
        locks_before=$(count "$err" 'status line line=locked')
        verb_at=$(now_ms)
        send lock || return 1
        wait_count "$err" 'status line line=locked' $((locks_before + 1)) 30 || { say "no locked line"; return 1; }
        locked_at=$(ts_of 'status line line=locked' $((locks_before + 1)))
        say "locked ${locked_at} (verb at ${verb_at}, $((locked_at - verb_at)) ms)"
    }

    shell_exit() { # asks the shell to quit and waits for the wrapper to say so
        send quit || return 1
        i=0
        while [ ! -e "$ctl/shell-exited" ] && [ "$i" -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
        [ -e "$ctl/shell-exited" ]
    }

    finish() { # the nest ends when the wrapper returns
        trap - EXIT
        if [ "${LOCK_REHEARSAL_SHELL:-0}" = 1 ] && [ ! -e "$ctl/shell-exited" ]; then
            "$bin_dir/celestina" quit >/dev/null 2>&1 || true
        fi
        : > "$ctl/finish"
    }

    # ── the steps ────────────────────────────────────────────────────────────

    step_r1() {
        lock_through_shell || { result R-1 FAIL "no locked line"; return; }
        # At once (the cover, or the prompt when it was fast enough), then
        # after a second and a half (the prompt).
        shot locked
        sleep 1.5
        shot prompt
        # The cover then the prompt: the first lines in order, the compositor
        # confirmation, and the time from the verb to `locked`.
        a=$(line_of 'status line line=hello 1' 1)
        b=$(line_of 'status line line=requested' 1)
        c=$(line_of 'status line line=locked' 1)
        msg="hello/requested/locked at lines ${a:-?}/${b:-?}/${c:-?}, $((locked_at - verb_at)) ms from the verb to locked"
        if [ -n "$a" ] && [ -n "$b" ] && [ -n "$c" ] && [ "$a" -lt "$b" ] && [ "$b" -lt "$c" ] \
            && [ "$(count_locking "$nest_log")" -ge 1 ]; then
            if [ $((locked_at - verb_at)) -le 1500 ]; then
                result R-1 PASS "$msg"
            else
                result R-1 NOTE "$msg (over 1.5 s: see the evidence note on the nest's frame callbacks)"
            fi
        else
            result R-1 FAIL "$msg"
        fi
    }

    step_r2() {
        lock_through_shell || { result R-2 FAIL "no locked line"; return; }
        first=$(lock_pid)
        say "first lock pid $first"
        nest_bg "$bin_dir/celestina-lock" < /dev/null > "$prefix.second.lines" 2> "$prefix.second.err" &
        second=$!
        track "$second" celestina-lock
        i=0
        while alive "$second" && [ "$i" -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
        if alive "$second"; then
            kill_pid "$second" celestina-lock
            result R-2 FAIL "the second celestina-lock was still running after 10 s"
            return
        fi
        wait "$second"
        status=$?
        shot after
        if [ "$status" -eq 10 ] && alive "$first" && [ "$(lock_pid)" = "$first" ] \
            && [ "$(count "$err" 'status line line=unlocked')" -eq 0 ]; then
            result R-2 PASS "second celestina-lock exit $status; first (pid $first) alive, same pid file, no unlock"
        else
            result R-2 FAIL "second exit $status; first alive: $(alive "$first" && echo yes || echo no)"
        fi
    }

    step_r3() {
        lock_through_shell || { result R-3 FAIL "no locked line"; return; }
        first=$(lock_pid)
        say "R-3 part 1: SIGKILL the lock by the pid in its pid file ($first)"
        replacing_before=$(count "$nest_log" 'replacing existing dead lock')
        kill_pid "$first" celestina-lock || { result R-3 FAIL "could not kill $first"; return; }
        sleep 0.3
        shot dead-lock
        wait_count "$err" 'status line line=locked' 2 30 || { result R-3 FAIL "the shell did not relaunch a lock that reached locked"; return; }
        relaunched_at=$(ts_of 'status line line=locked' 2)
        second=$(lock_pid)
        sleep 0.5
        shot relaunched
        replacing_after=$(count "$nest_log" 'replacing existing dead lock')
        part1=FAIL
        if [ -n "$second" ] && [ "$second" != "$first" ] && [ "$replacing_after" -gt "$replacing_before" ]; then
            part1=PASS
        fi
        say "R-3 part 1 $part1: relaunched as pid $second, niri replacements $replacing_before -> $replacing_after"

        say "R-3 part 2: stop the shell, kill the lock again, start swaylock against the nest"
        shell_exit || { result R-3 FAIL "the shell did not quit"; return; }
        kill_pid "$second" celestina-lock || { result R-3 FAIL "could not kill $second"; return; }
        sleep 0.5
        replacing_before=$(count "$nest_log" 'replacing existing dead lock')
        guard || return
        # `env -i` and no HOME: the author's swaylock configuration is not read.
        nest_bg swaylock -c 000000 > "$prefix.swaylock.out" 2>&1 &
        sway=$!
        track "$sway" swaylock
        i=0
        while [ "$(count "$nest_log" 'replacing existing dead lock')" -le "$replacing_before" ] && [ "$i" -lt 100 ]; do
            sleep 0.1
            i=$((i + 1))
        done
        sleep 1.5
        shot swaylock
        part2=FAIL
        if [ "$(count "$nest_log" 'replacing existing dead lock')" -gt "$replacing_before" ] && alive "$sway"; then
            part2=PASS
        fi
        say "R-3 part 2 $part2: swaylock pid $sway alive: $(alive "$sway" && echo yes || echo no)"
        # No password is typed. swaylock's own documented way to end a lock
        # is SIGUSR1 ("Unlock the screen and exit"); it proves the takeover
        # held the session and lets the nest end. Its PAM unlock was NOT
        # tried.
        unlocking_before=$(count "$nest_log" 'unlocking session')
        kill_pid "$sway" swaylock USR1
        i=0
        while alive "$sway" && [ "$i" -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
        part3=FAIL
        if ! alive "$sway" && [ "$(count "$nest_log" 'unlocking session')" -gt "$unlocking_before" ]; then
            part3=PASS
        fi
        if [ "$part1" = PASS ] && [ "$part2" = PASS ] && [ "$part3" = PASS ]; then
            result R-3 PASS "relaunch after SIGKILL (pid $first -> $second, ${relaunched_at}) and swaylock takeover of a dead lock; its unlock was SIGUSR1, not a password (the PAM unlock of swaylock was not tried)"
        else
            result R-3 FAIL "relaunch $part1, swaylock takeover $part2, swaylock SIGUSR1 unlock $part3"
        fi
    }

    # Between `requested` and `locked` is a window of a few tens of
    # milliseconds (the compositor confirms after one frame): the verb is sent
    # in the background, the lock's pid is read the moment its pid file
    # exists, and the kill follows the `requested` line as fast as a shell can
    # poll. A kill that lands after `locked` proves nothing about this window,
    # so it asks the host for another try in a fresh nest.
    step_r4() {
        attempt=${LOCK_REHEARSAL_ATTEMPT:-1}
        wait_shell || { result R-4 FAIL "shell not ready"; return; }
        replacing_before=$(count "$nest_log" 'replacing existing dead lock')
        guard || return
        "$bin_dir/celestina" lock > /dev/null 2>&1 &
        pid=
        i=0
        while [ -z "$pid" ] && [ "$i" -lt 5000 ]; do
            pid=$(lock_pid 2> /dev/null)
            [ -n "$pid" ] || sleep 0.002
            i=$((i + 1))
        done
        [ -n "$pid" ] || { result R-4 FAIL "the lock never wrote its pid file"; return; }
        is_program "$pid" celestina-lock || { result R-4 FAIL "pid $pid is not the lock"; return; }
        i=0
        until grep -qF 'status line line=requested' "$err"; do
            i=$((i + 1))
            [ "$i" -lt 5000 ] || { result R-4 FAIL "no requested line"; return; }
            sleep 0.002
        done
        kill -KILL "$pid" 2> /dev/null
        killed_at=$(now_ms)
        locked_before_kill=$(count "$err" 'status line line=locked')
        say "R-4 attempt $attempt: kill -KILL $pid (celestina-lock) at ${killed_at}, locked lines at that moment: $locked_before_kill"
        if [ "$locked_before_kill" -ne 0 ]; then
            result R-4 RETRY "the kill landed after locked (attempt $attempt)"
            return
        fi
        sleep 1.8
        shot after-kill
        wait_count "$err" 'status line line=locked' 1 40 || { result R-4 FAIL "the relaunched lock never reached locked"; return; }
        sleep 1
        shot relaunched
        again=$(lock_pid)
        if [ -n "$again" ] && [ "$again" != "$pid" ] \
            && [ "$(count "$nest_log" 'replacing existing dead lock')" -gt "$replacing_before" ]; then
            result R-4 PASS "killed between requested and locked (attempt $attempt); niri kept the session locked, the shell relaunched ($pid -> $again) and niri replaced the dead lock"
        else
            result R-4 FAIL "new pid ${again:-none}; niri replacements $replacing_before -> $(count "$nest_log" 'replacing existing dead lock')"
        fi
    }

    step_r5() {
        lock_through_shell || { result R-5 FAIL "no locked line"; return; }
        before=$(lock_pid)
        shot locked
        shell_exit || { result R-5 FAIL "the shell did not quit"; return; }
        sleep 1
        alive_after_quit=no
        alive "$before" && alive_after_quit=yes
        shot shell-gone
        : > "$ctl/restart"
        i=0
        while [ -e "$ctl/shell-exited" ] && [ "$i" -lt 60 ]; do sleep 0.1; i=$((i + 1)); done
        # The new shell's log lines start after the old ones.
        started=$(count "$err" 'lock: the duty is on')
        wait_count "$err" 'lock: the duty is on' 2 40 || { result R-5 FAIL "the second shell never started"; return; }
        say "R-5 second shell ready (duty lines: $started -> $(count "$err" 'lock: the duty is on'))"
        send lock suspend || say "lock suspend verb not accepted"
        # Two refusal retries at 0.5 s each, then the notice.
        sleep 6
        shot after-lock-suspend
        suspends=$(suspend_calls)
        still=$(lock_pid)
        if [ "$alive_after_quit" = yes ] && [ "$still" = "$before" ] && alive "$before" \
            && [ "${suspends:-0}" -eq 0 ] && [ "$(count "$err" 'status line line=unlocked')" -eq 0 ]; then
            result R-5 PASS "lock pid $before survived the shell's quit and restart, still the only lock; 'lock suspend' did not suspend (Suspend calls on the fake logind: ${suspends:-0})"
        else
            result R-5 FAIL "survived quit: $alive_after_quit; pid file $still (was $before); Suspend calls ${suspends:-0}"
        fi
        grep -E 'lock:|Suspend|suspend' "$err" | tail -n 25 > "$prefix.lock-suspend.excerpt" 2>/dev/null || true
    }

    step_r6() {
        lock_through_shell || { result R-6 FAIL "no locked line"; return; }
        pid=$(lock_pid)
        owner=$(stat -c '%u:%g' "/proc/$pid/environ" 2>&1)
        me=$(id -u)
        env_read=ok
        cat "/proc/$pid/environ" > /dev/null 2>&1 || env_read=denied
        mem_read=ok
        dd if="/proc/$pid/mem" of=/dev/null bs=1 count=1 skip=4194304 2> /dev/null || mem_read=denied
        say "R-6 pid $pid: environ owner $owner (uid is $me), read environ: $env_read, read mem: $mem_read"
        if [ "$env_read" = denied ] && [ "$mem_read" = denied ] && [ "$owner" = "0:0" ]; then
            result R-6 PASS "/proc/$pid/environ owned by $owner; the user cannot read environ or mem"
        else
            result R-6 FAIL "environ owner $owner, environ $env_read, mem $mem_read"
        fi
    }

    step_r7() {
        guard || return
        other_locks || { result R-7 FAIL "another display holds a lock"; return; }
        nest_bg "$bin_dir/celestina-lock" --rehearsal < /dev/null > "$prefix.lines" 2> "$prefix.lock.err" &
        lockpid=$!
        track "$lockpid" celestina-lock
        i=0
        while [ "$(count_line "$prefix.lines" locked)" -lt 1 ] && [ "$i" -lt 300 ]; do sleep 0.1; i=$((i + 1)); done
        if [ "$(count_line "$prefix.lines" locked)" -lt 1 ]; then
            kill_pid "$lockpid" celestina-lock
            result R-7 FAIL "the rehearsal lock never said locked"
            return
        fi
        sleep 1
        shot prompt
        # A made-up password; the author's is never typed.
        say "R-7 typing a wrong password into the nest's lock surface"
        # The attempt lines are timed by a poller of their own, so what is
        # measured does not depend on how long `wtype` takes to return: in a
        # slow nest it can take seconds, and a count read at a fixed wall-clock
        # offset then reads the delay wrong (R-7 failed that way on 2026-10-23).
        times=$prefix.attempt-times
        : > "$times"
        (
            seen=0
            while :; do
                c=$(count "$prefix.lines" 'attempt')
                while [ "$seen" -lt "$c" ]; do
                    seen=$((seen + 1))
                    printf '%s %s\n' "$seen" "$(now_ms)" >> "$times"
                done
                sleep 0.05
            done
        ) &
        poller=$!
        t1=$(now_ms)
        nestenv wtype wrongpass -k Return 2>> "$steps_log"
        i=0
        while [ "$(count "$prefix.lines" 'attempt')" -lt 1 ] && [ "$i" -lt 200 ]; do sleep 0.1; i=$((i + 1)); done
        sleep 0.5
        shot refused
        # Typing again at once and pressing Enter again every second: an Enter
        # inside the 2 s delay does nothing, so the first one the lock accepts
        # is the one at least 2 s after the refusal, whenever the nest delivers
        # it.
        nestenv wtype second -k Return 2>> "$steps_log"
        i=0
        while [ "$(count "$prefix.lines" 'attempt')" -lt 2 ] && [ "$i" -lt 20 ]; do
            sleep 1
            nestenv wtype -k Return 2>> "$steps_log"
            i=$((i + 1))
        done
        sleep 0.5
        shot refused-again
        kill -TERM "$poller" 2>/dev/null
        wait "$poller" 2>/dev/null
        a1=$(sed -n '1s/^1 //p' "$times")
        a2=$(sed -n '2s/^2 //p' "$times")
        unlocked=$(count_line "$prefix.lines" unlocked)
        still=no
        alive "$lockpid" && still=yes
        kill_pid "$lockpid" celestina-lock
        gap=-1
        [ -n "$a1" ] && [ -n "$a2" ] && gap=$((a2 - a1))
        say "R-7 attempt lines seen at ${a1:-never} and ${a2:-never} ms (first typed ${t1}), gap ${gap} ms, unlocked lines ${unlocked}, still locked ${still}"
        # The second attempt starts no sooner than the 2 s delay after the first
        # refusal and its line comes after the check, so the gap between the
        # two lines is at least 2 s (0.1 s of slack for the poller).
        if [ "$gap" -ge 1900 ] && [ "$unlocked" -eq 0 ] && [ "$still" = yes ]; then
            result R-7 PASS "wrong passwords refused through the rehearsal service (no such file: PAM 'other' denies); the two attempt lines were ${gap} ms apart, at least the 2 s delay; never unlocked"
        else
            result R-7 FAIL "gap between the first two attempt lines ${gap} ms (needs at least 1900); unlocked lines $unlocked; still locked $still"
        fi
    }

    step_r8() {
        guard || return
        other_locks || { result R-8 FAIL "another display holds a lock"; return; }
        unlocking_before=$(count "$nest_log" 'unlocking session')
        nest_bg "$LOCK_REHEARSAL_UNLOCK_PROBE" < /dev/null > "$prefix.lines" 2> "$prefix.probe.err" &
        probe=$!
        track "$probe" unlock_probe
        i=0
        while alive "$probe" && [ "$i" -lt 200 ]; do sleep 0.1; i=$((i + 1)); done
        if alive "$probe"; then
            kill_pid "$probe" unlock_probe
            result R-8 FAIL "unlock_probe did not end within 20 s"
            return
        fi
        wait "$probe"
        status=$?
        sleep 0.5
        lines=$(tr '\n' ',' < "$prefix.lines")
        if [ "$status" -eq 0 ] && [ "$(count_line "$prefix.lines" unlocked)" -ge 1 ] \
            && [ "$(count "$nest_log" 'unlocking session')" -gt "$unlocking_before" ]; then
            result R-8 PASS "unlock_probe: lines [$lines] exit 0; niri logged 'unlocking session'"
        else
            result R-8 FAIL "exit $status; lines [$lines]"
        fi
    }

    # R-9a and R-9b run the idle chain with its defaults (20 s screen off,
    # 240 s lock and suspend): the delays have no setting, so each takes about
    # four minutes of nobody touching the nest.

    step_r9a() {
        wait_shell || { result R-9a FAIL "shell not ready"; return; }
        armed_at=$(ts_of 'idle: armed' 1)
        say "R-9a idle armed at ${armed_at:-?}; waiting for the chain (about 250 s)"
        wait_count "$err" 'status line line=locked' 1 330 || { result R-9a FAIL "the idle chain never locked"; return; }
        locked_at=$(ts_of 'status line line=locked' 1)
        sleep 3
        shot locked
        suspend_line=$(grep -E '^[0-9]+ Suspend ' "$logind_log" | head -n 1)
        suspend_at=${suspend_line%% *}
        say "R-9a locked at ${locked_at}; first Suspend on the fake logind: ${suspend_line:-none}"
        if [ -n "$suspend_at" ] && [ "$suspend_at" -ge "$locked_at" ]; then
            result R-9a PASS "idle armed ${armed_at:-?}, locked ${locked_at}, Suspend recorded ${suspend_at} ($((suspend_at - locked_at)) ms after locked)"
        elif [ -z "$suspend_at" ]; then
            result R-9a FAIL "locked at ${locked_at} but no Suspend reached the fake logind"
        else
            result R-9a FAIL "Suspend ${suspend_at} BEFORE locked ${locked_at}"
        fi
    }

    # Input that ends the idle period before the chain's second step must stop
    # it: pointer input at 230 s (the unaided chain locks at 240 s, R-9a), and
    # nothing asked of the lock or of logind by 262 s. The window between the
    # lock's request and its confirmation is not reachable from outside: in
    # the nest `requested` and `locked` are a fraction of a millisecond apart
    # (spike run, evidence note), so "input between the two drops the
    # idle-origin suspend" stays with T-P7 and T-I1.
    step_r9b() {
        if [ -z "${LOCK_REHEARSAL_POINTER_TOOL:-}" ]; then
            result R-9b NOTE "not run: no --pointer-tool (a virtual keyboard does not count as activity)"
            return
        fi
        guard || return
        wait_shell || { result R-9b FAIL "shell not ready"; return; }
        armed_at=$(ts_of 'idle: armed' 1)
        [ -n "$armed_at" ] || { result R-9b FAIL "the idle chain never armed"; return; }
        # The pointer client connects first and waits for one line; a pipe
        # held open here (read and write) lets it start without a writer. It
        # ends by itself: with the nest gone, or with the pipe closed when
        # this probe exits.
        mkfifo "$ctl/pointer.cmd"
        exec 7<> "$ctl/pointer.cmd"
        nest_bg "$LOCK_REHEARSAL_POINTER_TOOL" < "$ctl/pointer.cmd" > "$prefix.pointer.out" 2>&1 &
        say "R-9b idle armed at ${armed_at}; pointer input scheduled for armed+230 s"
        while [ "$(now_ms)" -lt $((armed_at + 230000)) ]; do sleep 0.2; done
        printf 'go\n' >&7
        moved_at=$(now_ms)
        say "R-9b pointer input released at ${moved_at} (armed+$((moved_at - armed_at)) ms); lock requests so far: $(count "$err" 'status line line=requested')"
        while [ "$(now_ms)" -lt $((armed_at + 262000)) ]; do sleep 0.2; done
        shot after-240s
        requests=$(count "$err" 'status line line=requested')
        suspends=$(suspend_calls)
        if [ "$requests" -eq 0 ] && [ "${suspends:-0}" -eq 0 ]; then
            result R-9b PASS "pointer input at armed+$((moved_at - armed_at)) ms ended the idle period: at armed+262 s no lock was requested and no Suspend reached the fake logind (the unaided chain locks at armed+240 s, R-9a)"
        else
            result R-9b FAIL "lock requests: $requests, Suspend calls: ${suspends:-0} after input before the second step"
        fi
    }

    step_r9c() {
        wait_shell || { result R-9c FAIL "shell not ready"; return; }
        holds=$(grep -c 'Inhibit ' "$logind_log" 2>/dev/null || true)
        say "R-9c PrepareForSleep(true) with no lock held; inhibitors taken so far: ${holds:-0}"
        printf 'sleep\n' > "$ctl/logind.cmd"
        t0=$(now_ms)
        wait_count "$err" 'status line line=locked' 1 30 || { result R-9c FAIL "no lock after PrepareForSleep"; return; }
        locked_at=$(ts_of 'status line line=locked' 1)
        sleep 1
        released=$(grep 'InhibitorReleased' "$logind_log" | tail -n 1)
        rel_at=${released%% *}
        printf 'wake\n' > "$ctl/logind.cmd"
        sleep 0.5
        suspends=$(suspend_calls)
        if [ -n "$rel_at" ] && [ "$rel_at" -ge "$locked_at" ] && [ "${suspends:-0}" -eq 0 ]; then
            result R-9c PASS "PrepareForSleep(true) -> locked ($((locked_at - t0)) ms) -> inhibitor released ${rel_at} ($((rel_at - locked_at)) ms after locked); no Suspend call"
        else
            result R-9c FAIL "locked ${locked_at}; inhibitor released '${rel_at:-never}'; Suspend calls ${suspends:-0}"
        fi
    }

    # The p-th quantile (0..1) of the numbers in FILE, the way `paint_probe`
    # picks one: the sorted value at round((n-1)*p).
    quantile() { # quantile P FILE
        sort -n "$2" | awk -v p="$1" '{ v[NR] = $1 } END { if (NR == 0) { print "n/a"; exit } print v[int((NR - 1) * p + 0.5) + 1] }'
    }

    # R-10 (H-b): the lock with the dimmed backdrop and the clock, at scale
    # 1.5 on the 4K window, over a real wallpaper thumbnail (the shell runs the
    # wallpaper duty over a real picture and locks with its own `lock` verb, so
    # the bundle carries the thumbnail). Three things come out of it:
    #   1. the captures of the backdrop and the clock, and that the lock read a
    #      complete bundle;
    #   2. whether the clock redraws at the minute boundary, and an upper bound
    #      of how late (a capture is polled from just before the boundary);
    #   3. the wall-clock time from a key press to the first capture that
    #      differs, 20 samples, as p50 and p99.
    # LIMIT: `celestina-lock` exposes no frame time (no `--metrics`, no log
    # line, and `WAYLAND_DEBUG` would print key events, so it is not used), so
    # the painter's own p50/p99 are NOT measured here and none is invented. The
    # figures in 3 contain `wtype`, the nest's compositor and one `grim`
    # capture (hundreds of milliseconds at 4K): an upper bound that cannot be
    # compared with the 16 ms budget of H-15. Painter-only times without the
    # backdrop or the clock come from `examples/paint_probe`. The keys are an
    # `x` and a BackSpace, never Enter and never a password, and the nest's
    # lock is the only surface that gets them.
    step_r10() {
        guard || return
        wait_count "$err" 'wallpaper surface created' 1 40 || { result R-10 FAIL "the wallpaper duty never painted a surface in the nest (nothing to thumbnail)"; return; }
        # The thumbnail is derived beside the decode; give the store a moment.
        sleep 1
        lock_through_shell || { result R-10 FAIL "no locked line"; return; }
        wait_count "$err" 'backdrop bundle read' 1 15 || {
            result R-10 FAIL "the lock never read a complete backdrop bundle ($(count "$err" 'backdrop bundle not fully usable') 'not fully usable' lines)"
            return
        }
        sleep 2
        shot backdrop
        size=
        [ ! -f "$prefix-backdrop.png" ] || size=$(python3 -I -c 'import struct, sys; print(*struct.unpack(">II", open(sys.argv[1], "rb").read(24)[16:24]))' "$prefix-backdrop.png" 2>/dev/null)
        bytes=0
        [ ! -f "$prefix-backdrop.png" ] || bytes=$(stat -c %s "$prefix-backdrop.png")
        # Whether the backdrop is a flat colour is judged on the pixels, not on
        # the PNG's size: a dimmed and blurred picture compresses to a few tens
        # of kilobytes at 4K (the first R-10 of 2026-10-23 measured 78 KB over a
        # real backdrop and called it flat). An uncompressed PPM of the same
        # frame is sampled on a grid and the spread of its luminance decides.
        nestenv grim -t ppm "$work/backdrop.ppm" 2>> "$steps_log" || say "R-10 backdrop ppm capture failed"
        spread=$(python3 -I - "$work/backdrop.ppm" << 'PY'
import sys
try:
    data = open(sys.argv[1], "rb").read()
except OSError:
    print("n/a"); sys.exit(0)
fields, i = [], 0
while len(fields) < 4 and i < len(data):
    while i < len(data) and data[i:i+1].isspace(): i += 1
    if data[i:i+1] == b"#":
        while i < len(data) and data[i:i+1] not in (b"\n", b""): i += 1
        continue
    start = i
    while i < len(data) and not data[i:i+1].isspace(): i += 1
    fields.append(data[start:i])
i += 1
if len(fields) < 4 or fields[0] != b"P6":
    print("n/a"); sys.exit(0)
w, h = int(fields[1]), int(fields[2])
px = data[i:]
step = max(1, (w * h) // 20000)
lum = []
for n in range(0, w * h, step):
    o = n * 3
    if o + 2 >= len(px): break
    lum.append((299 * px[o] + 587 * px[o+1] + 114 * px[o+2]) // 1000)
if len(lum) < 100:
    print("n/a"); sys.exit(0)
mean = sum(lum) / len(lum)
var = sum((l - mean) ** 2 for l in lum) / len(lum)
print(int(var ** 0.5))
PY
)
        say "R-10 backdrop luminance spread (sampled std dev, 0-255): ${spread:-n/a}"
        say "R-10 capture ${size:-?} (width height), $bytes bytes"

        # 3. The key presses. The reference capture is taken before each one.
        # Each press also keeps the 30 s fade away.
        keys=$work/r10-key-ms
        : > "$keys"
        n=0
        while [ "$n" -lt 20 ]; do
            guard || return
            nestenv grim -t ppm "$work/poll-a.ppm" 2>> "$steps_log" || { say "R-10 reference capture failed"; break; }
            t0=$(now_ms)
            if [ $((n % 2)) -eq 0 ]; then
                nestenv wtype x 2>> "$steps_log"
            else
                nestenv wtype -k BackSpace 2>> "$steps_log"
            fi
            seen=
            k=0
            while [ "$k" -lt 20 ]; do
                nestenv grim -t ppm "$work/poll-b.ppm" 2>> "$steps_log" || break
                if ! cmp -s "$work/poll-a.ppm" "$work/poll-b.ppm"; then
                    seen=$(now_ms)
                    break
                fi
                k=$((k + 1))
            done
            [ -z "$seen" ] || printf '%s\n' $((seen - t0)) >> "$keys"
            n=$((n + 1))
            sleep 1
        done
        samples=$(wc -l < "$keys" | tr -d ' ')
        key_p50=$(quantile 0.50 "$keys")
        key_p99=$(quantile 0.99 "$keys")
        say "R-10 key press to visible change: n=$samples p50=${key_p50}ms p99=${key_p99}ms"

        # 2. The minute. Keep the fade away until the last seconds of a minute
        # (the typed `x` and the BackSpace leave the field empty), take two
        # reference captures to see that the screen is quiet, then poll.
        while [ "$(date +%-S)" -ge 52 ]; do sleep 1; done
        until [ "$(date +%-S)" -ge 52 ]; do
            guard || return
            nestenv wtype x -k BackSpace 2>> "$steps_log"
            sleep 5
        done
        sleep 1
        guard || return
        nestenv grim -t ppm "$work/poll-a.ppm" 2>> "$steps_log" || { result R-10 FAIL "reference capture failed"; return; }
        nestenv grim -t ppm "$work/poll-b.ppm" 2>> "$steps_log" || { result R-10 FAIL "reference capture failed"; return; }
        quiet=yes
        cmp -s "$work/poll-a.ppm" "$work/poll-b.ppm" || quiet=no
        boundary=$((($(date +%s) / 60 + 1) * 60 * 1000))
        minute_late=
        minute_early=
        if [ "$(now_ms)" -lt $((boundary - 800)) ]; then
            nestenv grim -t ppm "$work/poll-a.ppm" 2>> "$steps_log" || say "R-10 reference capture failed"
            while [ "$(now_ms)" -lt $((boundary + 8000)) ]; do
                s=$(now_ms)
                nestenv grim -t ppm "$work/poll-b.ppm" 2>> "$steps_log" || break
                e=$(now_ms)
                if ! cmp -s "$work/poll-a.ppm" "$work/poll-b.ppm"; then
                    minute_early=$((s - boundary))
                    minute_late=$((e - boundary))
                    break
                fi
            done
            say "R-10 minute boundary ${boundary}: first differing capture started ${minute_early:-never} ms and ended ${minute_late:-never} ms after it (screen quiet before: $quiet)"
        else
            say "R-10 the minute boundary was too close to start polling"
        fi
        shot after-minute

        # The verdicts. A flat colour has a luminance spread near 0; the dimmed
        # picture of the generated wallpaper measures well above 10. The
        # threshold is deliberately low: it tells black from a picture, nothing
        # finer.
        flat=yes
        case "$spread" in
            ''|n/a) ;;
            *) [ "$spread" -lt 10 ] || flat=no ;;
        esac
        why=
        [ "$flat" = no ] || why="$why backdrop is a flat colour (luminance spread ${spread:-n/a});"
        [ -n "$minute_late" ] || why="$why no redraw seen within 8 s of the minute boundary;"
        [ -z "$minute_late" ] || [ "$minute_early" -ge -200 ] || why="$why the screen changed more than 200 ms before the boundary;"
        if [ -z "$why" ]; then
            result R-10 PASS "the lock read a complete bundle over the wallpaper duty's picture; capture ${size:-?} px, luminance spread ${spread} (a picture, not a flat colour); the clock redrew at the minute (first differing capture ended ${minute_late} ms after the boundary, started ${minute_early} ms after it; screen quiet before: $quiet)"
        else
            result R-10 FAIL "${why# } capture ${size:-?} px, $bytes bytes, luminance spread ${spread:-n/a}; minute redraw: ${minute_late:-not seen within 8 s} ms (started ${minute_early:-n/a})"
        fi
        case "$size" in
            "3840 2160") ;;
            *) result R-10 NOTE "the capture is ${size:-unknown} px, not the 4K window (3840 2160): the figures are for that size, not for the author's monitor" ;;
        esac
        if [ "${samples:-0}" -ge 10 ]; then
            msg="key press (wtype x / BackSpace against the nest) to the first differing grim capture: n=$samples p50=${key_p50} ms p99=${key_p99} ms. This is an upper bound that includes wtype, the nest's compositor and a capture; it is NOT the painter's frame time, which celestina-lock does not expose, so none is reported and it cannot be compared with the 16 ms budget"
            [ "${key_p99:-0}" -le 16 ] || msg="$msg; p99 is above 16 ms: recorded for the author (H-15 revisit); the painter is unchanged"
            result R-10 NOTE "$msg"
        else
            result R-10 NOTE "only $samples of 20 key presses changed the screen: no p50/p99 recorded; the lock exposes no frame time, so there is no other source"
        fi
        result R-10 NOTE "the minute figure above has the resolution of one capture (hundreds of ms at 4K); the in-process frame times of the key press and of the minute redraw are not measured (no --metrics or log in celestina-lock): the painter-only figures come from examples/paint_probe, which draws neither the backdrop nor the clock"
        grep -E 'backdrop|clock' "$err" | tail -n 20 > "$prefix.lock-lines.excerpt" 2>/dev/null || true
    }

    # ── the probe ────────────────────────────────────────────────────────────

    trap finish EXIT
    trap 'finish; exit 1' INT TERM
    guard || exit 3
    other_locks || exit 3
    say "probe for ${LOCK_REHEARSAL_STEP} on $WAYLAND_DISPLAY (host is $LOCK_REHEARSAL_HOST_DISPLAY), nest niri from $NIRI_SOCKET"
    printf 'WAYLAND_DISPLAY=%s\nNIRI_SOCKET=%s\n' "$WAYLAND_DISPLAY" "$NIRI_SOCKET" > "$ctl/nest.env"
    case "$LOCK_REHEARSAL_STEP" in
        R-1) step_r1 ;;
        R-2) step_r2 ;;
        R-3) step_r3 ;;
        R-4) step_r4 ;;
        R-5) step_r5 ;;
        R-6) step_r6 ;;
        R-7) step_r7 ;;
        R-8) step_r8 ;;
        R-9a) step_r9a ;;
        R-9b) step_r9b ;;
        R-9c) step_r9c ;;
        R-10) step_r10 ;;
        *) say "unknown step $LOCK_REHEARSAL_STEP"; exit 2 ;;
    esac
    exit 0
fi

# ── on the host ──────────────────────────────────────────────────────────────

set -eu

usage() {
    echo "usage: lock-rehearsal.sh --bin-dir DIR --fake-logind PATH --out DIR" >&2
    echo "                         [--unlock-probe PATH] [--pointer-tool PATH]" >&2
    echo "                         [--wallpaper-folder DIR] [--monitor NAME|none]" >&2
    echo "                         [--check] [R-1 ... R-10 | R-9a R-9b R-9c]" >&2
}

bin_dir=
fake_logind=
out=
unlock_probe=
pointer_tool=
wallpaper_folder=
r10_monitor=DP-1
check=no
steps=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --bin-dir) [ "$#" -ge 2 ] || { usage; exit 2; }; bin_dir=$2; shift 2 ;;
        --fake-logind) [ "$#" -ge 2 ] || { usage; exit 2; }; fake_logind=$2; shift 2 ;;
        --out) [ "$#" -ge 2 ] || { usage; exit 2; }; out=$2; shift 2 ;;
        --unlock-probe) [ "$#" -ge 2 ] || { usage; exit 2; }; unlock_probe=$2; shift 2 ;;
        --pointer-tool) [ "$#" -ge 2 ] || { usage; exit 2; }; pointer_tool=$2; shift 2 ;;
        --wallpaper-folder) [ "$#" -ge 2 ] || { usage; exit 2; }; wallpaper_folder=$2; shift 2 ;;
        --monitor) [ "$#" -ge 2 ] || { usage; exit 2; }; r10_monitor=$2; shift 2 ;;
        --check) check=yes; shift ;;
        R-[1-9]|R-9[abc]|R-10) steps="$steps $1"; shift ;;
        --help|-h) usage; exit 0 ;;
        *) usage; exit 2 ;;
    esac
done
[ -n "$bin_dir" ] && [ -n "$fake_logind" ] && [ -n "$out" ] || { usage; exit 2; }
[ -n "$steps" ] || steps=" R-1 R-2 R-3 R-4 R-5 R-6 R-7 R-8 R-9 R-10"

refuse() {
    echo "lock-rehearsal: $*" >&2
    exit 1
}

# Absolute paths: the nest runs this from another directory.
abs() {
    case "$1" in
        /*) printf '%s' "$1" ;;
        *) printf '%s/%s' "$(pwd)" "$1" ;;
    esac
}
bin_dir=$(abs "$bin_dir")
fake_logind=$(abs "$fake_logind")
out=$(abs "$out")
[ -z "$unlock_probe" ] || unlock_probe=$(abs "$unlock_probe")
[ -z "$pointer_tool" ] || pointer_tool=$(abs "$pointer_tool")
[ -z "$wallpaper_folder" ] || wallpaper_folder=$(abs "$wallpaper_folder")

# The refusals.
[ -n "${XDG_RUNTIME_DIR:-}" ] || refuse "XDG_RUNTIME_DIR is empty"
[ -n "${WAYLAND_DISPLAY:-}" ] || refuse "WAYLAND_DISPLAY is empty: a nested niri needs a host compositor to open its window in"
host_display=$WAYLAND_DISPLAY
if [ -e /etc/pam.d/celestina-lock-rehearsal ] || [ -L /etc/pam.d/celestina-lock-rehearsal ]; then
    refuse "/etc/pam.d/celestina-lock-rehearsal exists: a rehearsal is only safe against a PAM service that does not (PAM then falls to 'other' and denies)"
fi
runtime_state=$XDG_RUNTIME_DIR/celestina
for f in "$runtime_state"/lock-*.state; do
    [ -f "$f" ] || continue
    pid=$(sed -n 's/^pid \([0-9][0-9]*\)$/\1/p' "$f")
    if [ -n "$pid" ] && [ -d "/proc/$pid" ]; then
        refuse "$f names process $pid, which is alive: a lock of the live session (or of another run) is held; nothing was started"
    fi
done
for tool in niri grim dbus-run-session dbus-daemon ss; do
    command -v "$tool" > /dev/null 2>&1 || refuse "$tool is not installed"
done
for binary in celestina celestina-lock celestina-lock-verify; do
    [ -x "$bin_dir/$binary" ] || refuse "$bin_dir/$binary is not an executable"
done
[ -x "$fake_logind" ] || refuse "$fake_logind is not an executable"
case " $steps " in
    *" R-3 "*) command -v swaylock > /dev/null 2>&1 || refuse "swaylock is not installed (R-3)" ;;
esac
case " $steps " in
    *" R-7 "*) command -v wtype > /dev/null 2>&1 || refuse "wtype is not installed (R-7)" ;;
esac
case " $steps " in
    *" R-8 "*) [ -n "$unlock_probe" ] && [ -x "$unlock_probe" ] || refuse "R-8 needs --unlock-probe (built with --features fake-verifier)" ;;
esac
case " $steps " in
    *" R-10 "*)
        command -v wtype > /dev/null 2>&1 || refuse "wtype is not installed (R-10)"
        command -v cmp > /dev/null 2>&1 || refuse "cmp is not installed (R-10)"
        command -v python3 > /dev/null 2>&1 || refuse "python3 is not installed (R-10 reads the capture's size and may generate its picture)"
        case "$r10_monitor" in
            none) ;;
            ''|*[!A-Za-z0-9_-]*) refuse "--monitor wants an output name or none (R-10)" ;;
        esac
        if [ -n "$wallpaper_folder" ]; then
            [ -d "$wallpaper_folder" ] || refuse "--wallpaper-folder $wallpaper_folder is not a directory (R-10)"
            [ -n "$(ls -A -- "$wallpaper_folder" 2> /dev/null)" ] || refuse "--wallpaper-folder $wallpaper_folder is empty (R-10)"
        fi
        ;;
esac
[ -z "$wallpaper_folder" ] || case " $steps " in
    *" R-10 "*) ;;
    *) refuse "--wallpaper-folder is only for R-10" ;;
esac
# The paths are quoted into the nest's command line with single quotes, and
# the wallpaper folder into a JSON string.
for path in "$self" "$bin_dir" "$out" "$wallpaper_folder"; do
    case "$path" in
        *"'"*) refuse "a path holds a single quote: $path" ;;
        *'"'*|*'\'*) refuse "a path holds a double quote or a backslash: $path" ;;
    esac
done

# The steps to run: R-9 is three runs.
plan=
for step in $steps; do
    case "$step" in
        R-9) plan="$plan R-9a R-9b R-9c" ;;
        *) plan="$plan $step" ;;
    esac
done

echo "lock-rehearsal: host display $host_display, runtime $XDG_RUNTIME_DIR"
echo "lock-rehearsal: bin dir $bin_dir, fake logind $fake_logind"
echo "lock-rehearsal: evidence in $out"
echo "lock-rehearsal: plan:$plan"
case " $plan " in
    *" R-10 "*)
        echo "lock-rehearsal: R-10: nest at scale 1.5, window on $r10_monitor, wallpaper ${wallpaper_folder:-generated by python3 in the run's scratch directory}"
        ;;
esac
if [ "$check" = yes ]; then
    echo "lock-rehearsal: --check: refusals passed, nothing started"
    exit 0
fi

mkdir -p "$out"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/celestina-lock-rehearsal.XXXXXX")
summary=$out/task7-summary.txt
: > "$summary"

logind_pid=
nest_launcher=
cleanup_run=

# Stops what this run started, by pid and only that: the nest's niri (read
# from the NIRI_SOCKET the probe recorded, after checking it was started with
# this run's .kdl), then everything the probe tracked.
cleanup() {
    work=$1
    if [ -f "$work/ctl/nest.env" ]; then
        socket=$(sed -n 's/^NIRI_SOCKET=//p' "$work/ctl/nest.env")
        npid=$(printf '%s' "$socket" | sed -n 's/.*niri\.[^.]*\.\([0-9]*\)\.sock$/\1/p')
        if [ -n "$npid" ] && [ -d "/proc/$npid" ] \
            && tr '\0' ' ' < "/proc/$npid/cmdline" 2>/dev/null | grep -qF -- "$work/run.kdl"; then
            echo "lock-rehearsal: the nest (niri $npid) did not end by itself; stopping it by that pid" >&2
            kill -TERM "$npid" 2> /dev/null || true
            i=0
            while [ -d "/proc/$npid" ] && [ "$i" -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
        fi
    fi
    if [ -f "$work/ctl/pids" ]; then
        while read -r pid comm; do
            [ -d "/proc/$pid" ] || continue
            [ "$(cat "/proc/$pid/comm" 2>/dev/null || true)" = "$comm" ] || continue
            kill -KILL "$pid" 2> /dev/null || true
        done < "$work/ctl/pids"
    fi
}

stop_logind() {
    exec 8>&-
    if [ -n "$logind_pid" ]; then
        i=0
        while [ -d "/proc/$logind_pid" ] && [ "$i" -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
        # Closing its input ends it and its dbus-daemon; if it is still there
        # it is stopped by the pid `$!` gave.
        [ ! -d "/proc/$logind_pid" ] || kill -TERM "$logind_pid" 2> /dev/null || true
    fi
    logind_pid=
}

on_signal() {
    echo "lock-rehearsal: interrupted" >&2
    [ -z "$cleanup_run" ] || cleanup "$cleanup_run"
    stop_logind
    exit 130
}
trap on_signal INT TERM

# A bright 960x540 picture (a near-white band, a gradient and a colour field),
# so the lock's scrim has the brightest part of a wallpaper to dim. Standard
# library only; the author's own pictures are never read.
make_wallpaper() { # make_wallpaper DIR
    mkdir -p "$1"
    # `default.png`: the wallpaper duty picks `<output>.<ext>`, then
    # `default.<ext>`, and never "the first file", so a picture under any other
    # name is a fallback (black) and R-10 has nothing to thumbnail (seen on
    # 2026-10-23: the first R-10 ran with `rehearsal.png` and failed that way).
    python3 -I - "$1/default.png" << 'PNG'
import struct, sys, zlib

width, height = 960, 540
rows = bytearray()
for y in range(height):
    rows.append(0)
    for x in range(width):
        if x < width // 3:
            rows += bytes((245, 245, 235))
        elif x < 2 * width // 3:
            rows += bytes((60 + (x - width // 3) * 150 // (width // 3), 90 + y * 120 // height, 200))
        else:
            rows += bytes((200, 40 + y * 150 // height, 60 + (x - 2 * width // 3) * 120 // (width // 3)))

def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

open(sys.argv[1], "wb").write(
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(bytes(rows)))
    + chunk(b"IEND", b"")
)
PNG
}

run_step() {
    step=$1
    attempt=${2:-1}
    slug=$(printf '%s' "$step" | tr 'A-Z' 'a-z' | tr -d '-')
    work=$scratch/$slug
    prefix=$out/task7-$slug
    mkdir -p "$work/ctl" "$work/xdg/celestina" "$work/state"
    cleanup_run=$work
    : > "$work/steps.log"

    shell=0
    idle=no
    emulate=
    limit=150
    case "$step" in
        R-1|R-2|R-3|R-4|R-5|R-6|R-9c) shell=1 ;;
        R-9a|R-9b) shell=1; idle=yes; limit=420; emulate=--emulate-sleep ;;
        R-7|R-8) shell=0 ;;
        R-10) shell=1; limit=360 ;;
    esac
    # The nest's window: R-10 asks for the 4K output at scale 1.5 (the author's
    # monitor); the others leave the window where the host puts it and never
    # touch the host compositor's IPC.
    nest_args="--monitor none"
    if [ "$step" = R-10 ]; then
        nest_args="--scale 1.5 --monitor $r10_monitor"
        folder=$wallpaper_folder
        if [ -z "$folder" ]; then
            folder=$work/wallpapers
            make_wallpaper "$folder"
        fi
        printf '{"schema_version":1,"duties":{"lock":"on","wallpaper":"on"},"wallpaper":{"folder":"%s"}}\n' "$folder" > "$work/xdg/celestina/shell.json"
        echo "R-10 setup: nest scale 1.5, window on $r10_monitor, wallpaper folder $folder" | tee -a "$summary"
    elif [ "$idle" = yes ]; then
        printf '%s\n' '{"schema_version":1,"duties":{"lock":"on","idle":"on"}}' > "$work/xdg/celestina/shell.json"
    else
        printf '%s\n' '{"schema_version":1,"duties":{"lock":"on"}}' > "$work/xdg/celestina/shell.json"
    fi

    # The fake logind, on a private bus, with a command pipe held open here.
    mkfifo "$work/ctl/logind.cmd"
    # shellcheck disable=SC2086 # emulate is one word or none
    "$fake_logind" --log "$work/logind.log" $emulate < "$work/ctl/logind.cmd" > "$work/logind.out" 2>&1 &
    logind_pid=$!
    exec 8> "$work/ctl/logind.cmd"
    i=0
    until grep -q '^DBUS_SYSTEM_BUS_ADDRESS=' "$work/logind.out" 2>/dev/null; do
        i=$((i + 1))
        [ "$i" -lt 100 ] || { stop_logind; refuse "the fake logind did not start"; }
        sleep 0.1
    done
    system_bus=$(sed -n 's/^DBUS_SYSTEM_BUS_ADDRESS=//p' "$work/logind.out")

    echo "lock-rehearsal: $step ..."
    set +e
    (
        LOCK_REHEARSAL_INSIDE=1
        LOCK_REHEARSAL_STEP=$step
        LOCK_REHEARSAL_WORK=$work
        LOCK_REHEARSAL_BIN_DIR=$bin_dir
        LOCK_REHEARSAL_PREFIX=$prefix
        LOCK_REHEARSAL_HOST_DISPLAY=$host_display
        LOCK_REHEARSAL_SHELL=$shell
        LOCK_REHEARSAL_UNLOCK_PROBE=$unlock_probe
        LOCK_REHEARSAL_POINTER_TOOL=$pointer_tool
        LOCK_REHEARSAL_ATTEMPT=$attempt
        XDG_CONFIG_HOME=$work/xdg
        XDG_STATE_HOME=$work/state
        DBUS_SYSTEM_BUS_ADDRESS=$system_bus
        RUST_LOG=info,celestina::lock=debug
        export LOCK_REHEARSAL_INSIDE LOCK_REHEARSAL_STEP LOCK_REHEARSAL_WORK \
            LOCK_REHEARSAL_BIN_DIR LOCK_REHEARSAL_PREFIX LOCK_REHEARSAL_HOST_DISPLAY \
            LOCK_REHEARSAL_SHELL LOCK_REHEARSAL_UNLOCK_PROBE LOCK_REHEARSAL_POINTER_TOOL \
            LOCK_REHEARSAL_ATTEMPT XDG_CONFIG_HOME XDG_STATE_HOME DBUS_SYSTEM_BUS_ADDRESS RUST_LOG
        # `--monitor none`: the nest's window is left where the host puts it
        # and nothing here touches the host compositor's IPC.
        # shellcheck disable=SC2086 # nest_args is a few plain words
        exec timeout "$limit" "$here/dev-session.sh" --own-bus $nest_args \
            --binary "$self" --probe "$self" --once "$work/run" -- --nest-shell \
            8>&- > "$work/nest.log" 2>&1
    ) &
    nest_launcher=$!
    wait "$nest_launcher"
    launcher_status=$?
    set -e
    nest_launcher=
    # The nest closes itself when the wrapper returns; anything left is
    # stopped by pid.
    cleanup "$work"
    stop_logind

    # The evidence, named after the task and the step.
    for part in err out status; do
        [ ! -f "$work/run.$part" ] || cp "$work/run.$part" "$prefix.shell.$part"
    done
    [ ! -f "$work/run.probe" ] || cp "$work/run.probe" "$prefix.probe"
    cp "$work/steps.log" "$prefix.steps.log"
    cp "$work/nest.log" "$prefix.nest.log"
    cp "$work/logind.log" "$prefix.logind.log" 2> /dev/null || true
    # A stale pid file of the nest's own display (a killed lock leaves one).
    if [ -f "$work/ctl/nest.env" ]; then
        display=$(sed -n 's/^WAYLAND_DISPLAY=//p' "$work/ctl/nest.env")
        stale=$runtime_state/lock-$display.state
        if [ "$display" != "$host_display" ] && [ -f "$stale" ]; then
            pid=$(sed -n 's/^pid \([0-9][0-9]*\)$/\1/p' "$stale")
            [ -n "$pid" ] && [ -d "/proc/$pid" ] || rm -f "$stale"
        fi
    fi
    if grep -q '^[0-9]* RESULT [^ ]* RETRY ' "$work/steps.log"; then
        cleanup_run=
        return 75
    fi
    if grep -q '^[0-9]* RESULT ' "$work/steps.log"; then
        grep '^[0-9]* RESULT ' "$work/steps.log" | sed 's/^[0-9]* //' | tee -a "$summary"
    else
        echo "RESULT $step FAIL no result (launcher status $launcher_status; see $prefix.steps.log)" | tee -a "$summary"
    fi
    cleanup_run=
}

# A step that lost a race it cannot win by design (R-4's window) asks for
# another try in a fresh nest, a few times.
for step in $plan; do
    attempt=1
    while :; do
        status=0
        run_step "$step" "$attempt" || status=$?
        [ "$status" -eq 75 ] || break
        attempt=$((attempt + 1))
        if [ "$attempt" -gt 8 ]; then
            echo "RESULT $step FAIL the race was lost in 8 nests" | tee -a "$summary"
            break
        fi
        echo "lock-rehearsal: $step lost its race; trying again (attempt $attempt)"
    done
done
rm -rf "$scratch"
echo "lock-rehearsal: done; summary in $summary"
