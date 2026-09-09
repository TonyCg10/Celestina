#!/bin/sh
# Reports whether the debug artifact exists and whether sources changed since.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
apk=$here/app/build/outputs/apk/debug/app-debug.apk
if [ ! -f "$apk" ]; then
    echo "artifact: none (run scripts/build-production.sh)"
    exit 0
fi
newer=$(find "$here/app/src" "$here/scripts" "$here/../celestina-rs/crates/magnetita-mobile/src" -type f -newer "$apk" | wc -l)
if [ "$newer" -eq 0 ]; then
    echo "artifact: magnetita-android debug current"
else
    echo "artifact: magnetita-android debug stale ($newer newer source file(s))"
fi
