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
import_root=${XDG_RUNTIME_DIR:-/tmp}/celestina-style-gallery
mkdir -p "$import_root"
ln -sfn "$style_root" "$import_root/CelestinaStyle"

qmlbin=$(command -v qml6 || command -v qml || echo /usr/lib/qt6/bin/qml)

platform=${QT_QPA_PLATFORM:-}
[ "${1:-}" = "--offscreen" ] && platform=offscreen

QML_IMPORT_PATH="$import_root" QT_QPA_PLATFORM="$platform" QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    exec "$qmlbin" "$here/Gallery.qml"
