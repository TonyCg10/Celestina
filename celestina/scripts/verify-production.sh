#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
build_dir=$project_root/build
artifact_tool=$suite_root/scripts/production_artifact.py
style_root=$suite_root/celestina-style

python3 "$artifact_tool" check celestina
"$style_root/scripts/verify-production.sh"
"$suite_root/scripts/test-production-artifacts.sh"
bash "$suite_root/scripts/check-architecture-contract.sh"
(cd "$project_root" && cargo fmt --all --check)
(cd "$project_root" && cargo clippy --all-targets --locked -- -D warnings)
(cd "$project_root" && cargo test --all-targets --locked)
(cd "$suite_root/celestina-rs" && cargo fmt --all --check)
(cd "$suite_root/celestina-rs" && cargo clippy --locked \
    -p celestina-shell-core -p celestina-core -p magnetita-core \
    --all-targets -- -D warnings)
(cd "$suite_root/celestina-rs" && \
    cargo test --locked -p celestina-shell-core -p celestina-core -p magnetita-core)
"$project_root/scripts/qmllint-production.sh"
ctest --test-dir "$build_dir" --output-on-failure
"$project_root/scripts/smoke-production.sh"

missing=$(
    ldd "$build_dir/celestina" \
        "$style_root/build/libcelestina-style.so" \
        "$style_root/build/CelestinaStyle/libcelestina-style-plugin.so" \
        | grep 'not found' || true
)
if [ -n "$missing" ]; then
    echo "verify-production: dependencias dinámicas ausentes:" >&2
    echo "$missing" >&2
    exit 1
fi

python3 "$artifact_tool" record-verification celestina \
    --verify-command 'celestina-style/scripts/verify-production.sh' \
    --verify-command 'scripts/test-production-artifacts.sh' \
    --verify-command 'bash scripts/check-architecture-contract.sh' \
    --verify-command 'cargo fmt/clippy/test --manifest-path celestina/Cargo.toml' \
    --verify-command 'cargo fmt --manifest-path celestina-rs/Cargo.toml --all --check' \
    --verify-command 'cargo clippy --manifest-path celestina-rs/Cargo.toml -p celestina-shell-core -p celestina-core -p magnetita-core --all-targets --locked -- -D warnings' \
    --verify-command 'cargo test -p celestina-shell-core -p celestina-core -p magnetita-core' \
    --verify-command 'celestina/scripts/qmllint-production.sh' \
    --verify-command 'ctest --test-dir celestina/build --output-on-failure' \
    --verify-command 'celestina/scripts/smoke-production.sh' \
    --verify-command 'ldd celestina host and CelestinaStyle libraries'

echo ">> bundle Celestina vigente y verificado; no se activó la sesión"
