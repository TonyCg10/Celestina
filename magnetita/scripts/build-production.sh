#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
rust_workspace=$suite_root/celestina-rs

(cd "$project_root" && cargo build --release --locked --bin magnetita)
(cd "$rust_workspace" && cargo build --release --locked -p magnetitad)

python3 "$suite_root/scripts/production_artifact.py" record-build magnetita \
    --build-command 'cargo build --manifest-path magnetita/Cargo.toml --release --locked --bin magnetita' \
    --build-command 'cargo build --manifest-path celestina-rs/Cargo.toml --release --locked -p magnetitad'

echo ">> bundle Magnetita + magnetitad listo (sin instalar ni tocar el servicio)"

