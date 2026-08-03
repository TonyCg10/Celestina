#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
build_dir=$project_root/build

cmake -S "$project_root" -B "$build_dir" \
    -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build "$build_dir" --parallel

python3 "$suite_root/scripts/production_artifact.py" record-build celestina-style \
    --build-command 'cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON' \
    --build-command 'cmake --build celestina-style/build --parallel'

echo ">> módulo release listo en $build_dir (sin instalar)"

