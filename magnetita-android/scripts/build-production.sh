#!/bin/sh
# Builds the debug APK with the Rust core compiled by Gradle's native task,
# sealed by the suite's production runner like every other project. A
# signed release build is AND-1-D's; until then the debug build is the
# artifact the author installs by hand.
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
artifact_tool=$suite_root/scripts/production_artifact.py

if [ "$#" -eq 0 ]; then
    exec python3 "$artifact_tool" run-build magnetita-android
fi
if [ "$#" -ne 1 ] || [ "$1" != "--production-runner-internal" ] || \
    [ "${CELESTINA_PRODUCTION_RUNNER_PHASE:-}" != "build" ]; then
    echo "build-production: internal mode is reserved for the production runner" >&2
    exit 2
fi

(cd "$project_root" && ./gradlew --no-daemon -q assembleDebug)

echo ">> Magnetita Android debug build completed (not installed)"
