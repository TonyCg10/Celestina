#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py
binary=$project_root/target/release/siderita

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-verification siderita
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "verify" ]; then
    echo "verify-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
(cd "$project_root" && cargo fmt --all --check)
(cd "$project_root" && cargo clippy --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --all-targets --locked)
(cd "$suite_root/celestina-rs" && cargo fmt --all --check)
(cd "$suite_root/celestina-rs" && cargo clippy --locked \
    -p celestina-core -p siderita-core -p siderita-ops -p siderita-qt \
    -p grafita-core -p fluorita-core -p fluorita-engine -p fluorita-qt \
    --all-targets -- -D warnings)
(cd "$suite_root/celestina-rs" && cargo test --locked \
    -p celestina-core -p siderita-core -p siderita-ops -p siderita-qt \
    -p grafita-core -p fluorita-core -p fluorita-engine -p fluorita-qt)
"$suite_root/scripts/qmllint-cxxqt.sh" "$project_root"
"$project_root/scripts/qml-tests.sh"
"$project_root/scripts/smoke.sh" --binary "$binary"

echo ">> Siderita verification steps completed; awaiting the runner seal"
