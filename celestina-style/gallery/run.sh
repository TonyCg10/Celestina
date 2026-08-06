#!/bin/sh
# Run the CelestinaStyle gallery with the plain QML runtime (no build step).
#
#   run.sh              open a real window (glass + shadow render here)
#   run.sh --offscreen  run under the offscreen QPA (colours/type/controls only;
#                        glass and shadow need a real GPU session)
#
# The style module is resolved by pointing QML_IMPORT_PATH at a directory that
# contains a `CelestinaStyle` entry (the source tree) — the same trick the
# shell's output chooser uses.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
style_root=$(CDPATH= cd -- "$here/.." && pwd)

# A fresh private directory, never a fixed name under a shared one. The import
# root is where the QML engine looks for the module it is about to execute, so
# whoever controls that directory controls the code this runs. `XDG_RUNTIME_DIR`
# is unset over ssh, under cron and in containers, and the old fallback then
# named a predictable path in world-writable /tmp: another local user can
# pre-create it and point the `CelestinaStyle` entry at a module of their own,
# which the runtime would then load with the author's privileges.
import_root=$(mktemp -d "${TMPDIR:-/tmp}/celestina-style-gallery.XXXXXX")
cleanup() { rm -rf -- "$import_root"; }
trap cleanup EXIT
trap 'cleanup; exit 130' INT
trap 'cleanup; exit 143' TERM HUP
ln -s "$style_root" "$import_root/CelestinaStyle"

qmlbin=$(command -v qml6 || command -v qml || echo /usr/lib/qt6/bin/qml)

platform=${QT_QPA_PLATFORM:-}
[ "${1:-}" = "--offscreen" ] && platform=offscreen

# Deliberately not `exec`: the shell has to outlive the runtime so the trap can
# remove the import root it created.
QML_IMPORT_PATH="$import_root" QT_QPA_PLATFORM="$platform" QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    "$qmlbin" "$here/Gallery.qml"
