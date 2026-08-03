#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)

(cd "$project_root" && cargo build --release --locked --bin grafita)

python3 "$suite_root/scripts/production_artifact.py" record-build grafita \
    --build-command 'cargo build --manifest-path grafita/Cargo.toml --release --locked --bin grafita'

echo ">> Grafita release lista en $project_root/target/release/grafita (sin instalar)"

