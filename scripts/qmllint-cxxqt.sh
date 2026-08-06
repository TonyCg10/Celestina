#!/bin/sh
set -eu

# Run Qt's qmllint against a CXX-Qt application's QML, using the module the
# release build generated, and ratchet the surviving warnings.
#
# Two properties matter here and neither used to hold:
#
#   * The linter's verdict must survive batching. `find -print0 | xargs` splits
#     the invocation once the argument list is long enough. GNU xargs does
#     report 123 when any invocation fails, but the script consumed `$?` from a
#     pipeline whose left-hand side was unchecked, and the reported code was
#     then neither the linter's nor obviously a batch verdict. The status is now
#     accumulated explicitly, one batch at a time.
#
#   * Warnings must not be allowed to grow. Architecture and language debt are
#     both ratcheted; qmllint warnings were merely counted and always printed
#     OK, so they could grow without limit and nobody would be told.
#
# The ratchet lives in this script rather than in `commit_scope.py` beside the
# other two. Those ratchets are files a commit hook can measure from the index;
# this one is a property of a release build that only exists after
# `build-production.sh` has run, and a commit hook must not build. The rule is
# the same — a row may only fall, and it falls in the commit that earns it —
# but the only honest place to enforce it is where the number is produced.

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
baseline_file=${QMLLINT_BASELINE_FILE:-$script_dir/qmllint-baseline.tsv}

warning_baseline() {
    # Prints the recorded maximum for a project, or nothing if it has no row.
    awk -F '\t' -v project="$1" '
        /^#/ || NF == 0 { next }
        NF != 2 { printf "invalid\n"; exit }
        $2 == project { print $1; found = 1 }
        END { if (!found) exit 0 }
    ' "$baseline_file"
}

check_warning_ratchet() {
    project=$1
    warnings=$2

    if [ ! -f "$baseline_file" ]; then
        echo "qmllint-production: missing warning baseline $baseline_file" >&2
        return 1
    fi

    recorded=$(warning_baseline "$project")
    case $recorded in
        '')
            echo "qmllint-production: $project has no row in $baseline_file;" \
                "add '$warnings	$project' to record its current warnings" >&2
            return 1
            ;;
        *[!0-9]*)
            echo "qmllint-production: $baseline_file: invalid row for $project" >&2
            return 1
            ;;
    esac

    if [ "$warnings" -gt "$recorded" ]; then
        echo "qmllint-production: $project: warnings grew from $recorded to" \
            "$warnings; inventoried qmllint debt may not grow" >&2
        return 1
    fi
    if [ "$warnings" -lt "$recorded" ]; then
        echo "qmllint-production: $project: warnings fell from $recorded to" \
            "$warnings; lower its row in $baseline_file to $warnings in this" \
            "same commit to lock the improvement in" >&2
        return 1
    fi
    return 0
}

# Internal entry point so the ratchet can be tested without a release build.
if [ "${1-}" = '--check-warning-ratchet' ]; then
    if [ "$#" -ne 3 ]; then
        echo "usage: scripts/qmllint-cxxqt.sh --check-warning-ratchet PROJECT COUNT" >&2
        exit 2
    fi
    check_warning_ratchet "$2" "$3"
    exit "$?"
fi

if [ "$#" -ne 1 ]; then
    echo "usage: scripts/qmllint-cxxqt.sh PROJECT_PATH" >&2
    exit 2
fi

app_root=$(CDPATH= cd -- "$1" && pwd)
project=$(basename -- "$app_root")
qml_root=$app_root/qml
module_root=$(
    find "$app_root/target/release/build" \
        -path '*/out/qt-build-utils/qml_modules' -type d \
        -printf '%T@ %p\n' 2>/dev/null \
    | sort -nr \
    | sed -n '1s/^[^ ]* //p'
)

if [ -z "$module_root" ]; then
    echo "qmllint-production: the generated release QML module is missing; run build-production.sh" >&2
    exit 1
fi

uri=$(sed -n 's/^module //p' "$module_root"/*/qmldir "$module_root"/*/*/qmldir "$module_root"/*/*/*/qmldir 2>/dev/null | head -1)
if [ -z "$uri" ]; then
    echo "qmllint-production: the release output declares no module URI" >&2
    exit 1
fi
module_relative=$(printf '%s' "$uri" | tr . /)
generated_module=$module_root/$module_relative
if [ ! -f "$generated_module/qmldir" ] || [ ! -f "$generated_module/plugin.qmltypes" ]; then
    echo "qmllint-production: qmldir/plugin.qmltypes are missing for $uri" >&2
    exit 1
fi

linter=${QMLLINT:-}
if [ -z "$linter" ]; then
    for candidate in \
        "$(qtpaths6 --query QT_INSTALL_BINS 2>/dev/null || true)/qmllint" \
        "$(qmake6 -query QT_INSTALL_BINS 2>/dev/null || true)/qmllint" \
        /usr/lib/qt6/bin/qmllint \
        "$(command -v qmllint 2>/dev/null || true)"
    do
        if [ -x "$candidate" ]; then
            linter=$candidate
            break
        fi
    done
fi
if [ -z "$linter" ]; then
    echo "qmllint-production: Qt 6's qmllint is missing (set QMLLINT)" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
scratch_module=$scratch/imports/$module_relative
mkdir -p "$scratch_module"
cp "$generated_module/qmldir" "$generated_module/plugin.qmltypes" "$scratch_module/"
ln -s "$qml_root" "$scratch_module/qml"
log=$scratch/qmllint.log
sources=$scratch/sources
: > "$log"

# Collect the inputs first: a failure of `find` inside a pipeline is invisible,
# and an empty list would lint nothing and report success.
if ! find "$qml_root" \( -type f -o -type l \) -name '*.qml' -print0 > "$sources"; then
    echo "qmllint-production: could not enumerate the QML under $qml_root" >&2
    exit 1
fi
if [ ! -s "$sources" ]; then
    echo "qmllint-production: found no QML under $qml_root" >&2
    exit 1
fi

# Keep the batched invocation — qmllint's diagnostics depend on seeing the
# whole set at once — but stop reading the verdict out of `$?`. Once the list
# is long enough for xargs to split it, that status describes one batch. Each
# batch now records its own failure, so no batch can be lost.
failure_marker=$scratch/failed
export QMLLINT_FAILURE_MARKER=$failure_marker
xargs -0 -a "$sources" sh -c '
    linter=$1
    imports=$2
    shift 2
    "$linter" -I "$imports" "$@" || echo failed >> "$QMLLINT_FAILURE_MARKER"
' sh "$linter" "$scratch/imports" >>"$log" 2>&1 || true

if [ -s "$failure_marker" ]; then
    echo "qmllint-production: failed for $uri" >&2
    cat "$log" >&2
    exit 1
fi

warnings=$(grep -c '^Warning:' "$log" || true)
if ! check_warning_ratchet "$project" "$warnings"; then
    cat "$log" >&2
    exit 1
fi
echo "qmllint-production: OK — $uri ($warnings non-fatal baseline warning(s))"
