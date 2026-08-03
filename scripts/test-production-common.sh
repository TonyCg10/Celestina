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

echo "production-common fixtures: OK"
