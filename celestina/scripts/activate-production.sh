#!/bin/sh
set -eu

# Starts the deployed shell, on the author's explicit request only
# (docs/contracts/production-artifacts.md). No other project of the suite
# has an activation entry; the argument handling and the verified-artifact
# gate follow hematita/scripts/deploy-production.sh at
# 9ecc72eeef9cb5a5418a8213c3fb8cdaa5edff72. It refuses while a shell owns
# `org.celestina.Shell`, and when it cannot tell: it never replaces a
# running shell (`celestina quit` stops one first).
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
prefix=${HOME}/.local
if [ "${1:-}" = "--prefix" ]; then shift; prefix=${1:?--prefix requires a directory}; shift; fi
[ "$#" -eq 0 ] || { echo "usage: scripts/activate-production.sh [--prefix DIR]" >&2; exit 2; }

production_require_verified "$suite_root" celestina
[ -n "${WAYLAND_DISPLAY:-}" ] || {
    echo "activate-production: no WAYLAND_DISPLAY; start the shell from the niri session" >&2
    exit 1
}
command -v busctl >/dev/null 2>&1 || {
    echo "activate-production: busctl is not installed, so a running shell cannot be ruled out;" >&2
    echo "  nothing was started" >&2
    exit 1
}
if busctl --user --no-pager status org.celestina.Shell >/dev/null 2>&1; then
    echo "activate-production: org.celestina.Shell has an owner; the shell is already running" >&2
    echo "  stop it with: celestina quit" >&2
    exit 1
fi
installed=celestina/target/release/celestina=$prefix/bin/celestina
production_status "$suite_root" celestina --installed "$installed"
# Diagnostics go to the journal (stderr through `systemd-cat`) where it
# exists, and nowhere otherwise: the shell outlives this terminal.
if command -v systemd-cat >/dev/null 2>&1; then
    setsid -f systemd-cat -t celestina "$prefix/bin/celestina" </dev/null >/dev/null 2>&1
else
    setsid -f "$prefix/bin/celestina" </dev/null >/dev/null 2>&1
fi
echo ">> Celestina started from $prefix/bin/celestina"
