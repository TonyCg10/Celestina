#!/usr/bin/env bash

set -uo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
cd "$repo_root"

contract_manifest=''
if ! contract_manifest=$(mktemp); then
    echo "No se pudo crear el manifiesto temporal del contrato QML." >&2
    exit 1
fi
trap 'rm -f -- "$contract_manifest"' EXIT

if ! find siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml celestina-style \
    -path '*/build' -prune -o \
    -type f -name '*.qml' \
    ! -path 'celestina-style/CelestinaTheme.qml' \
    -print0 > "$contract_manifest"; then
    echo "No se pudo enumerar el árbol QML completo." >&2
    exit 1
fi

contract_files=()
while IFS= read -r -d '' file; do
    contract_files+=("$file")
done < "$contract_manifest"

if ((${#contract_files[@]} == 0)); then
    echo "No se encontraron archivos QML para auditar." >&2
    exit 1
fi

failures=0

if structural_hits=$(python3 scripts/architecture_scanners.py qml-style-contract \
    celestina-style/CelestinaTheme.qml \
    siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml celestina-style); then
    if [[ -n $structural_hits ]]; then
        printf '%s\n' "$structural_hits"
        printf 'ERROR: el scanner estructural encontro valores visuales directos.\n\n' >&2
        failures=1
    fi
else
    printf 'ERROR: el scanner estructural no pudo completar el contrato visual.\n\n' >&2
    failures=1
fi

if copy_hits=$(python3 scripts/architecture_scanners.py style-copies \
    celestina-style siderita/qml magnetita/qml grafita/qml fluorita/qml celestina/qml); then
    if [[ -n $copy_hits ]]; then
        printf '%s\n' "$copy_hits"
        printf 'ERROR: una copia renombrada evade los enlaces de celestina-style.\n\n' >&2
        failures=1
    fi
else
    printf 'ERROR: no se pudo comparar el estilo compartido con sus consumidores.\n\n' >&2
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
            printf 'ERROR: grep no pudo completar el contrato visual (%s).\n\n' \
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
    'literal hexadecimal fuera de CelestinaTheme.qml' \
    '#[[:xdigit:]]{3,8}'
check_pattern \
    'color nominal fuera de CelestinaTheme.qml; usa un token semántico' \
    "(color|border\\.color|selectionColor|selectedTextColor|placeholderTextColor|fillColor)[[:space:]]*:[[:space:]]*['\"][[:alpha:]][[:alnum:]_-]*['\"]"
check_pattern \
    'transformación de color local; deriva el estado en CelestinaTheme.qml' \
    'Qt\.(rgba|darker|lighter|tint)[[:space:]]*\('
check_pattern \
    'acceso a ref.* desde un consumidor; usa un rol semántico sys.*' \
    'CelestinaTheme\.ref([^[:alnum:]_]|$)'

# These values affect the visual language globally. Numeric layout coordinates
# remain local by design, but visual anatomy and state must be tokenized.
check_pattern \
    'duración de animación directa; usa la escala motion*' \
    'duration[[:space:]]*:[[:space:]]*[0-9]'
check_pattern \
    'curva de animación directa; usa un token ease*' \
    'easing\.type[[:space:]]*:[[:space:]]*Easing\.'
check_pattern \
    'tamaño tipográfico directo; usa un rol font*' \
    'font\.pixelSize[[:space:]]*:[[:space:]]*[0-9]'
check_pattern \
    'peso tipográfico directo; usa un rol weight*' \
    'font\.weight[[:space:]]*:[[:space:]]*[0-9]|Font\.(Normal|Medium|DemiBold|Bold)'
check_pattern \
    'tracking tipográfico directo; usa un token de tipografía' \
    'font\.letterSpacing[[:space:]]*:[[:space:]]*[0-9]'
check_pattern \
    'radio visual directo; usa radius* o una cápsula derivada de height' \
    'radius[[:space:]]*:[[:space:]]*[0-9]'
check_pattern \
    'grosor de borde directo; usa borderHairline o borderFocus' \
    'border\.width[[:space:]]*:[^;]*[[:space:]?:][1-9][0-9]*(\.[0-9]+)?([[:space:]?:)]|$)'
check_pattern \
    'padding visual directo; usa space* o una métrica comp*' \
    '(^|[^[:alnum:]_])(leftPadding|rightPadding|topPadding|bottomPadding|padding)[[:space:]]*:[[:space:]]*[1-9][0-9]*(\.[0-9]+)?([[:space:]]|$)'
check_pattern \
    'opacidad visual directa; usa un token de estado/énfasis' \
    'opacity[[:space:]]*:[^;]*[[:space:]?:]0\.[0-9]+'

if ! python3 celestina-style/scripts/check-contrast-contract.py; then
    failures=1
fi

if ((failures)); then
    exit 1
fi

echo "Contrato visual QML: OK"
