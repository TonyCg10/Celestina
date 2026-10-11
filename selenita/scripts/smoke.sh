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
#     is idle again with its last file named, the corner preview shows the
#     latest result (its window is still up for its five seconds, so the
#     process outlives its first preview) and the appearance reached the
#     theme. The fake's 1×1 PNG must be in the scratch pictures folder's
#     `Capturas`, the fake MP4 in the scratch videos folder's `Grabaciones`
#     (never in its root, and no `Recordings` made), both lines in the
#     scratch history and the fake recording's poster in the scratch
#     runtime folder's `selenita`. The window's capture takes every output
#     (the card chose none) and the `--screenshot screen` launch's takes the
#     fake's focused output, `FAKE-1`. The plain launch must still run at 8 s; the `--screenshot` launch, whose
#     main window never shows, dismisses its preview at 5 s as the × does
#     (offscreen, the pointer rests on the preview and holds its timer) and
#     must then end by itself, within 14 s.
#     In both runs the recording bar shows at 2 s, while the fake recording
#     runs, titled «Grabando», and is hidden again in the report (SEL-4-A).
#  4) A plain launch that closes its window at 2 s (SELENITA_SMOKE_CLOSE),
#     while the capture's preview shows and a fake recording runs, must end
#     by itself (the preview and the bar may not keep it alive) with the
#     recording finished: the PNG, the MP4 and both history lines are there.
#  5) A `--record-focused` launch (SELENITA_SMOKE_DISCARD) records over the
#     fakes with its main window hidden: the fake niri names `FAKE-1`, the
#     worker leaves the pick request and the fake portal's chooser takes it
#     without its dialog; at 1.5 s the bar is the only window, and its
#     discard ends the recording and the process by itself, within 8 s,
#     leaving no MP4 (nor a hidden one), no history line, no preview and no
#     request.
#  6) `--pick-output`, run as the portal runs its chooser and on a Qt
#     platform that does not exist (it must not start Qt): a fresh request
#     prints `Monitor: FAKE-1` and is consumed; with none, and with a stale
#     one (removed), the fallback in SELENITA_FALLBACK_CHOOSER answers
#     `Monitor: FAKE-2` with its own status.
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
# switch), one as `--screenshot screen` with no instance running (the main
# window starts hidden and stays hidden: only the preview shows): both end
# with one capture, one recording and the preview shown.
smoke_run() {
    # Whether the main window is on screen when it starts and when it
    # reports: shown for a plain launch, hidden throughout for a launch
    # that captures from a key binding.
    start_shown=$1
    end_shown=$2
    # How long the run may take and how it must end: 124 (still running,
    # ended by the timeout) or 0 (ended by itself).
    seconds=$3
    expected_rc=$4
    # The output the screen capture took, as the fake says it.
    grabbed=$5
    shift 5
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
        timeout "$seconds" "$bin" "$@" >"$log" 2>&1
    rc=$?
    if [ "$rc" -ne "$expected_rc" ]; then
        echo "smoke: the binary ended with rc=$rc, not $expected_rc ($*); last lines:" >&2
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
        *" cards=3 history=2 recording=idle lastRecording=true shown=$end_shown preview=shown bar=hidden fake=true textScale=1 fontBody=13") ;;
        *)
            echo "smoke: the window did not report its cards, one capture, one recording, shown=$end_shown and the preview: ${report:-no report}" >&2
            return 1
            ;;
    esac

    if ! grep -q "selenita-smoke-bar: recording=recording bar=shown title=Grabando" "$log"; then
        echo "smoke: the recording bar did not show while the fake recording ran ($*)" >&2
        grep 'selenita-smoke-bar' "$log" >&2
        return 1
    fi

    if ! grep -q "selenita-fake: grab screen ($grabbed) to " "$log"; then
        echo "smoke: the screen capture did not take $grabbed ($*)" >&2
        return 1
    fi

    shots=$(find "$scratch/pictures" -name '*.png' | wc -l)
    if [ "$shots" -ne 1 ]; then
        echo "smoke: expected one fake capture in the pictures folder, found $shots" >&2
        return 1
    fi
    clips=$(find "$scratch/videos/Grabaciones" -name '*.mp4' 2>/dev/null | wc -l)
    if [ "$clips" -ne 1 ]; then
        echo "smoke: expected one fake recording in the videos folder's Grabaciones, found $clips" >&2
        return 1
    fi
    if [ -e "$scratch/videos/Recordings" ]; then
        echo "smoke: the old Recordings folder was made" >&2
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
    posters=$(find "$scratch/run/selenita" -name 'poster-*.png' 2>/dev/null | wc -l)
    if [ "$posters" -ne 1 ]; then
        echo "smoke: expected the fake recording's one poster in the runtime folder, found $posters" >&2
        return 1
    fi

    rm -rf "$scratch"
}

