#!/usr/bin/env bash

set -uo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
scanner="$script_dir/architecture_scanners.py"
fixtures="$script_dir/fixtures/architecture"
failures=0
fixture_tmp=''

fail() {
    printf 'architecture fixtures: ERROR: %s\n' "$*" >&2
    failures=1
}

if ! fixture_tmp=$(mktemp -d); then
    echo "architecture fixtures: ERROR: no se pudo crear el directorio temporal" >&2
    exit 1
fi
trap 'rm -R -- "$fixture_tmp"' EXIT

if ! mkdir -p "$fixture_tmp/qml" "$fixture_tmp/absolute"; then
    echo "architecture fixtures: ERROR: no se pudo preparar el fixture temporal" >&2
    exit 1
fi

relative_shared=$(realpath --relative-to="$fixture_tmp/qml" \
    "$fixtures/style/SharedButton.qml")
relative_violations=$(realpath --relative-to="$fixture_tmp/qml" \
    "$fixtures/qml/StyleViolations.qml")
relative_surface=$(realpath --relative-to="$fixture_tmp/qml" \
    "$fixtures/style/SharedSurface.qml")
if ! ln -s "$relative_shared" "$fixture_tmp/qml/SharedButton.qml" \
    || ! ln -s "$relative_shared" "$fixture_tmp/qml/RenamedButton.qml" \
    || ! ln -s "$relative_violations" "$fixture_tmp/qml/StyleViolationsLink.qml" \
    || ! ln -s "$relative_surface" "$fixture_tmp/qml/RenamedSurface.qml" \
    || ! ln -s "$fixtures/style/SharedButton.qml" \
        "$fixture_tmp/absolute/SharedButton.qml"; then
    echo "architecture fixtures: ERROR: no se pudieron crear los symlinks de prueba" >&2
    exit 1
fi

output=''
if ! output=$(python3 "$scanner" qml-auto-bindings "$fixtures/qml/Clean.qml"); then
    fail "el scanner de auto-bindings fallo sobre el fixture limpio"
elif [[ -n $output ]]; then
    fail "el fixture limpio produjo un falso positivo: $output"
fi

if ! output=$(python3 "$scanner" qml-auto-bindings "$fixtures/qml/AutoBinding.qml"); then
    fail "el scanner de auto-bindings fallo sobre el fixture positivo"
elif [[ $output != *"x: x;"* ]]; then
    fail "x: x; no fue detectado"
fi

if ! output=$(python3 "$scanner" local-controls "$fixtures/qml/RawControl.qml"); then
    fail "el scanner de controles fallo sobre el fixture positivo"
elif [[ $output != *$'RawControl.qml\tButton'* ]]; then
    fail "el control Qt local no fue detectado"
fi

if ! output=$(python3 "$scanner" local-controls \
    "$fixtures/qml/InlineControls.qml"); then
    fail "el scanner de controles fallo sobre declaraciones inline/object-valued"
elif [[ $(grep -c $'InlineControls.qml\tButton' <<< "$output") -ne 4 ]]; then
    fail "declaraciones inline u object-valued permitieron evadir controles Qt"
fi

if ! output=$(python3 "$scanner" local-controls \
    "$fixture_tmp/qml/RenamedButton.qml"); then
    fail "el scanner de controles fallo al seguir un symlink"
elif [[ $output != *$'RenamedButton.qml\tButton'* ]]; then
    fail "un control Qt detras de un symlink renombrado no fue detectado"
fi

if ! output=$(python3 "$scanner" local-controls \
    --style-root "$fixtures/style" "$fixture_tmp/qml/SharedButton.qml"); then
    fail "el scanner de controles rechazo un symlink canonico de estilo"
elif [[ -n $output ]]; then
    fail "un control compartido canonico conto como reconstruccion local: $output"
fi

if ! output=$(python3 "$scanner" local-controls "$fixtures/qml/Clean.qml"); then
    fail "el scanner de controles fallo sobre el fixture limpio"
elif [[ -n $output ]]; then
    fail "un control dentro de comentario produjo un falso positivo"
fi

if ! output=$(python3 "$scanner" qml-style-contract \
    "$fixtures/style/CelestinaTheme.qml" "$fixtures/qml/StyleClean.qml"); then
    fail "el scanner visual fallo sobre el fixture limpio"
elif [[ -n $output ]]; then
    fail "el fixture visual limpio produjo un falso positivo: $output"
fi

if ! output=$(python3 "$scanner" qml-style-contract \
    "$fixtures/style/CelestinaTheme.qml" \
    "$fixture_tmp/qml/StyleViolationsLink.qml"); then
    fail "el scanner visual fallo al seguir un symlink"
elif [[ $output != *"color nominal"* || $output != *"radio numerico"* \
        || $output != *"transformacion de color"* ]]; then
    fail "un symlink permitio ocultar valores visuales directos"
fi

if ! output=$(python3 "$scanner" qml-style-contract \
    "$fixtures/style/CelestinaTheme.qml" "$fixtures/qml/StyleViolations.qml"); then
    fail "el scanner visual fallo sobre el fixture positivo"
elif [[ $output != *"color nominal"* || $output != *"radio numerico"* \
        || $output != *"transformacion de color"* || $output != *'"blue"'* \
        || $output != *'"green"'* ]]; then
    fail "los valores visuales multilinea/property color no fueron detectados"
fi

if ! output=$(python3 "$scanner" style-copies \
    "$fixtures/style" "$fixtures/style-copy"); then
    fail "la comparacion de copias de estilo fallo"
