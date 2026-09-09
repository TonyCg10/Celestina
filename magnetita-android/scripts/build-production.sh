#!/bin/sh
# Builds the release APK with the Rust core compiled by Gradle's native task,
# sealed by the suite's production runner like every other project. The
# release is signed with the author's key when keystore.properties names it,
# and with the debug key otherwise; the artifact goes to the phone by hand.
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

(cd "$project_root" && ./gradlew --no-daemon -q assembleRelease)

echo ">> Magnetita Android release build completed (not installed)"
