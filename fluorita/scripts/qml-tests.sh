#!/bin/sh
set -eu

# Fluorita's QML tests (`tests/qml`), run with the Qt 6 `qmltestrunner`.
#
# The `org.celestina.fluorita` module is published by the binary, so outside
# it the module is rebuilt here without a plugin: a `qmldir` generated from the
# tree's own .qml files, so the source is what gets tested. The types the
# binary registers from Rust are replaced by the QML stand-ins in
# `tests/qml/stubs`, appended to the same `qmldir` (and the C++ video
# surface by `tests/qml/render`); a stub stands in for what
# the Rust type publishes and records what it is asked, and what the Rust
# side then does on disk is the business of its own crate tests. Selenita's
# harness is the model; nothing here reaches a decoder or a bus.

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
module=$scratch/imports/org/celestina/fluorita
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
    echo "module org.celestina.fluorita"
    list_types "$src" qml
    list_types "$stubs" stubs
} > "$module/qmldir"

# The video surface is hand-written C++ in a module of its own,
# `org.celestina.fluorita.render`; its stand-in lives in `tests/qml/render`.
render=$module/render
mkdir -p "$render"
ln -s "$root/tests/qml/render" "$render/stubs"
{
    echo "module org.celestina.fluorita.render"
    list_types "$root/tests/qml/render" stubs
} > "$render/qmldir"

QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    "$runner" -input "$root/tests/qml" -import "$scratch/imports" "$@"
