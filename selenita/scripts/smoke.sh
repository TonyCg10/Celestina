#!/bin/sh
set -u

# Selenita smoke: the quick gate without a visible window.
#
#  1) The shared static check for `x: x` auto-bindings, where an injected
#     property shadows the id, resolves to itself and stays undefined. The
#     engine and qmllint both accept it, so it is caught by pattern.
#  2) An 8-second offscreen start in a scratch home with no session bus: the
#     binary must still be running when the timeout ends it (status 124) and
#     the QML runtime must report no errors, including the ones that mean an
#     object never constructed.
#  3) Under SELENITA_FAKE=1 the window takes one capture through the report
#     switch (SELENITA_SMOKE_REPORT) after 1 s, starts a fake recording at
#     1.5 s and stops it at 2.5 s, and reports after 4 s: the three cards are
#     shown, the history holds the capture and the recording, the recording
#     is idle again with its last file named, and the appearance reached the
#     theme. The fake's 1×1 PNG must be in the scratch pictures folder's
#     `Capturas`, the fake MP4 in the scratch videos folder's `Recordings`
#     (never in its root), and both lines in the scratch history.
#
# Startup only. Keyboard, focus and assistive technology need a real Wayland
# session.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=$root/target/release/selenita
scanner=$root/../scripts/architecture_scanners.py
if [ "${1:-}" = "--binary" ]; then
    shift
    bin=${1:?--binary requires a path}
    shift
fi
if [ "$#" -ne 0 ]; then
    echo "usage: scripts/smoke.sh [--binary PATH]" >&2
    exit 2
fi

if ! autos=$(python3 "$scanner" qml-auto-bindings "$root/qml"); then
    echo "smoke: the auto-binding scanner could not complete" >&2
    exit 1
fi
if [ -n "$autos" ]; then
    echo "smoke: auto-binding 'x: x' (the property shadows the id):" >&2
    echo "$autos" >&2
    exit 1
fi

if [ ! -x "$bin" ]; then
    echo "smoke: the binary is missing: $bin" >&2
    exit 1
fi

# One run as a plain launch (the window takes a capture through the report
# switch), one as `--screenshot screen` with no instance running (the window
# starts hidden, takes the capture, then shows): both end shown with one
# capture.
smoke_run() {
    # Whether the window is on screen when it starts: shown for a plain
    # launch, hidden for a launch that captures first.
    start_shown=$1
    shift
    scratch=$(mktemp -d)
    mkdir -p "$scratch/config" "$scratch/data" "$scratch/cache" \
        "$scratch/state" "$scratch/run" "$scratch/pictures" "$scratch/videos"
    chmod 0700 "$scratch/run"
    log=$scratch/output.log

    XDG_CONFIG_HOME=$scratch/config \
    XDG_DATA_HOME=$scratch/data \
    XDG_CACHE_HOME=$scratch/cache \
    XDG_STATE_HOME=$scratch/state \
    XDG_RUNTIME_DIR=$scratch/run \
    XDG_PICTURES_DIR=$scratch/pictures \
    XDG_VIDEOS_DIR=$scratch/videos \
    DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
    SELENITA_FAKE=1 \
    SELENITA_SMOKE_REPORT=1 \
    QT_QPA_PLATFORM=offscreen \
    QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        timeout 8 "$bin" "$@" >"$log" 2>&1
    rc=$?
    if [ "$rc" -ne 124 ]; then
        echo "smoke: the binary exited on its own (rc=$rc); last lines:" >&2
        tail -20 "$log" >&2
        return 1
    fi

    errors=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Binding loop detected|QQmlApplicationEngine failed' "$log" || true)
    if [ -n "$errors" ]; then
        echo "smoke: QML errors at startup:" >&2
        echo "$errors" | sort | uniq -c | sort -rn >&2
        return 1
    fi

    if ! grep -q "selenita-smoke-start: shown=$start_shown" "$log"; then
        echo "smoke: the window did not start shown=$start_shown ($*)" >&2
        return 1
    fi

    report=$(grep -o 'selenita-smoke:.*' "$log" | tail -1)
    case $report in
        *" cards=3 history=2 recording=idle lastRecording=true shown=true fake=true textScale=1 fontBody=13") ;;
        *)
            echo "smoke: the window did not report its cards, one capture and one recording: ${report:-no report}" >&2
            return 1
            ;;
    esac

    shots=$(find "$scratch/pictures" -name '*.png' | wc -l)
    if [ "$shots" -ne 1 ]; then
        echo "smoke: expected one fake capture in the pictures folder, found $shots" >&2
        return 1
    fi
    clips=$(find "$scratch/videos/Recordings" -name '*.mp4' 2>/dev/null | wc -l)
    if [ "$clips" -ne 1 ]; then
        echo "smoke: expected one fake recording in the videos folder's Recordings, found $clips" >&2
        return 1
    fi
    loose=$(find "$scratch/videos" -maxdepth 1 -name '*.mp4' | wc -l)
    if [ "$loose" -ne 0 ]; then
        echo "smoke: a recording landed in the videos root" >&2
        return 1
    fi
    if [ "$(wc -l < "$scratch/data/selenita/history" 2>/dev/null || echo 0)" -ne 2 ]; then
        echo "smoke: the history file does not hold the capture and the recording" >&2
        return 1
    fi

    rm -rf "$scratch"
}

scratch=
trap 'rm -rf "${scratch:-}"' EXIT HUP INT TERM
smoke_run true || exit 1
smoke_run false --screenshot screen || exit 1

echo "smoke: OK — binary alive for 8 s, no QML errors, no auto-bindings, one fake capture and one fake recording in the history ($report)"
