#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
prefix=${HOME}/.local
if [ "${1:-}" = "--prefix" ]; then shift; prefix=${1:?--prefix necesita un directorio}; shift; fi
[ "$#" -eq 0 ] || { echo "uso: scripts/deploy-production.sh [--prefix DIR]" >&2; exit 2; }

app_id=org.celestina.Grafita
production_require_verified "$suite_root" grafita
production_install_xdg_application \
    "$project_root/target/release/grafita" grafita "$app_id" \
    "$project_root/$app_id.desktop" \
    "$suite_root/celestina-style/icons/apps/$app_id.svg" "$prefix"
echo ">> Grafita verificada desplegada en $prefix sin recompilar"

