#!/bin/sh
set -u

# Calcita smoke: the quick gate without a visible window.
#
#  1) The shared static check for `x: x` auto-bindings, where an injected
#     property shadows the id, resolves to itself and stays undefined. The
#     engine and qmllint both accept it, so it is caught by pattern.
#  2) An 8-second offscreen start in a scratch home with no session bus: the
#     binary must still be running when the timeout ends it (status 124) and
#     the QML runtime must report no errors, including the ones that mean an
#     object never constructed.
#  3) The window's own report (CALCITA_SMOKE_REPORT) after 3 s: the empty state
#     is shown and the appearance reached the theme.
#  4) The same start with tests/fixtures/three-pages.pdf on the command line:
#     a document window opens, QtPdf reads three pages and the empty window
#     has retired.
#
# Startup only. Keyboard, focus and assistive technology need a real Wayland
# session.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=$root/target/release/calcita
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

fixture=$root/tests/fixtures/three-pages.pdf
if [ ! -f "$fixture" ]; then
    echo "smoke: the fixture is missing: $fixture" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM

# One 8-second offscreen start in a fresh scratch home, with the arguments
# given; the window's last report is left in $report.
run() {
    rm -rf "$scratch/home"
    mkdir -p "$scratch/home/config" "$scratch/home/data" "$scratch/home/cache" \
        "$scratch/home/state" "$scratch/home/run"
    chmod 0700 "$scratch/home/run"
    log=$scratch/output.log

    XDG_CONFIG_HOME=$scratch/home/config \
    XDG_DATA_HOME=$scratch/home/data \
    XDG_CACHE_HOME=$scratch/home/cache \
    XDG_STATE_HOME=$scratch/home/state \
    XDG_RUNTIME_DIR=$scratch/home/run \
    DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/home/run/no-session-bus \
    CALCITA_FAKE=1 \
    CALCITA_SMOKE_REPORT=1 \
    QT_QPA_PLATFORM=offscreen \
    QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        timeout 8 "$bin" "$@" >"$log" 2>&1
    rc=$?
    if [ "$rc" -ne 124 ]; then
        echo "smoke: the binary exited on its own (rc=$rc); last lines:" >&2
        tail -20 "$log" >&2
        exit 1
    fi

    errors=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Binding loop detected|QQmlApplicationEngine failed|QML [A-Za-z]*: ' "$log" || true)
    if [ -n "$errors" ]; then
        echo "smoke: QML errors at startup:" >&2
        echo "$errors" | sort | uniq -c | sort -rn >&2
        exit 1
    fi

    report=$(grep -o 'calcita-smoke:.*' "$log" | tail -1)
}

run
case $report in
    *" empty=true pageCount=0 textScale=1 fontBody=13") ;;
    *)
        echo "smoke: the window did not report the empty state: ${report:-no report}" >&2
        exit 1
        ;;
esac
empty_report=$report

run "$fixture"
case $report in
    *" empty=false pageCount=3 textScale=1 fontBody=13") ;;
    *)
        echo "smoke: the fixture did not open with three pages: ${report:-no report}" >&2
        exit 1
        ;;
esac

echo "smoke: OK — binary alive for 8 s twice, no QML errors, no auto-bindings ($empty_report; $report)"
