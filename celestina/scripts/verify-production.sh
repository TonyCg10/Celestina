#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py
binary=$project_root/target/release/celestina

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-verification celestina
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "verify" ]; then
    echo "verify-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
(cd "$project_root" && cargo fmt --all --check)
# Release profile here so clippy and tests share the release artifact cache
# built above.
(cd "$project_root" && cargo clippy --workspace --release --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --workspace --release --locked)
"$project_root/scripts/smoke.sh" --binary "$binary"

echo ">> Celestina verification steps completed; awaiting the runner seal"
