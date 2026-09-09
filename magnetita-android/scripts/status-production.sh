#!/bin/sh
# Reports the sealed debug artifact against the tree, through the suite's
# runner. Nothing is installed on the host: the APK goes to the phone.
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"
[ "$#" -eq 0 ] || { echo "usage: scripts/status-production.sh" >&2; exit 2; }
production_status "$suite_root" magnetita-android
