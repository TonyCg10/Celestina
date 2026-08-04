#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
build_dir=$project_root/build
artifact_tool=$suite_root/scripts/production_artifact.py

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-verification celestina-style
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "verify" ]; then
    echo "verify-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
bash "$project_root/scripts/check-style-contract.sh"
cmake --build "$build_dir" --target all_qmllint
ctest --test-dir "$build_dir" --output-on-failure
"$project_root/scripts/smoke-production.sh"

echo ">> CelestinaStyle verification steps completed"
