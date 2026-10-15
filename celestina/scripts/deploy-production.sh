#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
prefix=${HOME}/.local
if [ "${1:-}" = "--prefix" ]; then shift; prefix=${1:?--prefix requires a directory}; shift; fi
[ "$#" -eq 0 ] || { echo "usage: scripts/deploy-production.sh [--prefix DIR]" >&2; exit 2; }

production_require_verified "$suite_root" celestina
# Not production_install_xdg_application: it installs an application icon,
# and the shell has none. The binary and its desktop entry are all it needs.
production_install_file "$project_root/target/release/celestina" "$prefix/bin/celestina" 0755
production_install_file "$project_root/celestina.desktop" \
    "$prefix/share/applications/celestina.desktop" 0644
echo ">> verified Celestina deployed to $prefix without rebuilding"
