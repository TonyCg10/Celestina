#!/bin/sh
# Verifies the tree behind the release artifact: the Rust core's tests, the
# app's unit tests, lint, and that the artifact exists. Never installs anything.
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py
apk=$project_root/app/build/outputs/apk/release/app-release.apk

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-verification magnetita-android
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "verify" ]; then
    echo "verify-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

(cd "$suite_root/celestina-rs" && cargo test -q --locked -p magnetita-mobile -p magnetita-peer)
(cd "$project_root" && ./gradlew --no-daemon -q testDebugUnitTest lintRelease)
[ -f "$apk" ] || { echo "verify: no release artifact; run build-production.sh" >&2; exit 1; }

echo ">> Magnetita Android verification steps completed"
