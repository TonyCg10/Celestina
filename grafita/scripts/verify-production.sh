#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py
binary=$project_root/target/release/grafita

python3 "$artifact_tool" check grafita
"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
(cd "$project_root" && cargo fmt --all --check)
(cd "$project_root" && cargo clippy --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --all-targets --locked)
(cd "$suite_root/celestina-rs" && cargo fmt --all --check)
(cd "$suite_root/celestina-rs" && \
    cargo clippy --locked -p grafita-core --all-targets -- -D warnings)
(cd "$suite_root/celestina-rs" && cargo test --locked -p grafita-core)
"$suite_root/scripts/qmllint-cxxqt.sh" "$project_root"
"$project_root/scripts/smoke.sh" --binary "$binary"

python3 "$artifact_tool" record-verification grafita \
    --verify-command 'scripts/test-production-artifacts.sh' \
    --verify-command 'bash scripts/check-architecture-contract.sh' \
    --verify-command 'cargo fmt/clippy/test --manifest-path grafita/Cargo.toml' \
    --verify-command 'cargo fmt --manifest-path celestina-rs/Cargo.toml --all --check' \
    --verify-command 'cargo clippy --manifest-path celestina-rs/Cargo.toml -p grafita-core --all-targets --locked -- -D warnings' \
    --verify-command 'cargo test --manifest-path celestina-rs/Cargo.toml -p grafita-core' \
    --verify-command 'scripts/qmllint-cxxqt.sh grafita' \
    --verify-command 'grafita/scripts/smoke.sh --binary grafita/target/release/grafita'

echo ">> Grafita vigente y verificada; lista para deploy sin recompilar"
