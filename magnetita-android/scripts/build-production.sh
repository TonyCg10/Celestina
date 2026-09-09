#!/bin/sh
# Builds the debug APK with the Rust core compiled by Gradle's native task.
# A signed release build is AND-1-D's; until then the debug build is the
# artifact the author installs by hand.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$here"
./gradlew --no-daemon -q assembleDebug
echo ">> Magnetita Android debug build: app/build/outputs/apk/debug/app-debug.apk"
