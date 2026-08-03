#!/bin/sh
set -eu
suite_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
exec python3 "$suite_root/scripts/complete-production.py" fluorita
