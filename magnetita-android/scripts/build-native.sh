#!/bin/sh
# Cross-compiles magnetita-mobile for arm64 Android and generates its Kotlin
# bindings into "$1/jniLibs/arm64-v8a" and "$1/kotlin". Gradle calls this
# before compiling; it can also be run by hand.
#
# Tooling, all the author's: rustup with the aarch64-linux-android target on
# the workspace's pinned toolchain, cargo-ndk, and an NDK found under
# ANDROID_NDK_HOME, or under the SDK named by ANDROID_HOME or local.properties.
set -eu

out=${1:?output directory}
here=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
workspace=$here/../celestina-rs

find_sdk() {
    if [ -n "${ANDROID_HOME:-}" ]; then
        printf '%s\n' "$ANDROID_HOME"
    elif [ -f "$here/local.properties" ]; then
        sed -n 's/^sdk.dir=//p' "$here/local.properties" | sed 's#\\:#:#g'
    fi
}

if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    sdk=$(find_sdk)
    [ -n "$sdk" ] || { echo "build-native: no ANDROID_NDK_HOME, ANDROID_HOME or local.properties" >&2; exit 1; }
    ANDROID_NDK_HOME=$(ls -d "$sdk"/ndk/* 2>/dev/null | sort -V | tail -1)
    [ -n "$ANDROID_NDK_HOME" ] || { echo "build-native: no NDK under $sdk/ndk" >&2; exit 1; }
    export ANDROID_NDK_HOME
fi

case ":$PATH:" in *":$HOME/.local/share/cargo/bin:"*) ;; *) PATH=$HOME/.local/share/cargo/bin:$HOME/.cargo/bin:$PATH ;; esac
export PATH

mkdir -p "$out/jniLibs" "$out/kotlin"
cd "$workspace"
cargo ndk -t arm64-v8a -o "$out/jniLibs" build --release -p magnetita-mobile
cargo build --release -p magnetita-mobile
cargo run -q --release -p magnetita-mobile --bin uniffi-bindgen -- \
    generate --library target/release/libmagnetita_mobile.so \
    --language kotlin --no-format --out-dir "$out/kotlin"
echo "build-native: $(ls "$out/jniLibs/arm64-v8a") and bindings in $out/kotlin"
