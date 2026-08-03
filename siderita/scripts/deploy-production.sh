#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
default_prefix=${HOME}/.local
prefix=$default_prefix
if [ "${1:-}" = "--prefix" ]; then
    shift
    prefix=${1:?--prefix necesita un directorio}
    shift
fi
[ "$#" -eq 0 ] || { echo "uso: scripts/deploy-production.sh [--prefix DIR]" >&2; exit 2; }

app_id=org.celestina.Siderita
production_require_verified "$suite_root" siderita
production_install_xdg_application \
    "$project_root/target/release/siderita" siderita "$app_id" \
    "$project_root/$app_id.desktop" \
    "$suite_root/celestina-style/icons/apps/$app_id.svg" "$prefix"

production_install_file "$project_root/portal/celestina.portal" \
    "$prefix/share/xdg-desktop-portal/portals/celestina.portal" 0644
production_install_template \
    "$project_root/portal/org.freedesktop.impl.portal.desktop.celestina.service" \
    "$prefix/share/dbus-1/services/org.freedesktop.impl.portal.desktop.celestina.service" \
    '@BIN@' "$prefix/bin/siderita"
if [ "$prefix" = "$default_prefix" ] && command -v busctl >/dev/null 2>&1; then
    busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
        org.freedesktop.DBus ReloadConfig >/dev/null 2>&1 || true
fi

echo ">> Siderita verificada desplegada en $prefix sin recompilar"
