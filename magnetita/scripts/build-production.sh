#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
rust_workspace=$suite_root/celestina-rs
artifact_tool=$suite_root/scripts/production_artifact.py

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-build magnetita
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "build" ]; then
    echo "build-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

(cd "$project_root" && cargo build --release --locked --bin magnetita)
(cd "$rust_workspace" && cargo build --release --locked -p magnetitad)

echo ">> Magnetita + magnetitad build steps completed (not installed; service untouched)"
