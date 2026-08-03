#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
prefix=${HOME}/.local
if [ "${1:-}" = "--prefix" ]; then shift; prefix=${1:?--prefix necesita un directorio}; shift; fi
[ "$#" -eq 0 ] || { echo "uso: scripts/deploy-production.sh [--prefix DIR]" >&2; exit 2; }

app_id=org.celestina.Magnetita
daemon_unit=magnetitad.service
default_prefix=${HOME}/.local
service_was_active=0
service_stopped=0

restore_running_service() {
    if [ "$service_was_active" -eq 1 ] && [ "$service_stopped" -eq 1 ]; then
        systemctl --user start "$daemon_unit" >/dev/null 2>&1 || true
    fi
}

production_require_verified "$suite_root" magnetita

if [ "$prefix" = "$default_prefix" ] && command -v systemctl >/dev/null 2>&1; then
    if systemctl --user is-active --quiet "$daemon_unit"; then
        service_was_active=1
        systemctl --user stop "$daemon_unit"
        service_stopped=1
        trap restore_running_service EXIT HUP INT TERM
    fi
fi

production_install_xdg_application \
    "$project_root/target/release/magnetita" magnetita "$app_id" \
    "$project_root/$app_id.desktop" \
    "$suite_root/celestina-style/icons/apps/$app_id.svg" "$prefix"
production_install_file "$suite_root/celestina-rs/target/release/magnetitad" \
    "$prefix/bin/magnetitad" 0755

if [ "$prefix" = "$default_prefix" ]; then
    config_root=${XDG_CONFIG_HOME:-${HOME}/.config}
    service_destination=$config_root/systemd/user/$daemon_unit
else
    service_destination=$prefix/share/systemd/user/$daemon_unit
fi
production_install_template "$project_root/magnetitad.service" \
    "$service_destination" '^ExecStart=.*$' "ExecStart=$prefix/bin/magnetitad"

if [ "$prefix" = "$default_prefix" ] && command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload
    if [ "$service_was_active" -eq 1 ]; then
        systemctl --user start "$daemon_unit"
        service_stopped=0
    fi
fi
trap - EXIT HUP INT TERM

echo ">> Magnetita + magnetitad verificados desplegados en $prefix sin recompilar"
if [ "$service_was_active" -eq 0 ]; then
    echo "   magnetitad permaneció inactivo; deploy no habilita servicios nuevos"
fi