elif [[ $output != *"RenamedSurface.qml: copia estructural"* ]]; then
    fail "una copia renombrada del estilo no fue detectada"
fi

if ! output=$(python3 "$scanner" style-copies \
    "$fixtures/style" "$fixture_tmp/qml/RenamedSurface.qml"); then
    fail "la comparacion de estilo fallo al seguir un symlink renombrado"
elif [[ $output != *"RenamedSurface.qml: copia estructural"* ]]; then
    fail "un symlink renombrado oculto una copia estructural del estilo"
fi

if ! python3 "$scanner" shared-style-links \
    "$fixtures/style" "$fixture_tmp/qml/SharedButton.qml"; then
    fail "un symlink relativo al componente canonico fue rechazado"
fi

if output=$(python3 "$scanner" shared-style-links \
    "$fixtures/style" "$fixture_tmp/qml/RenamedButton.qml" 2>&1); then
    fail "un symlink renombrado evadio la restriccion de destino"
elif [[ $output != *"componente homonimo"* ]]; then
    fail "el symlink renombrado fallo sin diagnostico de destino canonico"
fi

if output=$(python3 "$scanner" shared-style-links \
    "$fixtures/style" "$fixture_tmp/absolute/SharedButton.qml" 2>&1); then
    fail "un symlink absoluto evadio la politica de enlaces relativos"
elif [[ $output != *"debe ser relativo"* ]]; then
    fail "el symlink absoluto fallo sin diagnostico de enlace relativo"
fi

if ! python3 "$scanner" cmake-qml-registration \
    "$fixtures/cmake-valid/CMakeLists.txt" \
    "$fixtures/cmake-valid/qml" \
    celestina; then
    fail "el registro CMake valido fue rechazado"
fi

if output=$(python3 "$scanner" cmake-qml-registration \
    "$fixtures/cmake-invalid/CMakeLists.txt" \
    "$fixtures/cmake-invalid/qml" \
    celestina 2>&1); then
    fail "un QML sin registrar evadio la paridad CMake"
elif [[ $output != *"Unregistered.qml"* || $output != *"Missing.qml"* ]]; then
    fail "la paridad CMake no informo ambos lados del desajuste"
fi

if python3 "$scanner" qml-auto-bindings "$fixtures/does-not-exist" \
    >/dev/null 2>&1; then
    fail "una entrada ausente no hizo fallar el scanner"
fi

if python3 "$scanner" dependency-metadata </dev/null >/dev/null 2>&1; then
    fail "metadata vacia no hizo fallar el scanner de dependencias"
fi

metadata='{"packages":[{"name":"core","dependencies":[{"name":"qt6-types","rename":"ui"}]}]}'
if ! output=$(python3 "$scanner" dependency-metadata <<< "$metadata"); then
    fail "el scanner rechazo metadata valida"
elif [[ $output != "core: qt6-types (alias ui)" ]]; then
    fail "una dependencia UI renombrada no fue detectada"
fi

metadata='{"packages":[{"name":"core","dependencies":[{"name":"gtk4"},{"name":"iced"},{"name":"slint"},{"name":"smithay-client-toolkit"}]}]}'
if ! output=$(python3 "$scanner" dependency-metadata <<< "$metadata"); then
    fail "el scanner rechazo metadata UI valida"
elif [[ $output != *"gtk4"* || $output != *"iced"* || $output != *"slint"* \
        || $output != *"smithay-client-toolkit"* ]]; then
    fail "una familia UI/compositor no fue detectada"
fi

# Cobertura de los guards: ambos enumeran proyectos por nombre, así que un
# proyecto nuevo con QML puede quedar fuera de una lista y "pasar" sin ser
# inspeccionado nunca. Esto no prueba un scanner: prueba que los scanners
# reciben todo lo que existe.
repo_root=$(cd -- "$script_dir/.." && pwd)
architecture_guard="$script_dir/check-architecture-contract.sh"
style_guard="$repo_root/celestina-style/scripts/check-style-contract.sh"

for guard in "$architecture_guard" "$style_guard"; do
    [[ -f $guard ]] || fail "falta el guard $guard"
done

for candidate in "$repo_root"/*/; do
    project=$(basename -- "$candidate")
    [[ $project == celestina-style ]] && continue
    [[ -d $candidate/qml ]] || continue

    # Por línea, no por archivo: cada invocación de scanner enumera sus
    # entradas en una sola línea, y estar en tres de las cuatro listas deja un
    # scanner ciego. `siderita/qml` es el miembro que aparece en todas, así que
    # sirve de plantilla de lo que cada lista debe contener.
    for guard in "$architecture_guard" "$style_guard"; do
        while IFS= read -r line; do
            if [[ $line != *"$project/qml"* ]]; then
                fail "$(basename -- "$guard"): una lista de entradas omite $project/qml -> $line"
            fi
        done < <(grep -F 'siderita/qml' "$guard")
    done

    # Los proyectos con build.rs además pasan por el bucle de registro QML y
    # por el de symlinks de estilo, que enumeran por nombre desnudo.
    if [[ -f $candidate/build.rs ]]; then
        while IFS= read -r line; do
            if [[ $line != *" $project"* ]]; then
                fail "guard de arquitectura: un bucle 'for app in' omite $project -> $line"
            fi
        done < <(grep -E 'for app in .*siderita' "$architecture_guard")
    fi
done

if ((failures)); then
    exit 1
fi

echo "Fixtures de arquitectura: OK"
