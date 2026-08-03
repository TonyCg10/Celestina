#!/bin/sh
set -eu

suite_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
python3 "$suite_root/scripts/test-production-artifacts.py"
exec "$suite_root/scripts/test-production-common.sh"
