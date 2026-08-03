#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
response=$project_root/build/.rcc/qmllint/celestina.rsp

if [ ! -f "$response" ]; then
    echo "qmllint-production: falta la respuesta release; ejecuta build-production.sh" >&2
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

"$linter" "@$response"
echo "qmllint-production: OK — se reutilizó el módulo generado, sin invocar CMake/Cargo"
