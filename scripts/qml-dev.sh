#!/bin/sh
set -eu

# Run an application with its QML read from the repository instead of from the
# module compiled into its binary, so a QML change, such as a colour in the
# shared CelestinaTheme, needs a restart instead of a build.
#
#   scripts/qml-dev.sh APP [--binary PATH] [-- ARGUMENTS...]
#
# APP is the application directory (siderita, grafita, hematita, fluorita,
# magnetita). The script takes the qmldir and plugin.qmltypes of the release
# module the last build generated, which scripts/qmllint-cxxqt.sh locates, and
# lays them out under $XDG_RUNTIME_DIR/celestina-qml-dev/APP/ with `qml`
# linked to APP/qml, as the lint does. The `prefer` line is dropped, since it
# sends the engine back to the module in the binary. The binary, by default the
# installed ~/.local/bin/APP, then loads that tree: CELESTINA_QML_DEV_IMPORT is
# put first among its import paths and CELESTINA_QML_DEV_MAIN is the file URL
# of the Main.qml it loads.
#
# This is a development aid, not a delivery path. The binary must carry the
# same Rust and C++ types the QML uses, and a new QML file, image or font is
# registered by the build; those still need one. A production build and the
# landing never read this tree.
#
# Exit 2 on usage errors and 1 when the tree cannot be laid out.

usage() {
    echo "usage: scripts/qml-dev.sh APP [--binary PATH] [-- ARGUMENTS...]" >&2
    exit 2
}

refuse() {
    printf 'qml-dev: %s\n' "$*" >&2
    exit 1
}

[ "$#" -ge 1 ] || usage
app=$1
shift
case $app in
    '' | */* | .*) usage ;;
esac
binary=
while [ "$#" -gt 0 ]; do
    case $1 in
        --binary)
            [ "$#" -ge 2 ] || usage
            binary=$2
            shift 2
            ;;
        --)
            shift
            break
            ;;
        *) usage ;;
    esac
done

suite_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
app_root=$suite_root/$app
[ -f "$app_root/build.rs" ] && [ -d "$app_root/qml" ] \
    || refuse "$app is not an application with a QML module"
[ -n "$binary" ] || binary=$HOME/.local/bin/$app
[ -x "$binary" ] || refuse "no executable $binary; install $app or pass --binary"

located=$(sh "$suite_root/scripts/qmllint-cxxqt.sh" --print-module "$app_root") \
    || refuse "build $app once (build-production.sh) so its module exists"
uri=${located%%"	"*}
generated=${located#*"	"}
module_relative=$(printf '%s' "$uri" | tr . /)

runtime=${XDG_RUNTIME_DIR:-}
[ -n "$runtime" ] && [ -d "$runtime" ] || refuse "XDG_RUNTIME_DIR is not set"
tree=$runtime/celestina-qml-dev/$app
rm -rf -- "$tree"
module=$tree/$module_relative
mkdir -p -- "$module"
grep -v '^prefer ' "$generated/qmldir" > "$module/qmldir"
cp -- "$generated/plugin.qmltypes" "$module/plugin.qmltypes"
ln -s -- "$app_root/qml" "$module/qml"
[ -f "$module/qml/Main.qml" ] || refuse "$app_root/qml has no Main.qml"
main_url=$(python3 -c 'import pathlib, sys; print(pathlib.Path(sys.argv[1]).as_uri())' \
    "$module/qml/Main.qml") || refuse "cannot spell the URL of $module/qml/Main.qml"

echo "qml-dev: $app reads its QML from $app_root/qml" >&2
CELESTINA_QML_DEV_IMPORT=$tree \
CELESTINA_QML_DEV_MAIN=$main_url \
    exec "$binary" "$@"
