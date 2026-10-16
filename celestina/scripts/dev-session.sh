#!/bin/sh

set -eu

# Copied from celestina-qt/scripts/dev-session.sh at
# c7fb0b5d8a48ace14d3f7e1184aa4f780d90f413 (salvage by copy, AGENTS.md
# "Halted projects"), adapted for the Rust shell: the nest runs the
# `celestina` binary itself instead of the halted shell's `dev-restart.sh`
# (there is no `--restart`; quit the shell with `celestina quit` from
# `--shell` and start it again with `niri msg action spawn`), `--binary`
# names the binary, `--once` runs it once and closes the nest (the smoke's
# mode), and the strength-override strip follows this profile's one layer
# rule.
#
# dev-session.sh — a nested niri in a window, to look at Celestina without
# touching the session you are looking at it from.
#
#   dev-session.sh              start the nest and run the shell inside it
#   dev-session.sh --own-bus    the same, on a session bus of the nest's own
#   dev-session.sh --shell      open a shell attached to the running nest
#   dev-session.sh --once PREFIX ... -- ARGS
#                               run the shell once with ARGS, its stdout,
#                               stderr and exit status written to
#                               PREFIX.out, PREFIX.err and PREFIX.status,
#                               then close the nest
#   --probe SCRIPT              with --once: run SCRIPT inside the nest beside
#                               the shell (same bus and display), its output
#                               and status in PREFIX.probe and
#                               PREFIX.probe.status
#
# The nest reproduces the author's own working monitor by default: its window
# is placed fullscreen on `DP-1`, the 3840x2160 output, and the nested `winit`
# output is given scale 1.5, the factor that monitor really runs at. `--scale`
# and `--monitor` change or disable both halves for a deliberate second case.
#
# What this does NOT do: it never passes `--session` to niri (which would
# import this nest's environment into systemd and D-Bus globally), never reads
# or writes the author's niri configuration, and never activates anything on
# the live session.

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_root=$(CDPATH= cd -- "$here/.." && pwd)
# CELESTINA_NEST_CONFIG names a different profile, for comparing compositor
# behaviour without editing the reference one.
config=${CELESTINA_NEST_CONFIG:-$here/dev-session.kdl}
runtime=${XDG_RUNTIME_DIR:-/tmp}
env_file=$runtime/celestina-dev-session.env

# The author's 4K monitor and the factor it really runs at.
scale=1.5
monitor=DP-1
# Whether the nest gets a session bus of its own. By default it shares the
# desktop's, which is what makes the tray and the shell's own name behave as
# they really do; the cost is every well-known name the live session already
# owns (a shell that finds `org.celestina.Shell` taken exits at once). On its
# own bus the nested shell claims its names unopposed, and nothing on the
# desktop's bus is touched.
own_bus=no
binary=$project_root/target/release/celestina
once=
probe=

usage() {
    echo "usage: dev-session.sh [--shell]" >&2
    echo "                      [--own-bus] [--binary PATH] [--once PREFIX [--probe SCRIPT]]" >&2
    echo "                      [--scale FACTOR] [--monitor NAME|none]" >&2
    echo "                      [-- shell arguments...]" >&2
}

mode=start
case "${1:-}" in
    --shell) mode=shell; shift ;;
esac

while [ "$#" -gt 0 ]; do
    case "$1" in
        --scale) [ "$#" -ge 2 ] || { usage; exit 2; }; scale=$2; shift 2 ;;
        --monitor) [ "$#" -ge 2 ] || { usage; exit 2; }; monitor=$2; shift 2 ;;
        --binary) [ "$#" -ge 2 ] || { usage; exit 2; }; binary=$2; shift 2 ;;
        --probe) [ "$#" -ge 2 ] || { usage; exit 2; }; probe=$2; shift 2 ;;
        --once) [ "$#" -ge 2 ] || { usage; exit 2; }; once=$2; shift 2 ;;
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

# The recorded environment of a running nest, or a refusal: a stale env file
# from a nest that has exited would otherwise point a shell at a socket
# nobody is listening on.
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

    # A nest on its own bus recorded that address; a nest sharing the
    # desktop's records the value the caller already has.
    export WAYLAND_DISPLAY NIRI_SOCKET
    [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] && export DBUS_SESSION_BUS_ADDRESS
}

if [ "$mode" = shell ]; then
    load_nest
    echo ">> ${SHELL:-/bin/sh} attached to the nest on $WAYLAND_DISPLAY" >&2
    exec "${SHELL:-/bin/sh}"
fi

# ── start ────────────────────────────────────────────────────────────────────

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "dev-session: WAYLAND_DISPLAY is empty; a nested niri needs a host" >&2
    echo "  compositor to open its window in" >&2
    exit 1
fi

if [ ! -x "$binary" ]; then
    echo "dev-session: $binary is not an executable; build it with" >&2
    echo "  (cd celestina && cargo build --release --locked)" >&2
    exit 1
fi

