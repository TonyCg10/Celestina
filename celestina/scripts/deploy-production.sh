#!/bin/sh
set -eu

# Installs the verified shell (docs/contracts/production-artifacts.md): the
# binary at $prefix/bin/celestina, its desktop entry, and the style's font and
# icons under $prefix/share/celestina/, where the painter looks for them
# beside an installed binary. The argument handling and the verified-artifact
# gate follow hematita/scripts/deploy-production.sh at
# 9ecc72eeef9cb5a5418a8213c3fb8cdaa5edff72. It never starts or replaces the
# live shell.
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
prefix=${HOME}/.local
if [ "${1:-}" = "--prefix" ]; then shift; prefix=${1:?--prefix requires a directory}; shift; fi
[ "$#" -eq 0 ] || { echo "usage: scripts/deploy-production.sh [--prefix DIR]" >&2; exit 2; }

production_require_verified "$suite_root" celestina
# Not production_install_xdg_application: it installs an application icon,
# and the shell has none.
production_install_file "$project_root/target/release/celestina" "$prefix/bin/celestina" 0755
applications=$prefix/share/applications
production_install_file "$project_root/celestina.desktop" "$applications/celestina.desktop" 0644
style=$suite_root/celestina-style
share=$prefix/share/celestina
production_install_file "$style/fonts/InterVariable.ttf" "$share/fonts/InterVariable.ttf" 0644
for icon in "$style"/icons/*.svg; do
    production_install_file "$icon" "$share/icons/$(basename -- "$icon")" 0644
done
echo ">> verified Celestina deployed to $prefix; the shell was not started"
