#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py
binary=$project_root/target/release/hematita

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-verification hematita
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
# built above; CXX-Qt is expensive enough to compile once, let alone twice.
(cd "$project_root" && cargo clippy --release --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --release --all-targets --locked)
(cd "$suite_root/celestina-rs" && cargo fmt --all --check)
(cd "$suite_root/celestina-rs" && \
    cargo clippy --locked -p hematita-core --all-targets -- -D warnings)
(cd "$suite_root/celestina-rs" && cargo test --locked -p hematita-core)
"$suite_root/scripts/qmllint-cxxqt.sh" "$project_root"
"$project_root/scripts/smoke.sh" --binary "$binary"

echo ">> Hematita verification steps completed; awaiting the runner seal"
