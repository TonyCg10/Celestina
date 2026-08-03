#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "uso: scripts/qmllint-cxxqt.sh RUTA_DEL_PROYECTO" >&2
    exit 2
fi

app_root=$(CDPATH= cd -- "$1" && pwd)
qml_root=$app_root/qml
module_root=$(
    find "$app_root/target/release/build" \
        -path '*/out/qt-build-utils/qml_modules' -type d \
        -printf '%T@ %p\n' 2>/dev/null \
    | sort -nr \
    | sed -n '1s/^[^ ]* //p'
)

if [ -z "$module_root" ]; then
    echo "qmllint-production: falta el módulo QML release generado; ejecuta build-production.sh" >&2
    exit 1
fi

uri=$(sed -n 's/^module //p' "$module_root"/*/qmldir "$module_root"/*/*/qmldir "$module_root"/*/*/*/qmldir 2>/dev/null | head -1)
if [ -z "$uri" ]; then
    echo "qmllint-production: el output release no declara el URI del módulo" >&2
    exit 1
fi
module_relative=$(printf '%s' "$uri" | tr . /)
generated_module=$module_root/$module_relative
if [ ! -f "$generated_module/qmldir" ] || [ ! -f "$generated_module/plugin.qmltypes" ]; then
    echo "qmllint-production: faltan qmldir/plugin.qmltypes para $uri" >&2
    exit 1
fi

linter=${QMLLINT:-}
if [ -z "$linter" ]; then
    for candidate in \
        "$(qtpaths6 --query QT_INSTALL_BINS 2>/dev/null || true)/qmllint" \
        "$(qmake6 -query QT_INSTALL_BINS 2>/dev/null || true)/qmllint" \
        /usr/lib/qt6/bin/qmllint \
        "$(command -v qmllint 2>/dev/null || true)"
    do
        if [ -x "$candidate" ]; then
            linter=$candidate
            break
        fi
    done
fi
if [ -z "$linter" ]; then
    echo "qmllint-production: falta qmllint de Qt 6 (fija QMLLINT)" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
scratch_module=$scratch/imports/$module_relative
mkdir -p "$scratch_module"
cp "$generated_module/qmldir" "$generated_module/plugin.qmltypes" "$scratch_module/"
ln -s "$qml_root" "$scratch_module/qml"
log=$scratch/qmllint.log

set +e
find "$qml_root" \( -type f -o -type l \) -name '*.qml' -print0 \
    | xargs -0 "$linter" -I "$scratch/imports" >"$log" 2>&1
rc=$?
set -e
if [ "$rc" -ne 0 ]; then
    echo "qmllint-production: falló para $uri (rc=$rc)" >&2
    cat "$log" >&2
    exit "$rc"
fi

warnings=$(grep -c '^Warning:' "$log" || true)
echo "qmllint-production: OK — $uri ($warnings avisos no fatales del baseline)"
