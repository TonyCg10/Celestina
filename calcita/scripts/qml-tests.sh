#!/bin/sh
set -eu

# Calcita's QML tests (`tests/qml`), run with the Qt 6 `qmltestrunner`.
#
# The `org.celestina.calcita` module is published by the binary, so outside
# it the module is rebuilt here without a plugin: a `qmldir` generated from the
# tree's own .qml files, so the source is what gets tested. The types the
# binary registers from Rust are replaced by the QML stand-ins in
# `tests/qml/stubs`, appended to the same `qmldir`, so the whole `Main.qml`
# can be built offscreen. CALCITA_FAKE=1 is exported all the same.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
src=$root/qml
stubs=$root/tests/qml/stubs

runner=${QMLTESTRUNNER:-}
if [ -z "$runner" ]; then
    # Qt 5's runner may come first on PATH and cannot read this tree, so the
    # Qt 6 installation is asked first.
    for candidate in \
        "$(qtpaths6 --query QT_INSTALL_BINS 2>/dev/null || true)/qmltestrunner" \
        "$(qmake6 -query QT_INSTALL_BINS 2>/dev/null || true)/qmltestrunner" \
        /usr/lib/qt6/bin/qmltestrunner
    do
        if [ -x "$candidate" ]; then
            runner=$candidate
            break
        fi
    done
fi
if [ -z "$runner" ]; then
    echo "qml-tests: the Qt 6 qmltestrunner is missing (set QMLTESTRUNNER)" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
module=$scratch/imports/org/celestina/calcita
mkdir -p "$module"
ln -s "$src" "$module/qml"
ln -s "$stubs" "$module/stubs"

# One qmldir line per file under `dir`, as `prefix/relative path`; a file
# whose head says `pragma Singleton` is declared a singleton.
list_types() {
    dir=$1
    prefix=$2
    (
        cd "$dir"
        # The shared files are symlinks into celestina-style.
        find . \( -type f -o -type l \) -name '*.qml' | sed 's|^\./||' | sort \
        | while read -r rel; do
            name=$(basename "$rel" .qml)
            if head -5 "$rel" | grep -q '^pragma Singleton'; then
                echo "singleton $name 1.0 $prefix/$rel"
            else
                echo "$name 1.0 $prefix/$rel"
            fi
        done
    )
}

{
    echo "module org.celestina.calcita"
    list_types "$src" qml
    list_types "$stubs" stubs
} > "$module/qmldir"

CALCITA_FAKE=1 QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    "$runner" -input "$root/tests/qml" -import "$scratch/imports" "$@"
