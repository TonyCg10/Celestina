#!/bin/sh
# Verifies the tree behind the debug artifact: the Rust core's tests, the
# app's unit tests, and that the artifact exists. Never installs anything.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$here/../celestina-rs"
cargo test -q -p magnetita-mobile -p magnetita-peer
cd "$here"
./gradlew --no-daemon -q testDebugUnitTest
[ -f app/build/outputs/apk/debug/app-debug.apk ] || { echo "verify: no debug artifact; run build-production.sh" >&2; exit 1; }
echo ">> Magnetita Android verification steps completed"
