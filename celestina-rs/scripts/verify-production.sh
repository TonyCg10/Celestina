#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py

python3 "$artifact_tool" check celestina-rs
"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
(cd "$project_root" && cargo fmt --all --check)
(cd "$project_root" && cargo clippy --workspace --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --workspace --locked)

python3 "$artifact_tool" record-verification celestina-rs \
    --verify-command 'scripts/test-production-artifacts.sh' \
    --verify-command 'bash scripts/check-architecture-contract.sh' \
    --verify-command 'cargo fmt --manifest-path celestina-rs/Cargo.toml --all --check' \
    --verify-command 'cargo clippy --manifest-path celestina-rs/Cargo.toml --workspace --all-targets --locked -- -D warnings' \
    --verify-command 'cargo test --manifest-path celestina-rs/Cargo.toml --workspace --locked'

echo ">> workspace Rust vigente y verificado"
