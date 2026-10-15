#!/bin/sh
set -eu

# Starts or replaces the live shell, on the author's explicit request only
# (docs/contracts/production-artifacts.md). CEL-1-B replaces this body: until
# the shell is delivered there is nothing to activate.
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"

production_require_verified "$suite_root" celestina
echo "activate-production: the shell is not deliverable before CEL-1-B" >&2
exit 1
