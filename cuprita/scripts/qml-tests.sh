#!/bin/sh
set -eu

# Cuprita's QML interaction tests (`tests/qml`): they click and walk the real
# components with `qmltestrunner`.
#
# The `org.celestina.cuprita` module is normally published by the binary, so
# its types do not exist outside the app. A plugin-less equivalent is built
# here: a `qmldir` generated from the tree's own .qml files (so what is tested
# is the source, not a copy). Types registered from Rust are not part of it;
# the pages take their controllers and models as properties, so the page tests
# hand them QML stand-ins scripted like cuprita-core's fakes
# (tests/qml/fakes). CUPRITA_FAKE=1 is exported all the same, so nothing
# started from here can reach a real backend.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
src=$root/qml

runner=${QMLTESTRUNNER:-}
if [ -z "$runner" ]; then
    # A bare `qmltestrunner` may be Qt 5's, which cannot read this tree, so
    # the Qt 6 one is asked for first.
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
module=$scratch/imports/org/celestina/cuprita
mkdir -p "$module"
ln -s "$src" "$module/qml"

{
    echo "module org.celestina.cuprita"
    cd "$src"
    # The shared .qml files arrive as symlinks from celestina-style, so links
    # are accepted as well as files.
    find . \( -type f -o -type l \) -name '*.qml' | sed 's|^\./||' | sort \
    | while read -r rel; do
        name=$(basename "$rel" .qml)
        if head -5 "$rel" | grep -q '^pragma Singleton'; then
            echo "singleton $name 1.0 qml/$rel"
        else
            echo "$name 1.0 qml/$rel"
        fi
    done
} > "$module/qmldir"

CUPRITA_FAKE=1 QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    "$runner" -input "$root/tests/qml" -import "$scratch/imports" "$@"
