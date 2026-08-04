#!/usr/bin/env bash

set -uo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
cd "$repo_root"

contract_manifest=''
if ! contract_manifest=$(mktemp); then
    echo "Could not create the temporary QML contract manifest." >&2
    exit 1
fi
trap 'rm -f -- "$contract_manifest"' EXIT

if ! find siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml celestina-style \
    -path '*/build' -prune -o \
    -type f -name '*.qml' \
    ! -path 'celestina-style/CelestinaTheme.qml' \
    -print0 > "$contract_manifest"; then
    echo "Could not enumerate the complete QML tree." >&2
    exit 1
fi

contract_files=()
while IFS= read -r -d '' file; do
    contract_files+=("$file")
done < "$contract_manifest"

if ((${#contract_files[@]} == 0)); then
    echo "Found no QML files to audit." >&2
    exit 1
fi

failures=0

if structural_hits=$(python3 scripts/architecture_scanners.py qml-style-contract \
    celestina-style/CelestinaTheme.qml \
    siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml celestina-style); then
    if [[ -n $structural_hits ]]; then
        printf '%s\n' "$structural_hits"
        printf 'ERROR: the structural scanner found direct visual values.\n\n' >&2
        failures=1
    fi
else
    printf 'ERROR: the structural scanner could not complete the visual contract.\n\n' >&2
    failures=1
fi

if copy_hits=$(python3 scripts/architecture_scanners.py style-copies \
    celestina-style siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml); then
    if [[ -n $copy_hits ]]; then
        printf '%s\n' "$copy_hits"
        printf 'ERROR: a renamed copy bypasses the celestina-style links.\n\n' >&2
        failures=1
    fi
else
    printf 'ERROR: could not compare the shared style with its consumers.\n\n' >&2
    failures=1
fi

check_pattern() {
    local message=$1
    local pattern=$2
    local hits status

    if hits=$(grep -nEH -- "$pattern" "${contract_files[@]}"); then
        :
    else
        status=$?
        if ((status != 1)); then
            printf 'ERROR: grep could not complete the visual contract (%s).\n\n' \
                "$message" >&2
            failures=1
            return
        fi
    fi
    if [[ -n $hits ]]; then
        printf '%s\n' "$hits"
        printf 'ERROR: %s\n\n' "$message" >&2
        failures=1
    fi
}

# CelestinaTheme.qml is deliberately excluded: it is the canonical place where
# primitive values and derivation recipes live. App and component QML consume
# semantic tokens only.
check_pattern \
    'hexadecimal literal outside CelestinaTheme.qml' \
    '#[[:xdigit:]]{3,8}'
check_pattern \
    'named color outside CelestinaTheme.qml; use a semantic token' \
    "(color|border\\.color|selectionColor|selectedTextColor|placeholderTextColor|fillColor)[[:space:]]*:[[:space:]]*['\"][[:alpha:]][[:alnum:]_-]*['\"]"
check_pattern \
    'local color transformation; derive the state in CelestinaTheme.qml' \
    'Qt\.(rgba|darker|lighter|tint)[[:space:]]*\('
check_pattern \
    'ref.* access from a consumer; use a semantic sys.* role' \
    'CelestinaTheme\.ref([^[:alnum:]_]|$)'

# These values affect the visual language globally. Numeric layout coordinates
# remain local by design, but visual anatomy and state must be tokenized.
check_pattern \
    'direct animation duration; use the motion* scale' \
    'duration[[:space:]]*:[[:space:]]*[0-9]'
check_pattern \
    'direct animation curve; use an ease* token' \
    'easing\.type[[:space:]]*:[[:space:]]*Easing\.'
check_pattern \
    'direct type size; use a font* role' \
    'font\.pixelSize[[:space:]]*:[[:space:]]*[0-9]'
check_pattern \
    'direct type weight; use a weight* role' \
    'font\.weight[[:space:]]*:[[:space:]]*[0-9]|Font\.(Normal|Medium|DemiBold|Bold)'
check_pattern \
    'direct type tracking; use a typography token' \
    'font\.letterSpacing[[:space:]]*:[[:space:]]*[0-9]'
check_pattern \
    'direct visual radius; use radius* or a capsule derived from height' \
    'radius[[:space:]]*:[[:space:]]*[0-9]'
check_pattern \
    'direct border thickness; use borderHairline or borderFocus' \
    'border\.width[[:space:]]*:[^;]*[[:space:]?:][1-9][0-9]*(\.[0-9]+)?([[:space:]?:)]|$)'
check_pattern \
    'direct visual padding; use space* or a comp* metric' \
    '(^|[^[:alnum:]_])(leftPadding|rightPadding|topPadding|bottomPadding|padding)[[:space:]]*:[[:space:]]*[1-9][0-9]*(\.[0-9]+)?([[:space:]]|$)'
check_pattern \
    'direct visual opacity; use a state/emphasis token' \
    'opacity[[:space:]]*:[^;]*[[:space:]?:]0\.[0-9]+'

if ! python3 celestina-style/scripts/check-contrast-contract.py; then
    failures=1
fi

if ((failures)); then
    exit 1
fi

echo "QML visual contract: OK"
