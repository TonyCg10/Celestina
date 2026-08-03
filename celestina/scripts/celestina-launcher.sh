#!/bin/sh
set -eu

launcher_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=$(CDPATH= cd -- "$launcher_dir/.." && pwd)
bundle=$prefix/libexec/celestina

export CELESTINA_NIRI_ADAPTER_PATH=$bundle/celestina-niri-adapter
export CELESTINA_PROVIDER_ADAPTER_PATH=$bundle/celestina-provider-adapter
export CELESTINA_STYLE_PATH=$bundle/CelestinaStyle
if [ -n "${LD_LIBRARY_PATH:-}" ]; then
    export LD_LIBRARY_PATH=$bundle:$LD_LIBRARY_PATH
else
    export LD_LIBRARY_PATH=$bundle
fi

exec "$bundle/celestina" "$@"
