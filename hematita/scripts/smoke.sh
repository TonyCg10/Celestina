#!/bin/sh
set -u

# Hematita smoke: the fast gate without a window.
#
#  1) The shared static check for the `x: x` auto-binding: an injected
#     property that shadows the id resolves to itself and stays undefined.
#     Legal for the engine and for qmllint, so it is caught by pattern.
#  2) A 10-second offscreen start: the binary must still be alive (timeout
#     answers 124) and the QML runtime must not report errors. Looking only
#     for TypeError/ReferenceError would let the worst case through — an
#     object that does not construct — so "Cannot create delegate" and its
#     relatives fail too.
#  3) The shape gate: with HEMATITA_SMOKE_SHAPE set the window prints the
#     first row's contract once the third revision has landed. An empty or
#     wrong line means the nested lists did not cross as the page expects,
#     which no error message would have said.
#
# This catches *startup* errors only. Keyboard, focus and accessibility need a
# real Wayland session.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=$root/target/release/hematita
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

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/config" "$scratch/data" "$scratch/cache" \
    "$scratch/state" "$scratch/run"
chmod 0700 "$scratch/run"
log=$scratch/output.log

XDG_CONFIG_HOME=$scratch/config \
XDG_DATA_HOME=$scratch/data \
XDG_CACHE_HOME=$scratch/cache \
XDG_STATE_HOME=$scratch/state \
XDG_RUNTIME_DIR=$scratch/run \
DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
QT_QPA_PLATFORM=offscreen \
QT_ASSUME_STDERR_HAS_CONSOLE=1 \
HEMATITA_SMOKE_SHAPE=1 \
    timeout 10 "$bin" >"$log" 2>&1
rc=$?
if [ "$rc" -ne 124 ]; then
    echo "smoke: the binary exited on its own (rc=$rc); last lines:" >&2
    tail -20 "$log" >&2
    exit 1
fi

errors=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Binding loop detected' "$log" || true)
if [ -n "$errors" ]; then
    echo "smoke: QML errors at startup:" >&2
    echo "$errors" | sort | uniq -c | sort -rn >&2
    exit 1
fi

shape=$(grep -E 'hematita-shape cpu 3 60$' "$log" | head -1 || true)
if [ -z "$shape" ]; then
    echo "smoke: the first row did not publish the CPU contract (expected 'hematita-shape cpu 3 60'); got: '$(grep -E 'hematita-shape' "$log" | head -1)'" >&2
    exit 1
fi

echo "smoke: OK — binary alive for 10 s, the first row published the CPU contract, no QML errors, no auto-bindings"
