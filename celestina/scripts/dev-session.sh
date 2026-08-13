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
#   dev-session.sh --own-bus    the same, on a session bus of the nest's own
#   dev-session.sh --restart    rebuild and restart the shell in the running nest
#   dev-session.sh --shell      open a shell attached to the running nest
#
# The nest reproduces the author's own working monitor by default: its window is
# placed fullscreen on `DP-1`, the 3840x2160 output, and the nested `winit`
# output is given scale 1.5 — the same factor that monitor really runs at. A
# nest at scale 1 on a 1080p window proves nothing about a shell whose geometry
# is divided by the output factor. `--scale` and `--monitor` change or disable
# both halves for a deliberate second case.
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
# The configuration Niri is actually given: this file's own, with the nested
# output's scale substituted. Niri reads no environment, so a scale that the
# caller chooses has to reach it as bytes.
generated_config=$runtime/celestina-dev-session.kdl

# The author's 4K monitor and the factor it really runs at.
scale=1.5
monitor=DP-1
# Whether the nest gets a session bus of its own.
#
# By default it shares the desktop's, which is what makes the tray, MPRIS and
# the shell's own name behave as they really do. The cost is every well-known
# name the live session already owns: the notification server is claimed with
# `DoNotQueue` and no `ReplaceExisting` — deliberately, a shell either is the
# session's server or is not one — so while Noctalia holds
# `org.freedesktop.Notifications`, nothing sent with `notify-send` can reach
# the nested shell, and its toasts cannot be looked at at all. On its own bus
# the nested shell claims the name unopposed and `notify-send` from inside the
# nest raises a real toast there. Nothing on the desktop's bus is touched
# either way, and the trade is the other direction: no live tray items, no
# MPRIS players and no portal in the nest.
own_bus=no

usage() {
    echo "usage: dev-session.sh [--restart | --shell]" >&2
    echo "                      [--own-bus]" >&2
    echo "                      [--scale FACTOR] [--monitor NAME|none]" >&2
    echo "                      [-- shell arguments...]" >&2
}

mode=start
case "${1:-}" in
    --restart) mode=restart; shift ;;
    --shell) mode=shell; shift ;;
esac

while [ "$#" -gt 0 ]; do
    case "$1" in
        --scale) [ "$#" -ge 2 ] || { usage; exit 2; }; scale=$2; shift 2 ;;
        --monitor) [ "$#" -ge 2 ] || { usage; exit 2; }; monitor=$2; shift 2 ;;
        --own-bus) own_bus=yes; shift ;;
        --help|-h) usage; exit 0 ;;
        --) shift; break ;;
        -*) usage; exit 2 ;;
        *) break ;;
    esac
done

case "$scale" in
    ''|*[!0-9.]*|*.*.*) echo "dev-session: --scale wants a number" >&2; exit 2 ;;
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

    # A nest on its own bus recorded that address, and a shell or a restart
    # that ignored it would talk to the desktop's bus while looking at the
    # nest's windows — which is exactly the confusion this record exists to
    # prevent. A nest sharing the desktop's bus records the same value the
    # caller already has, so exporting it unconditionally changes nothing.
    export WAYLAND_DISPLAY NIRI_SOCKET
    [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && export DBUS_SESSION_BUS_ADDRESS
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

# The nested output's scale is the one number in the configuration a caller
# chooses, so the file Niri reads is generated rather than edited in place: the
# checked-in `dev-session.kdl` stays the reference profile and the author's
# configuration is still never touched. `awk` rather than a blind `sed` on
# `scale`, because three paper monitors below declare one too and only the
# `winit` block is ours to change.
awk -v factor="$scale" '
    /^output "winit"/ { inside = 1 }
    inside && /^[[:space:]]*scale / { sub(/scale .*/, "scale " factor) }
    inside && /^}/ { inside = 0 }
    { print }
' "$config" > "$generated_config"

if ! grep -q "scale $scale" "$generated_config"; then
    echo "dev-session: could not set the nested output's scale to $scale" >&2
    exit 1
fi

# Where the nest's own window goes on the live session. Fullscreen on the real
# 4K output is what makes the nested surface 3840x2160, and only then does a
# scale of 1.5 divide the geometry the way that monitor really does. The
# placement runs against the *host* socket, captured here before Niri replaces
# it for its children, and it never touches any window but the one this script
# just created: the placer waits for a `niri` window that was not already open.
place_nest_window() {
    # A host that exports no socket still answers through WAYLAND_DISPLAY.
    host_socket=${NIRI_SOCKET:-}
    before=$(niri msg -j windows 2>/dev/null || echo '[]')

    (
        if [ -n "$host_socket" ]; then
            NIRI_SOCKET=$host_socket
            export NIRI_SOCKET
        fi
        attempt=0
        while [ "$attempt" -lt 100 ]; do
            attempt=$((attempt + 1))
            sleep 0.2
            id=$(niri msg -j windows 2>/dev/null | python3 -c '
import json, sys

before = {w["id"] for w in json.loads(sys.argv[1])}
fresh = [w["id"] for w in json.load(sys.stdin)
         if w.get("app_id") == "niri" and w["id"] not in before]
print(fresh[0] if fresh else "")
' "$before" 2>/dev/null) || continue
            [ -n "$id" ] || continue
            niri msg action move-window-to-monitor --id "$id" "$monitor" \
                >/dev/null 2>&1 || true
            niri msg action fullscreen-window --id "$id" >/dev/null 2>&1 || true
            exit 0
        done
        echo "dev-session: the nest window never appeared; it was left where" >&2
        echo "  the host put it" >&2
    ) &
}

if [ "$monitor" != "none" ]; then
    if niri msg -j outputs 2>/dev/null | grep -q "\"$monitor\""; then
        echo ">> the nest window goes fullscreen on $monitor at scale $scale" >&2
        place_nest_window
    else
        echo "dev-session: the host session has no output named $monitor;" >&2
        echo "  the nest window is left where the host puts it" >&2
    fi
fi

# Niri exports `WAYLAND_DISPLAY` and `NIRI_SOCKET` into whatever it starts, so
# the nest's own child is what records them: nothing here has to guess which
# display number the compositor picked or poll for it to appear.
record_and_run="
    printf 'WAYLAND_DISPLAY=%s\nNIRI_SOCKET=%s\nDBUS_SESSION_BUS_ADDRESS=%s\n' \
        \"\$WAYLAND_DISPLAY\" \"\$NIRI_SOCKET\" \
        \"\$DBUS_SESSION_BUS_ADDRESS\" > '$env_file'
    exec '$restart' \"\$@\"
"

trap 'rm -f "$env_file"' EXIT INT TERM

echo ">> nested Niri: $config" >&2
echo ">> the shell runs inside it; Ctrl-C here stops the whole nest" >&2

if [ "$own_bus" = yes ]; then
    echo ">> on a session bus of its own: the nested shell owns the" >&2
    echo "   notification server, and only what runs inside the nest can" >&2
    echo "   reach it (dev-session.sh --shell, then notify-send)" >&2
    exec dbus-run-session -- \
        niri -c "$generated_config" -- /bin/sh -c "$record_and_run" dev-session "$@"
fi

exec niri -c "$generated_config" -- /bin/sh -c "$record_and_run" dev-session "$@"
