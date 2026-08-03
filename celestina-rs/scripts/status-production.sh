#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
[ "$#" -eq 0 ] || { echo "uso: scripts/status-production.sh" >&2; exit 2; }
production_status "$suite_root" celestina-rs
