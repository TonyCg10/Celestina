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

# 3. The whole lint finds the module under the redirected directory.
module=$shared/release/build/demo-hash/out/qt-build-utils/qml_modules/org/example
mkdir -p "$module"
printf 'module org.example\n' > "$module/qmldir"
: > "$module/plugin.qmltypes"
printf '#!/bin/sh\nexit 0\n' > "$temporary/fake-qmllint"
chmod 0755 "$temporary/fake-qmllint"
printf '%s\n' '# warnings<TAB>project' '0	app' > "$temporary/baseline.tsv"
if ! FAKE_CARGO_MODE=shared FAKE_TARGET=$shared FAKE_CARGO_LOG=$temporary/cargo.log \
    PATH=$bin:$PATH QMLLINT=$temporary/fake-qmllint \
    QMLLINT_BASELINE_FILE=$temporary/baseline.tsv \
    sh "$qmllint_script" "$app" > "$temporary/lint.out" 2>&1; then
    fail "the lint did not find the redirected module: $(cat "$temporary/lint.out")"
fi
grep -F 'qmllint-production: OK' "$temporary/lint.out" >/dev/null \
    || fail "the lint did not report OK: $(cat "$temporary/lint.out")"
printf 'ok %s\n' "the lint reads the release module from the redirected target directory"
