#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
prefix=${HOME}/.local
if [ "${1:-}" = "--prefix" ]; then
    shift
    prefix=${1:?--prefix necesita un directorio}
    shift
fi
[ "$#" -eq 0 ] || { echo "uso: scripts/status-production.sh [--prefix DIR]" >&2; exit 2; }

production_status "$suite_root" celestina \
    --installed "celestina/build/celestina=$prefix/libexec/celestina/celestina" \
    --installed "celestina/build/rust-target/release/celestina-niri-adapter=$prefix/libexec/celestina/celestina-niri-adapter" \
    --installed "celestina/build/rust-target/release/celestina-provider-adapter=$prefix/libexec/celestina/celestina-provider-adapter" \
    --installed "celestina-style/build/libcelestina-style.so=$prefix/libexec/celestina/libcelestina-style.so" \
    --installed "celestina-style/build/CelestinaStyle=$prefix/libexec/celestina/CelestinaStyle" \
    --installed "celestina/scripts/celestina-launcher.sh=$prefix/bin/celestina" \
    --installed "celestina/celestina.desktop=$prefix/share/applications/celestina.desktop"
