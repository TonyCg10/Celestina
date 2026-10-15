#!/bin/sh

set -eu

# run.sh — build the Celestina shell (Release) and activate it: map the panel on
# every output of the running session. The one script the shell needs. Unlike
# the apps, the shell is not a launcher entry — running it *is* activating it, so
# this launches in the foreground and stays up (Ctrl-C to stop).
#
# Any arguments pass through to the binary (e.g. `run.sh --pick-output` for the
# screen-share chooser). CelestinaStyle is resolved from source by the binary
# itself (see src/main.cpp), so no import-path wiring is needed here.

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/.." && pwd)
build=$root/build

cmake -S "$root" -B "$build" -DCMAKE_BUILD_TYPE=Release >/dev/null
cmake --build "$build" --parallel

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "aviso: WAYLAND_DISPLAY vacío; el panel necesita una sesión Wayland viva" >&2
fi

echo ">> activando el panel de Celestina" >&2
exec "$build/celestina" "$@"
