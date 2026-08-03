#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
build_dir=$project_root/build
style_root=$suite_root/celestina-style

"$style_root/scripts/build-production.sh"
cmake -S "$project_root" -B "$build_dir" \
    -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build "$build_dir" --parallel

python3 "$suite_root/scripts/production_artifact.py" record-build celestina \
    --build-command 'celestina-style/scripts/build-production.sh' \
    --build-command 'cmake -S celestina -B celestina/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON' \
    --build-command 'cmake --build celestina/build --parallel'

echo ">> bundle release listo en $build_dir (sin activar ni instalar)"
