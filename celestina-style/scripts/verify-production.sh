#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
build_dir=$project_root/build
artifact_tool=$suite_root/scripts/production_artifact.py

python3 "$artifact_tool" check celestina-style
"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
bash "$project_root/scripts/check-style-contract.sh"
cmake --build "$build_dir" --target all_qmllint
ctest --test-dir "$build_dir" --output-on-failure
"$project_root/scripts/smoke-production.sh"

python3 "$artifact_tool" record-verification celestina-style \
    --verify-command 'scripts/test-production-artifacts.sh' \
    --verify-command 'bash scripts/check-architecture-contract.sh' \
    --verify-command 'bash celestina-style/scripts/check-style-contract.sh' \
    --verify-command 'cmake --build celestina-style/build --target all_qmllint' \
    --verify-command 'ctest --test-dir celestina-style/build --output-on-failure' \
    --verify-command 'celestina-style/scripts/smoke-production.sh'

echo ">> módulo CelestinaStyle vigente y verificado"
