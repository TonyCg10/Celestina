#!/bin/sh

set -eu

# dev-session.sh — a nested Niri in a window, to look at Celestina without
# touching the session you are looking at it from.
#
# `dev-restart.sh` already gives a fast build-and-run loop, but it runs the panel
# on whatever session it inherits: on the live desktop that means mapping layer
# surfaces over the shell that currently owns it. This script gives that loop a
# session of its own — a real Niri, in a window, with its own outputs, scale and
# workspaces — so the shell under test maps its panels there and nowhere else.
#
#   dev-session.sh              start the nest and run the shell inside it
#   dev-session.sh --restart    rebuild and restart the shell in the running nest
#   dev-session.sh --shell      open a shell attached to the running nest
#
# The nest is long-lived and `--restart` is the inner loop: leave the window
# open, edit QML or C++, and restart from another terminal in a couple of
# seconds. Ctrl-C in the terminal that started it tears the whole nest down.
#
# What this does NOT do: it never passes `--session` to Niri (which would import
# this nest's environment into systemd and D-Bus globally), never reads or writes
# the author's Niri configuration, and never activates anything on the live
# session.

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
config=$here/dev-session.kdl
restart=$here/dev-restart.sh
runtime=${XDG_RUNTIME_DIR:-/tmp}
env_file=$runtime/celestina-dev-session.env

usage() {
    echo "usage: dev-session.sh [--restart | --shell] [-- shell arguments...]" >&2
}

mode=start
case "${1:-}" in
    --restart) mode=restart; shift ;;
    --shell) mode=shell; shift ;;
    --help|-h) usage; exit 0 ;;
    -*) usage; exit 2 ;;
esac

# The recorded environment of a running nest, or a refusal. Both `--restart` and
# `--shell` need the same two values and the same proof that the nest is alive:
# a stale env file from a nest that has exited would otherwise point a build at a
# socket nobody is listening on.
load_nest() {
    if [ ! -f "$env_file" ]; then
        echo "dev-session: no nested session is recorded at $env_file" >&2
        echo "  start one first with: dev-session.sh" >&2
        exit 1
    fi

    # shellcheck source=/dev/null
    . "$env_file"

    if [ -z "${WAYLAND_DISPLAY:-}" ] || [ -z "${NIRI_SOCKET:-}" ]; then
        echo "dev-session: the recorded session is incomplete; remove $env_file" >&2
        exit 1
    fi

    if [ ! -S "$NIRI_SOCKET" ]; then
        echo "dev-session: the recorded nest is gone (no socket at $NIRI_SOCKET)" >&2
        echo "  it exited without cleaning up; start a new one with: dev-session.sh" >&2
        exit 1
    fi

    export WAYLAND_DISPLAY NIRI_SOCKET
}

case "$mode" in
    restart)
        load_nest
        echo ">> restarting Celestina in the nest on $WAYLAND_DISPLAY" >&2
        exec "$restart" "$@"
        ;;
    shell)
        load_nest
        echo ">> ${SHELL:-/bin/sh} attached to the nest on $WAYLAND_DISPLAY" >&2
        exec "${SHELL:-/bin/sh}"
        ;;
esac

# ── start ────────────────────────────────────────────────────────────────────

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "dev-session: WAYLAND_DISPLAY is empty; a nested Niri needs a host" >&2
    echo "  compositor to open its window in" >&2
    exit 1
fi

if [ ! -x "$restart" ]; then
    echo "dev-session: $restart is missing or not executable" >&2
    exit 1
fi

if [ -f "$env_file" ]; then
    recorded_socket=$(sed -n 's/^NIRI_SOCKET=//p' "$env_file")
    if [ -n "$recorded_socket" ] && [ -S "$recorded_socket" ]; then
        echo "dev-session: a nest is already running on that record." >&2
        echo "  restart the shell inside it with: dev-session.sh --restart" >&2
        exit 1
    fi
    rm -f "$env_file"
fi

# One session bus is shared with the desktop this window opens on, and panel mode
# claims `org.celestina.Shell` unconditionally. A shell already holding it would
# be retired by dev-restart.sh — correct on this machine, surprising in a nested
# window, so it is said out loud before anything is started.
if busctl --user --no-pager status org.celestina.Shell >/dev/null 2>&1; then
    echo "note: org.celestina.Shell already has an owner on the session bus." >&2
    echo "  Starting the nest will retire it; the tray and notification names" >&2
    echo "  are claimed only when free and are left alone either way." >&2
fi

# Niri exports `WAYLAND_DISPLAY` and `NIRI_SOCKET` into whatever it starts, so
# the nest's own child is what records them: nothing here has to guess which
# display number the compositor picked or poll for it to appear.
record_and_run="
    printf 'WAYLAND_DISPLAY=%s\nNIRI_SOCKET=%s\n' \
        \"\$WAYLAND_DISPLAY\" \"\$NIRI_SOCKET\" > '$env_file'
    exec '$restart' \"\$@\"
"

trap 'rm -f "$env_file"' EXIT INT TERM

echo ">> nested Niri: $config" >&2
echo ">> the shell runs inside it; Ctrl-C here stops the whole nest" >&2
exec niri -c "$config" -- /bin/sh -c "$record_and_run" dev-session "$@"
