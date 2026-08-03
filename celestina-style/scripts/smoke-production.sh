#!/bin/sh
set -u

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
build_dir=$project_root/build
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/config" "$scratch/data" "$scratch/cache" "$scratch/state" "$scratch/run"
chmod 0700 "$scratch/run"
log=$scratch/gallery.log

if [ ! -f "$build_dir/CelestinaStyle/qmldir" ] || \
    [ ! -f "$build_dir/CelestinaStyle/libcelestina-style-plugin.so" ] || \
    [ ! -f "$build_dir/libcelestina-style.so" ]; then
    echo "smoke-production: falta el módulo CelestinaStyle compilado" >&2
    exit 1
fi

qmlbin=$(command -v qml6 || command -v qml || echo /usr/lib/qt6/bin/qml)
if [ -n "${LD_LIBRARY_PATH:-}" ]; then
    production_library_path=$build_dir:$LD_LIBRARY_PATH
else
    production_library_path=$build_dir
fi

XDG_CONFIG_HOME=$scratch/config \
XDG_DATA_HOME=$scratch/data \
XDG_CACHE_HOME=$scratch/cache \
XDG_STATE_HOME=$scratch/state \
XDG_RUNTIME_DIR=$scratch/run \
QT_QPA_PLATFORM=offscreen \
QT_ASSUME_STDERR_HAS_CONSOLE=1 \
QML_IMPORT_PATH=$build_dir \
QML2_IMPORT_PATH=$build_dir \
LD_LIBRARY_PATH=$production_library_path \
    timeout 8 "$qmlbin" "$project_root/gallery/Gallery.qml" >"$log" 2>&1
rc=$?

if [ "$rc" -ne 124 ]; then
    echo "smoke-production: la galería terminó sola (rc=$rc)" >&2
    tail -20 "$log" >&2
    exit 1
fi

errors=$(grep -Ei 'TypeError|ReferenceError|SyntaxError|failed to load component|failed to create component|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|unavailable|is not a type|module .* is not installed|plugin cannot be loaded|Required property .* was not initialized|Binding loop detected' "$log" || true)
if [ -n "$errors" ]; then
    echo "smoke-production: errores QML en la galería:" >&2
    echo "$errors" | sort | uniq -c | sort -rn >&2
    exit 1
fi

echo "smoke-production: OK — galería viva 8 s sobre el módulo compilado"
