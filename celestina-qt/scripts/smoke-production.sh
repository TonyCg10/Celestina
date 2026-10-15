#!/bin/sh
set -u

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
style_build=$suite_root/celestina-style/build
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/config" "$scratch/data" "$scratch/cache" "$scratch/state" "$scratch/run"
chmod 0700 "$scratch/run"
log=$scratch/celestina.log

if [ -n "${LD_LIBRARY_PATH:-}" ]; then
    production_library_path=$style_build:$LD_LIBRARY_PATH
else
    production_library_path=$style_build
fi

# `ddcutil` is the only thing a provider helper does that reaches real
# hardware — the graphics card's own I²C buses, which the desktop this runs
# inside is already using. Two GPU losses have been recorded with concurrent
# `ddcutil` children on one bus, and this smoke only ever existed to prove the
# release host and the compiled module load and stay up. It gets the helper it
# would get in a session, minus the one call that can take the machine down.
XDG_CONFIG_HOME=$scratch/config \
XDG_DATA_HOME=$scratch/data \
XDG_CACHE_HOME=$scratch/cache \
XDG_STATE_HOME=$scratch/state \
XDG_RUNTIME_DIR=$scratch/run \
DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
CELESTINA_DDC=0 \
QT_QPA_PLATFORM=offscreen \
QT_ASSUME_STDERR_HAS_CONSOLE=1 \
CELESTINA_STYLE_PATH=$style_build/CelestinaStyle \
CELESTINA_NIRI_ADAPTER_PATH=$project_root/build/rust-target/release/celestina-niri-adapter \
CELESTINA_PROVIDER_ADAPTER_PATH=$project_root/build/rust-target/release/celestina-provider-adapter \
LD_LIBRARY_PATH=$production_library_path \
    timeout 8 "$project_root/build/celestina" --pick-output >"$log" 2>&1
rc=$?

if [ "$rc" -ne 124 ]; then
    echo "smoke-production: el selector terminó solo (rc=$rc)" >&2
    tail -30 "$log" >&2
    exit 1
fi

style_link=$scratch/run/celestina-shell-import/CelestinaStyle
if [ ! -L "$style_link" ] || \
    [ "$(readlink -f "$style_link")" != "$(readlink -f "$style_build/CelestinaStyle")" ]; then
    echo "smoke-production: el host no enlazó el módulo CelestinaStyle compilado" >&2
    exit 1
fi

errors=$(grep -Ei 'TypeError|ReferenceError|SyntaxError|failed to load component|failed to create component|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|unavailable|is not a type|module .* is not installed|plugin cannot be loaded|Required property .* was not initialized|Binding loop detected' "$log" || true)
if [ -n "$errors" ]; then
    echo "smoke-production: errores QML al cargar host + módulo compilado:" >&2
    echo "$errors" | sort | uniq -c | sort -rn >&2
    exit 1
fi

echo "smoke-production: OK — host release + CelestinaStyle compilado vivos 8 s"