# A run with --once is not the long-lived nest: it records nothing, so it
# can run beside one.
if [ -z "$once" ] && [ -f "$env_file" ]; then
    recorded_socket=$(sed -n 's/^NIRI_SOCKET=//p' "$env_file")
    if [ -n "$recorded_socket" ] && [ -S "$recorded_socket" ]; then
        echo "dev-session: a nest is already running on that record." >&2
        echo "  attach to it with: dev-session.sh --shell" >&2
        exit 1
    fi
    rm -f "$env_file"
fi

# The configuration niri is actually given: the profile with the nested
# output's scale substituted. niri reads no environment, so a scale the
# caller chooses has to reach it as bytes. A `--once` run keeps it beside its
# output.
generated_config=${once:-$runtime/celestina-dev-session}.kdl

# `awk` rather than a blind `sed` on `scale`, because the paper monitors
# declare one too and only the `winit` block is ours to change.
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

# Where the nest's own window goes on the live session: fullscreen on the
# real 4K output, so a scale of 1.5 divides the geometry the way that monitor
# really does. The placement runs against the *host* socket, captured here
# before niri replaces it for its children, and touches only a `niri` window
# that was not already open.
place_nest_window() {
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

# A nest may run a different niri than the session. CELESTINA_NEST_NIRI names
# one outright; otherwise the patched build is preferred when it is there.
niri_binary=${CELESTINA_NEST_NIRI:-}
if [ -z "$niri_binary" ]; then
    patched=${CELESTINA_NIRI_PREFIX:-$HOME/.local/lib/celestina}/niri
    if [ -x "$patched" ]; then
        niri_binary=$patched
    else
        niri_binary=niri
    fi
fi

# niri exports `WAYLAND_DISPLAY` and `NIRI_SOCKET` into whatever it starts,
# so the nest's own child is what records them (the long-lived nest) or runs
# the shell once and closes the nest (`--once`).
if [ -n "$once" ]; then
    probe_start=
    probe_wait=
    if [ -n "$probe" ]; then
        probe_start="
        '$probe' > '$once.probe' 2>&1 &
        probe_pid=\$!"
        probe_wait="
        probe_status=0
        wait \"\$probe_pid\" || probe_status=\$?
        printf '%s\n' \"\$probe_status\" > '$once.probe.status'"
    fi
    inside="$probe_start
        '$binary' \"\$@\" > '$once.out' 2> '$once.err'
        printf '%s\n' \"\$?\" > '$once.status'$probe_wait
        exec '$niri_binary' msg action quit --skip-confirmation
    "
else
    inside="
        printf 'WAYLAND_DISPLAY=%s\nNIRI_SOCKET=%s\nDBUS_SESSION_BUS_ADDRESS=%s\n' \
            \"\$WAYLAND_DISPLAY\" \"\$NIRI_SOCKET\" \
            \"\$DBUS_SESSION_BUS_ADDRESS\" > '$env_file'
        exec '$binary' \"\$@\"
    "
    trap 'rm -f "$env_file"' EXIT INT TERM
fi

# A per-layer blur strength is the one thing a profile may carry that a
# stock niri rejects, and a rejected profile is a nest that does not start.
# The generated profile is offered to the very binary that will run it and,
# if it is refused, the strength lines inside the shell's layer rule are
# dropped and the rest is kept.
if ! "$niri_binary" validate -c "$generated_config" >/dev/null 2>&1; then
    if command -v python3 >/dev/null 2>&1; then
        python3 - "$generated_config" <<'STRIP'
import re, sys

# Only inside the shell's layer rule: the global `blur` block names the very
# same words and every niri accepts them there.
path = sys.argv[1]
text = open(path).read()
marker = 'match namespace="^celestina-(panel|overlay|toasts)$"'
at = text.find(marker)
if at >= 0:
    end = text.find("\n}", at)
    end = len(text) if end < 0 else end
    head, rule, tail = text[:at], text[at:end], text[end:]
    rule = re.sub(r"^[ \t]*(passes|offset|noise|saturation)[ \t]+[0-9.]+[ \t]*\n",
                  "", rule, flags=re.M)
    text = head + rule + tail
open(path, "w").write(text)
STRIP
    fi
    if ! "$niri_binary" validate -c "$generated_config" >/dev/null 2>&1; then
        echo "dev-session: $niri_binary refuses $generated_config:" >&2
        "$niri_binary" validate -c "$generated_config" >&2 || true
        exit 1
    fi
    echo ">> this niri has no per-layer blur strength; the shell's glass" >&2
    echo "   keeps the global one" >&2
fi

echo ">> nested niri ($niri_binary): $config" >&2
if [ -z "$once" ]; then
    echo ">> the shell runs inside it; Ctrl-C here stops the whole nest" >&2
fi

if [ "$own_bus" = yes ]; then
    exec dbus-run-session -- \
        "$niri_binary" -c "$generated_config" -- /bin/sh -c "$inside" dev-session "$@"
fi
exec "$niri_binary" -c "$generated_config" -- /bin/sh -c "$inside" dev-session "$@"
