#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py
binary=$project_root/target/release/magnetita

python3 "$artifact_tool" check magnetita
"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
(cd "$project_root" && cargo fmt --all --check)
(cd "$project_root" && cargo clippy --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --all-targets --locked)
(cd "$suite_root/celestina-rs" && cargo fmt --all --check)
(cd "$suite_root/celestina-rs" && cargo clippy --locked \
    -p celestina-core -p magnetita-core -p magnetita-net -p magnetitad \
    --all-targets -- -D warnings)
(cd "$suite_root/celestina-rs" && cargo test --locked \
    -p celestina-core -p magnetita-core -p magnetita-net -p magnetitad)
"$suite_root/scripts/qmllint-cxxqt.sh" "$project_root"
"$project_root/scripts/smoke.sh" --binary "$binary"

python3 "$artifact_tool" record-verification magnetita \
    --verify-command 'scripts/test-production-artifacts.sh' \
    --verify-command 'bash scripts/check-architecture-contract.sh' \
    --verify-command 'cargo fmt/clippy/test --manifest-path magnetita/Cargo.toml' \
    --verify-command 'cargo fmt --manifest-path celestina-rs/Cargo.toml --all --check' \
    --verify-command 'cargo clippy --manifest-path celestina-rs/Cargo.toml -p celestina-core -p magnetita-core -p magnetita-net -p magnetitad --all-targets --locked -- -D warnings' \
    --verify-command 'cargo test --manifest-path celestina-rs/Cargo.toml -p celestina-core -p magnetita-core -p magnetita-net -p magnetitad' \
    --verify-command 'scripts/qmllint-cxxqt.sh magnetita' \
    --verify-command 'magnetita/scripts/smoke.sh --binary magnetita/target/release/magnetita'

echo ">> bundle Magnetita + magnetitad vigente y verificado; servicio intacto"
