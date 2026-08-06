#!/bin/sh
set -eu

suite_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$suite_root/scripts/production-common.sh"

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM

source_file=$scratch/source.bin
template_file=$scratch/template.in
source_tree=$scratch/source-tree
destination_tree=$scratch/destination-tree
trap_log=$scratch/outer-trap.log

printf 'release bytes\n' > "$source_file"
printf 'Exec=@BIN@\n' > "$template_file"
mkdir -p "$source_tree/nested" "$destination_tree"
printf 'new\n' > "$source_tree/nested/current"
printf 'stale\n' > "$destination_tree/stale"

(
    trap 'printf "outer trap preserved\n" > "$trap_log"' EXIT
    production_install_file "$source_file" "$scratch/install/bin/demo" 0755
    production_install_template \
        "$template_file" "$scratch/install/demo.desktop" '@BIN@' \
        "$scratch/a&b|demo"
    production_install_tree "$source_tree" "$destination_tree"
)

[ "$(cat "$trap_log")" = 'outer trap preserved' ]
[ "$(cat "$scratch/install/bin/demo")" = 'release bytes' ]
[ "$(cat "$scratch/install/demo.desktop")" = "Exec=$scratch/a&b|demo" ]
[ "$(cat "$destination_tree/nested/current")" = 'new' ]
[ ! -e "$destination_tree/stale" ]

printf 'keep file\n' > "$scratch/keep-file"
if production_install_file "$scratch/missing" "$scratch/keep-file" 0644 2>/dev/null; then
    echo "production-common fixture: install_file aceptó un origen ausente" >&2
    exit 1
fi
[ "$(cat "$scratch/keep-file")" = 'keep file' ]

printf 'keep template\n' > "$scratch/keep-template"
if production_install_template \
    "$scratch/missing-template" "$scratch/keep-template" '@X@' replacement \
    2>/dev/null; then
    echo "production-common fixture: install_template aceptó un origen ausente" >&2
    exit 1
fi
[ "$(cat "$scratch/keep-template")" = 'keep template' ]

mkdir -p "$scratch/keep-tree"
printf 'keep tree\n' > "$scratch/keep-tree/sentinel"
if production_install_tree "$scratch/missing-tree" "$scratch/keep-tree" 2>/dev/null; then
    echo "production-common fixture: install_tree aceptó un origen ausente" >&2
    exit 1
fi
[ "$(cat "$scratch/keep-tree/sentinel")" = 'keep tree' ]

fake_bin=$scratch/fake-bin
signal_source=$scratch/signal-source
signal_destination=$scratch/signal-destination
mkdir -p "$fake_bin" "$signal_source" "$signal_destination"
printf 'replacement\n' > "$signal_source/current"
printf 'original\n' > "$signal_destination/sentinel"
printf '%s\n' \
    '#!/bin/sh' \
    'set -eu' \
    'if [ "$1" = "$FIXTURE_SWAP_DEST" ]; then' \
    '    /usr/bin/mv "$@"' \
    '    kill -TERM "$PPID"' \
    '    sleep 1' \
    '    exit 143' \
    'fi' \
    'exec /usr/bin/mv "$@"' \
    > "$fake_bin/mv"
chmod 0755 "$fake_bin/mv"

if PATH=$fake_bin:$PATH FIXTURE_SWAP_DEST=$signal_destination \
    production_install_tree "$signal_source" "$signal_destination" 2>/dev/null; then
    echo "production-common fixture: install_tree ignoró TERM durante el swap" >&2
    exit 1
fi
[ "$(cat "$signal_destination/sentinel")" = 'original' ]
[ ! -e "$signal_destination/current" ]

leftovers=$(find "$scratch" -name '.production-*' -print)
if [ -n "$leftovers" ]; then
    echo "production-common fixture: quedaron temporales tras fallo/señal:" >&2
    echo "$leftovers" >&2
    exit 1
fi

# --- qmllint warning ratchet and batch-boundary status ------------------------
# The linter used to always print OK, so its warnings could grow without limit,
# and it read its verdict out of `$?` after an `xargs` pipeline that splits once
# the argument list is long enough.

qmllint_script=$suite_root/scripts/qmllint-cxxqt.sh
ratchet_baseline=$scratch/qmllint-baseline.tsv
printf '%s\n' '# warnings<TAB>project' '10	demo' > "$ratchet_baseline"

ratchet() {
    QMLLINT_BASELINE_FILE=$ratchet_baseline \
        sh "$qmllint_script" --check-warning-ratchet "$1" "$2" 2>"$scratch/ratchet.log"
}

if ! ratchet demo 10; then
    echo "qmllint fixture: the recorded warning count was rejected" >&2
    exit 1
fi
if ratchet demo 11; then
    echo "qmllint fixture: a grown warning count was accepted" >&2
    exit 1
fi
grep -q 'may not grow' "$scratch/ratchet.log" || {
    echo "qmllint fixture: growth failed without a ratchet diagnostic" >&2
    exit 1
}
if ratchet demo 9; then
    echo "qmllint fixture: an unrecorded improvement was accepted" >&2
    exit 1
fi
grep -q 'lower its row' "$scratch/ratchet.log" || {
    echo "qmllint fixture: an improvement failed without a lower-the-row diagnostic" >&2
    exit 1
}
if ratchet unregistered 0; then
    echo "qmllint fixture: a project without a baseline row was accepted" >&2
    exit 1
fi
if QMLLINT_BASELINE_FILE=$scratch/absent.tsv \
    sh "$qmllint_script" --check-warning-ratchet demo 10 2>/dev/null; then
    echo "qmllint fixture: a missing baseline file was accepted" >&2
    exit 1
fi

# A failure in a non-final xargs batch must still fail the run. The fixture
# builds enough long paths that xargs splits the invocation, and makes the fake
# linter refuse exactly one file placed at the front of the list.
lint_app=$scratch/lint-app
module_root=$lint_app/target/release/build/demo-hash/out/qt-build-utils/qml_modules
mkdir -p "$module_root/org/example" "$lint_app/qml"
printf 'module org.example\n' > "$module_root/org/example/qmldir"
: > "$module_root/org/example/plugin.qmltypes"

printf '%s\n' 'import QtQuick' > "$lint_app/qml/AAA-refused.qml"
filler=$lint_app/qml
depth=0
while [ "$depth" -lt 6 ]; do
    filler=$filler/a-directory-with-a-deliberately-long-name-to-lengthen-paths
    depth=$((depth + 1))
done
mkdir -p "$filler"
index=0
while [ "$index" -lt 400 ]; do
    printf 'import QtQuick\n' \
        > "$filler/Filler-with-a-deliberately-long-file-name-$index.qml"
    index=$((index + 1))
done

fake_linter=$scratch/fake-qmllint
cat > "$fake_linter" <<'FAKE'
#!/bin/sh
status=0
for argument do
    case $argument in
        *AAA-refused.qml) status=1 ;;
    esac
done
exit "$status"
FAKE
chmod 0755 "$fake_linter"

printf '%s\n' '# warnings<TAB>project' '0	lint-app' > "$scratch/lint-baseline.tsv"
if QMLLINT=$fake_linter QMLLINT_BASELINE_FILE=$scratch/lint-baseline.tsv \
    sh "$qmllint_script" "$lint_app" >/dev/null 2>&1; then
    echo "qmllint fixture: a refused file in a non-final batch reported OK" >&2
    exit 1
fi

echo "production-common fixtures: OK"
