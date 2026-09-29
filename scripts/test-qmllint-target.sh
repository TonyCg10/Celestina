#!/bin/sh
set -eu

# Fixture tests for the Cargo target directory scripts/qmllint-cxxqt.sh reads
# the release QML module from. A session worktree's .cargo/config.toml moves
# that directory out of the application, so the script asks Cargo instead of
# assuming <app>/target. A fake cargo on PATH stands in for Cargo.

suite_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
qmllint_script=$suite_root/scripts/qmllint-cxxqt.sh
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT HUP INT TERM
temporary=$(CDPATH= cd -- "$temporary" && pwd -P)

fail() {
    printf 'test-qmllint-target: FAIL: %s\n' "$*" >&2
    exit 1
}

app=$temporary/app
shared=$temporary/shared-target
bin=$temporary/bin
mkdir -p "$app/qml" "$bin"
printf 'import QtQuick\n' > "$app/qml/Main.qml"

cat > "$bin/cargo" <<'FAKE'
#!/bin/sh
# Fixture double of cargo: record the call and its directory, then answer as
# FAKE_CARGO_MODE says.
printf '%s|%s\n' "$(pwd -P)" "$*" >> "$FAKE_CARGO_LOG"
case $FAKE_CARGO_MODE in
    shared)
        printf '{"packages":[],"target_directory":"%s","version":1}\n' "$FAKE_TARGET"
        ;;
    garbage) printf 'not metadata\n' ;;
    *) exit 101 ;;
esac
FAKE
chmod 0755 "$bin/cargo"

resolve() {
    FAKE_CARGO_MODE=$1 FAKE_TARGET=$shared FAKE_CARGO_LOG=$temporary/cargo.log \
        PATH=$bin:$PATH sh "$qmllint_script" --print-target-directory "$app"
}

# 1. Cargo's answer wins: it is where a redirected release build wrote.
: > "$temporary/cargo.log"
[ "$(resolve shared)" = "$shared" ] || fail "the reported target directory was ignored"
[ "$(cat "$temporary/cargo.log")" = "$app|metadata --no-deps --format-version 1 --offline" ] \
    || fail "cargo ran as '$(cat "$temporary/cargo.log")'"
printf 'ok %s\n' "the target directory is the one cargo metadata reports from the app root"

# 2. Without an answer from Cargo, the application's own target/ remains.
[ "$(resolve fail)" = "$app/target" ] || fail "a failing cargo did not fall back"
[ "$(resolve garbage)" = "$app/target" ] || fail "unreadable metadata did not fall back"
printf 'ok %s\n' "a failing or unreadable cargo falls back to the app's target/"

# 3. The whole lint finds the module under the redirected directory: the
#    app's own, named by the URI its build.rs declares and built by its
#    package.
printf '[package]\nname = "app"\nversion = "0.1.0"\n' > "$app/Cargo.toml"
printf 'fn main() {\n    let module = QmlModule::new("org.example")\n        .qml_file("qml/Main.qml");\n}\n' \
    > "$app/build.rs"
make_module() {
    # make_module BUILD_DIRECTORY URI
    directory=$shared/release/build/$1/out/qt-build-utils/qml_modules/$(printf '%s' "$2" | tr . /)
    mkdir -p "$directory"
    printf 'module %s\n' "$2" > "$directory/qmldir"
    printf '%s\n' "$1" > "$directory/plugin.qmltypes"
}
make_module app-0123456789abcdef org.example
# The fake linter records whose types it was given: `-I IMPORTS` holds a copy
# of the chosen module's plugin.qmltypes, which names its build directory.
cat > "$temporary/fake-qmllint" <<'LINTER'
#!/bin/sh
[ "$1" = -I ] && cat "$2"/org/*/plugin.qmltypes >> "$QMLLINT_ARGUMENTS"
exit 0
LINTER
chmod 0755 "$temporary/fake-qmllint"
printf '%s\n' '# warnings<TAB>project' '0	app' > "$temporary/baseline.tsv"
lint() {
    : > "$temporary/arguments"
    FAKE_CARGO_MODE=shared FAKE_TARGET=$shared FAKE_CARGO_LOG=$temporary/cargo.log \
        PATH=$bin:$PATH QMLLINT=$temporary/fake-qmllint \
        QMLLINT_ARGUMENTS=$temporary/arguments \
        QMLLINT_BASELINE_FILE=$temporary/baseline.tsv \
        sh "$qmllint_script" "$app" > "$temporary/lint.out" 2>&1
}
if ! lint; then
    fail "the lint did not find the redirected module: $(cat "$temporary/lint.out")"
fi
grep -F 'qmllint-production: OK — org.example' "$temporary/lint.out" >/dev/null \
    || fail "the lint did not report OK: $(cat "$temporary/lint.out")"
printf 'ok %s\n' "the lint reads the release module from the redirected target directory"

# 4. Several applications share the target directory. A newer module of
#    another application, or of a crate whose name only starts with the
#    package's, is not the app's: the lint still reads org.example (TOOL-10).
sleep 1
make_module app-core-00112233445566aa org.example
make_module other-fedcba9876543210 org.other
if ! lint; then
    fail "the lint failed with several modules: $(cat "$temporary/lint.out")"
fi
grep -F "qmllint-production: OK — org.example" "$temporary/lint.out" >/dev/null \
    || fail "the lint chose another module: $(cat "$temporary/lint.out")"
[ "$(sort -u "$temporary/arguments")" = app-0123456789abcdef ] \
    || fail "the lint read another build's types: $(cat "$temporary/arguments")"
printf 'ok %s\n' "the lint picks the app's module by URI and package among several"

# 5. Without the app's module, the lint names what is missing instead of
#    borrowing another application's.
rm -rf -- "$shared/release/build/app-0123456789abcdef"
if lint; then
    fail "the lint passed with only other applications' modules"
fi
grep -F 'the release QML module org.example of app is missing' "$temporary/lint.out" >/dev/null \
    || fail "the missing module produced no stable diagnostic: $(cat "$temporary/lint.out")"
printf 'ok %s\n' "the lint refuses to lint against another application's module"
