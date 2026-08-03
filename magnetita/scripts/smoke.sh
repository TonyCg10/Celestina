#!/bin/sh
set -u

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binary=$project_root/target/release/magnetita
if [ "${1:-}" = "--binary" ]; then
    shift
    binary=${1:?--binary necesita una ruta}
    shift
fi
if [ "$#" -ne 0 ]; then
    echo "uso: scripts/smoke.sh [--binary RUTA]" >&2
    exit 2
fi
if [ ! -x "$binary" ]; then
    echo "smoke: falta el binario indicado: $binary" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/config" "$scratch/data" "$scratch/cache" \
    "$scratch/state" "$scratch/run"
chmod 0700 "$scratch/run"
log=$scratch/salida.log

XDG_CONFIG_HOME=$scratch/config \
XDG_DATA_HOME=$scratch/data \
XDG_CACHE_HOME=$scratch/cache \
XDG_STATE_HOME=$scratch/state \
XDG_RUNTIME_DIR=$scratch/run \
DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
QT_QPA_PLATFORM=offscreen \
QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    timeout 8 "$binary" >"$log" 2>&1
rc=$?
if [ "$rc" -ne 124 ]; then
    echo "smoke: el binario terminó solo (rc=$rc); últimas líneas:" >&2
    tail -20 "$log" >&2
    exit 1
fi

errors=$(grep -E 'TypeError|ReferenceError|SyntaxError|failed to load component|failed to create component|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Required property [A-Za-z_][A-Za-z0-9_]* was not initialized|Binding loop detected' "$log" || true)
if [ -n "$errors" ]; then
    echo "smoke: errores QML en el arranque:" >&2
    echo "$errors" | sort | uniq -c | sort -rn >&2
    exit 1
fi

echo "smoke: OK — cliente vivo 8 s, XDG/D-Bus aislados y sin errores QML"
