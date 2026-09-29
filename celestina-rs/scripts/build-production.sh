#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-build celestina-rs
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "build" ]; then
    echo "build-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

# The workspace build unifies features across every member, so its binaries
# differ from the ones an app builds with `-p`. It writes to its own target
# directory: sharing `target/release` let each build overwrite the other's
# registered `magnetitad`, and a landing then refused to deploy Magnetita.
(cd "$project_root" && cargo build --workspace --release --locked --target-dir target/workspace)

echo ">> Rust release workspace build steps completed (not installed)"
