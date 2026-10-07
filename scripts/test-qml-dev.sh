#!/bin/sh
set -eu

# Fixture tests for scripts/qml-dev.sh: a fake application with a generated
# release module, a fake cargo that names no target directory (so the module
# is read from <app>/target), and a fake binary that reports what it was given.

suite_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT HUP INT TERM
temporary=$(CDPATH= cd -- "$temporary" && pwd -P)

fail() {
    printf 'test-qml-dev: FAIL: %s\n' "$*" >&2
    exit 1
}

# The script resolves applications next to its own scripts/ directory, so the
# fixture is a small suite with a copy of both scripts.
suite=$temporary/suite
mkdir -p "$suite/scripts" "$temporary/bin" "$temporary/runtime"
cp "$suite_root/scripts/qml-dev.sh" "$suite_root/scripts/qmllint-cxxqt.sh" "$suite/scripts/"
app=$suite/demo
mkdir -p "$app/qml"
printf 'import QtQuick\nItem {}\n' > "$app/qml/Main.qml"
printf 'fn main() { QmlModule::new("org.celestina.demo"); }\n' > "$app/build.rs"
printf '[package]\nname = "demo"\n' > "$app/Cargo.toml"
generated=$app/target/release/build/demo-0123456789abcdef/out/qt-build-utils/qml_modules/org/celestina/demo
mkdir -p "$generated"
printf 'module org.celestina.demo\nprefer :/qt/qml/org/celestina/demo/\nsingleton CelestinaTheme 1.0 qml/CelestinaTheme.qml\n' \
    > "$generated/qmldir"
printf 'Module {}\n' > "$generated/plugin.qmltypes"

printf '#!/bin/sh\nexit 101\n' > "$temporary/bin/cargo"
chmod 0755 "$temporary/bin/cargo"
binary=$temporary/demo-binary
cat > "$binary" <<'FAKE'
#!/bin/sh
printf 'import=%s\nmain=%s\nargs=%s\n' "$CELESTINA_QML_DEV_IMPORT" "$CELESTINA_QML_DEV_MAIN" "$*"
FAKE
chmod 0755 "$binary"

run() {
    status=0
    PATH=$temporary/bin:$PATH XDG_RUNTIME_DIR=$temporary/runtime \
        sh "$suite/scripts/qml-dev.sh" "$@" > "$temporary/out" 2> "$temporary/err" || status=$?
}

# 1. The tree mirrors the module, links the source QML and drops `prefer`.
run demo --binary "$binary" -- --flag value
[ "$status" -eq 0 ] || fail "qml-dev demo exited $status: $(cat "$temporary/err")"
tree=$temporary/runtime/celestina-qml-dev/demo
module=$tree/org/celestina/demo
grep -qx "import=$tree" "$temporary/out" || fail "wrong import root: $(cat "$temporary/out")"
grep -qx "main=file://$module/qml/Main.qml" "$temporary/out" || fail "wrong Main.qml: $(cat "$temporary/out")"
grep -qx "args=--flag value" "$temporary/out" || fail "arguments were not passed on"
[ "$(readlink "$module/qml")" = "$app/qml" ] || fail "qml does not link to the source"
grep -q '^prefer ' "$module/qmldir" && fail "the prefer line survived"
grep -qx 'singleton CelestinaTheme 1.0 qml/CelestinaTheme.qml' "$module/qmldir" \
    || fail "the qmldir lost its entries"
cmp -s "$generated/plugin.qmltypes" "$module/plugin.qmltypes" || fail "plugin.qmltypes differs"
printf 'ok %s\n' "the tree mirrors the module with the source QML and no prefer line"

# 2. A second run replaces the tree instead of mixing in stale files.
printf 'stale\n' > "$module/stale.qml"
run demo --binary "$binary"
[ "$status" -eq 0 ] || fail "second run exited $status"
[ ! -e "$module/stale.qml" ] || fail "a stale file survived the second run"
printf 'ok %s\n' "each run lays the tree out afresh"

# 3. Refusals: no module built yet, an unknown application, a missing binary.
rm -rf "$app/target"
run demo --binary "$binary"
[ "$status" -eq 1 ] || fail "a missing module exited $status"
grep -q 'build demo once' "$temporary/err" || fail "missing module message: $(cat "$temporary/err")"
run nothing --binary "$binary"
[ "$status" -eq 1 ] || fail "an unknown application exited $status"
run demo --binary "$temporary/missing"
[ "$status" -eq 1 ] || fail "a missing binary exited $status"
run
[ "$status" -eq 2 ] || fail "no arguments exited $status"
run ../demo
[ "$status" -eq 2 ] || fail "a path as the application exited $status"
printf 'ok %s\n' "qml-dev refuses what it cannot lay out"
