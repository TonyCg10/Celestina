#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"

prefix=${HOME}/.local
from_build=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help)
            echo "uso: scripts/activate-production.sh [--prefix DIR | --from-build] [-- ARGS...]" >&2
            exit 0
            ;;
        --prefix)
            shift
            prefix=${1:?--prefix necesita un directorio}
            ;;
        --from-build)
            from_build=1
            ;;
        --)
            shift
            break
            ;;
        *)
            break
            ;;
    esac
    shift
done

production_require_verified "$suite_root" celestina
if [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "activate-production: WAYLAND_DISPLAY está vacío; Celestina necesita Wayland" >&2
    exit 1
fi

if [ "$from_build" -eq 1 ]; then
    export CELESTINA_NIRI_ADAPTER_PATH=$project_root/build/rust-target/release/celestina-niri-adapter
    export CELESTINA_PROVIDER_ADAPTER_PATH=$project_root/build/rust-target/release/celestina-provider-adapter
    export CELESTINA_STYLE_PATH=$suite_root/celestina-style/build/CelestinaStyle
    if [ -n "${LD_LIBRARY_PATH:-}" ]; then
        export LD_LIBRARY_PATH=$suite_root/celestina-style/build:$LD_LIBRARY_PATH
    else
        export LD_LIBRARY_PATH=$suite_root/celestina-style/build
    fi
    exec "$project_root/build/celestina" "$@"
fi

production_status "$suite_root" celestina \
    --installed "celestina/build/celestina=$prefix/libexec/celestina/celestina" \
    --installed "celestina/build/rust-target/release/celestina-niri-adapter=$prefix/libexec/celestina/celestina-niri-adapter" \
    --installed "celestina/build/rust-target/release/celestina-provider-adapter=$prefix/libexec/celestina/celestina-provider-adapter" \
    --installed "celestina-style/build/libcelestina-style.so=$prefix/libexec/celestina/libcelestina-style.so" \
    --installed "celestina-style/build/CelestinaStyle=$prefix/libexec/celestina/CelestinaStyle" \
    --installed "celestina/scripts/celestina-launcher.sh=$prefix/bin/celestina" \
    --installed "celestina/celestina.desktop=$prefix/share/applications/celestina.desktop"
exec "$prefix/bin/celestina" "$@"
