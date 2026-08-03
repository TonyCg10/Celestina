#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)

(cd "$project_root" && cargo build --release --locked --bin siderita)

python3 "$suite_root/scripts/production_artifact.py" record-build siderita \
    --build-command 'cargo build --manifest-path siderita/Cargo.toml --release --locked --bin siderita'

echo ">> Siderita release lista en $project_root/target/release/siderita (sin instalar)"

