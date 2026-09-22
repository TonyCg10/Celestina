#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
prefix=${HOME}/.local
if [ "${1:-}" = "--prefix" ]; then shift; prefix=${1:?--prefix requires a directory}; shift; fi
[ "$#" -eq 0 ] || { echo "usage: scripts/status-production.sh [--prefix DIR]" >&2; exit 2; }
production_status "$suite_root" hematita \
    --installed "hematita/target/release/hematita=$prefix/bin/hematita"

