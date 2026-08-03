#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)

(cd "$project_root" && cargo build --workspace --release --locked)

python3 "$suite_root/scripts/production_artifact.py" record-build celestina-rs \
    --build-command 'cargo build --manifest-path celestina-rs/Cargo.toml --workspace --release --locked'

echo ">> workspace Rust release listo en $project_root/target (sin instalar)"

