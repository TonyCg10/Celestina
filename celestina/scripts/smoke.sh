#!/bin/sh
set -eu

# Celestina smoke: the stub binary of CEL-1-A answers `--check` with exit 0.
# CEL-1-B replaces it with the nested niri session of the design (§8).
usage() {
    echo "usage: scripts/smoke.sh --binary PATH" >&2
    exit 2
}
[ "$#" -eq 2 ] && [ "$1" = "--binary" ] || usage
bin=$2
[ -x "$bin" ] || { echo "smoke: $bin is not an executable" >&2; exit 1; }
if ! "$bin" --check; then
    echo "smoke: $bin --check failed" >&2
    exit 1
fi
echo "smoke: OK"
