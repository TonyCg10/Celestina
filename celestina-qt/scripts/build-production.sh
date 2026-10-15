#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
build_dir=$project_root/build
style_root=$suite_root/celestina-style
artifact_tool=$suite_root/scripts/production_artifact.py

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-build celestina
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "build" ]; then
    echo "build-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

. "$project_root/scripts/session-interlock.sh"
celestina_refuse_if_running "$project_root" "${HOME}/.local"

"$style_root/scripts/build-production.sh" --production-runner-internal
cmake -S "$project_root" -B "$build_dir" \
    -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build "$build_dir" --parallel

echo ">> release bundle build steps completed (not activated or installed)"