scratch=
trap 'rm -rf "${scratch:-}"' EXIT HUP INT TERM
# The window closed while the preview shows ends the process, and a
# recording under way is finished on the way out.
smoke_close() {
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
    SELENITA_SMOKE_CLOSE=1 \
    QT_QPA_PLATFORM=offscreen \
    QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        timeout 8 "$bin" >"$log" 2>&1
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "smoke: closing the window with the preview shown did not end Selenita (rc=$rc); last lines:" >&2
        tail -20 "$log" >&2
        return 1
    fi
    shots=$(find "$scratch/pictures" -name '*.png' | wc -l)
    clips=$(find "$scratch/videos/Grabaciones" -name '*.mp4' 2>/dev/null | wc -l)
    lines=$(wc -l < "$scratch/data/selenita/history" 2>/dev/null || echo 0)
    if [ "$shots" -ne 1 ] || [ "$clips" -ne 1 ] || [ "$lines" -ne 2 ]; then
        echo "smoke: after the close: $shots capture(s), $clips recording(s), $lines history line(s); expected 1, 1, 2" >&2
        return 1
    fi
    rm -rf "$scratch"
}

# A `--record-focused` launch records the fake's focused output without the
# chooser's dialog, shows only the bar and, discarded from it, ends with no
# file, no history line and no request left.
smoke_discard() {
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
    SELENITA_SMOKE_DISCARD=1 \
    QT_QPA_PLATFORM=offscreen \
    QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        timeout 8 "$bin" --record-focused >"$log" 2>&1
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "smoke: the discarded --record-focused launch did not end by itself (rc=$rc); last lines:" >&2
        tail -20 "$log" >&2
        return 1
    fi
    errors=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot assign|Unable to assign|is not a type|Binding loop detected|QQmlApplicationEngine failed' "$log" || true)
    if [ -n "$errors" ]; then
        echo "smoke: QML errors in the discard run:" >&2
        echo "$errors" >&2
        return 1
    fi
    if ! grep -q "selenita-smoke-discard: recording=recording bar=shown shown=false preview=hidden" "$log"; then
        echo "smoke: the bar was not the only window while the focused recording ran: $(grep 'selenita-smoke-discard' "$log")" >&2
        return 1
    fi
    if ! grep -q "selenita: recording: the chooser is asked for FAKE-1" "$log" \
        || ! grep -q "selenita-fake: the chooser took FAKE-1 without its dialog" "$log"; then
        echo "smoke: the focused start did not hand FAKE-1 to the chooser; last lines:" >&2
        tail -20 "$log" >&2
        return 1
    fi
    clips=$(find "$scratch/videos" -name '*.mp4' | wc -l)
    if [ "$clips" -ne 0 ]; then
        echo "smoke: the discarded recording left $clips MP4 file(s)" >&2
        return 1
    fi
    if [ "$(wc -l < "$scratch/data/selenita/history" 2>/dev/null || echo 0)" -ne 0 ]; then
        echo "smoke: the discarded recording has a history line" >&2
        return 1
    fi
    if [ -e "$scratch/run/selenita/pick-output" ]; then
        echo "smoke: the pick request was left behind" >&2
        return 1
    fi
    rm -rf "$scratch"
}

# `--pick-output` as the portal runs it: answered from a fresh request, else
# by the fallback chooser, and never through Qt (the platform named here
# does not exist, so a Qt start would abort).
smoke_pick() {
    scratch=$(mktemp -d)
    mkdir -p "$scratch/run/selenita"
    chmod 0700 "$scratch/run" "$scratch/run/selenita"
    request=$scratch/run/selenita/pick-output
    pick() {
        XDG_RUNTIME_DIR=$scratch/run \
        DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
        SELENITA_FALLBACK_CHOOSER='echo Monitor: FAKE-2' \
        QT_QPA_PLATFORM=selenita-no-qt \
            timeout 5 "$bin" --pick-output </dev/null 2>"$scratch/pick.err"
    }
    printf 'output=FAKE-1\nwritten=%s\n' "$(date +%s%3N)" > "$request"
    answer=$(pick)
    rc=$?
    if [ "$rc" -ne 0 ] || [ "$answer" != "Monitor: FAKE-1" ]; then
        echo "smoke: --pick-output with a fresh request answered '$answer' (rc=$rc)" >&2
        cat "$scratch/pick.err" >&2
        return 1
    fi
    if [ -e "$request" ]; then
        echo "smoke: --pick-output did not consume the request" >&2
        return 1
    fi
    answer=$(pick)
    rc=$?
    if [ "$rc" -ne 0 ] || [ "$answer" != "Monitor: FAKE-2" ]; then
        echo "smoke: --pick-output without a request answered '$answer' (rc=$rc), not the fallback's" >&2
        cat "$scratch/pick.err" >&2
        return 1
    fi
    printf 'output=FAKE-1\nwritten=1000\n' > "$request"
    answer=$(pick)
    rc=$?
    if [ "$rc" -ne 0 ] || [ "$answer" != "Monitor: FAKE-2" ] || [ -e "$request" ]; then
        echo "smoke: --pick-output with a stale request answered '$answer' (rc=$rc) or left it" >&2
        cat "$scratch/pick.err" >&2
        return 1
    fi
    rm -rf "$scratch"
}

smoke_run true true 8 124 "every output" || exit 1
smoke_run false false 14 0 FAKE-1 --screenshot screen || exit 1
smoke_close || exit 1
smoke_discard || exit 1
smoke_pick || exit 1

echo "smoke: OK — binary alive for 8 s, no QML errors, no auto-bindings, one fake capture and one fake recording in the history, the preview shown, the main window hidden after a key-binding launch that ends with its preview and took the focused output, the window's close ending Selenita with its recording finished, the recording bar shown while recording, a --record-focused launch handing FAKE-1 to the chooser and discarded from its bar with no file left, --pick-output answering a fresh request and falling back otherwise ($report)"
