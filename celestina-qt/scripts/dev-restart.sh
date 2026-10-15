#!/bin/sh

set -eu

# Fast, explicitly non-canonical visual iteration loop. It incrementally builds
# the style module and shell host, gracefully retires only the Celestina process
# that owns the session bus name, and runs the build-tree bytes in the
# foreground. Ctrl-C stops the shell and its owned helpers.

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
build=$project_root/build
style_build=$suite_root/celestina-style/build

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "dev-restart: WAYLAND_DISPLAY is empty; Celestina needs Wayland" >&2
    exit 1
fi

cmake -S "$suite_root/celestina-style" -B "$style_build" \
    -DCMAKE_BUILD_TYPE=Release >/dev/null
cmake --build "$style_build" --target celestina-style-plugin --parallel

cmake -S "$project_root" -B "$build" -DCMAKE_BUILD_TYPE=Release >/dev/null
cmake --build "$build" --target celestina --parallel

owner_pid=$(
    busctl --user --no-pager status org.celestina.Shell 2>/dev/null \
        | sed -n 's/^PID=//p'
)

if [ -n "$owner_pid" ]; then
    case "$owner_pid" in
        *[!0-9]*|'')
            echo "dev-restart: refused an unreadable shell-owner PID: $owner_pid" >&2
            exit 1
            ;;
    esac

    owner_exe=$(readlink "/proc/$owner_pid/exe" 2>/dev/null || true)
    case "$owner_exe" in
        "$build/celestina"|"$build/celestina (deleted)"|\
        "$HOME/.local/libexec/celestina/celestina"|\
        "$HOME/.local/libexec/celestina/celestina (deleted)")
            ;;
        *)
            echo "dev-restart: org.celestina.Shell belongs to an unexpected executable:" >&2
            echo "  $owner_exe" >&2
            exit 1
            ;;
    esac

    echo ">> stopping the current Celestina host ($owner_pid)" >&2
    kill -TERM "$owner_pid"
    attempts=0
    while kill -0 "$owner_pid" 2>/dev/null; do
        if [ "$attempts" -ge 100 ]; then
            echo "dev-restart: the current host did not stop after SIGTERM" >&2
            exit 1
        fi
        attempts=$((attempts + 1))
        sleep 0.1
    done
fi

export CELESTINA_NIRI_ADAPTER_PATH=$build/rust-target/release/celestina-niri-adapter
export CELESTINA_PROVIDER_ADAPTER_PATH=$build/rust-target/release/celestina-provider-adapter
export CELESTINA_STYLE_PATH=$style_build/CelestinaStyle
if [ -n "${LD_LIBRARY_PATH:-}" ]; then
    export LD_LIBRARY_PATH=$style_build:$LD_LIBRARY_PATH
else
    export LD_LIBRARY_PATH=$style_build
fi

echo ">> running incremental Celestina build; Ctrl-C stops it" >&2
exec "$build/celestina" "$@"
