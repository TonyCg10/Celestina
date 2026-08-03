#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)

(cd "$project_root" && cargo build --release --locked --bin fluorita)

python3 "$suite_root/scripts/production_artifact.py" record-build fluorita \
    --build-command 'cargo build --manifest-path fluorita/Cargo.toml --release --locked --bin fluorita'

echo ">> Fluorita release lista en $project_root/target/release/fluorita (sin instalar)"
