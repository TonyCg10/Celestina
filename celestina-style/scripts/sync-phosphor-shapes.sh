#!/usr/bin/env bash

set -euo pipefail

# Sincroniza el catálogo de *formas* de contenido: los iconos rellenos que la
# suite pinta con su lavado de color, frente a los glifos de trazo de Lucide que
# visten los controles.
#
# Por qué formas y no SVG teñidos: un relleno plano no admite gradiente, y
# meterlo con una máscara re-muestrea el dibujo (se probó: sale engordado y con
# dientes). `svgtoqml` convierte cada SVG en trazos vectoriales, y de ahí este
# script extrae sólo los datos de camino. Un `ShapePath` los rellena con el
# gradiente nativo, sin máscara y sin pérdida.
#
# La salida es `CelestinaIconShapes.qml`: una tabla nombre → caminos, generada,
# nunca editada a mano. Se regenera con este script y se revisa como cualquier
# otro asset vendorizado.
#
# Phosphor Icons, MIT © Phosphor Icons — el aviso viaja en la cabecera generada.

readonly phosphor_version="v2.0.8"
readonly phosphor_base="https://raw.githubusercontent.com/phosphor-icons/core/${phosphor_version}/assets/fill"

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
style_dir=$(cd -- "$script_dir/.." && pwd)
out_file="$style_dir/CelestinaIconShapes.qml"
work_dir=$(mktemp -d /tmp/celestina-phosphor-XXXXXX)
trap 'rm -rf -- "$work_dir"' EXIT

svgtoqml=${SVGTOQML:-}
if [[ -z "$svgtoqml" ]]; then
    for candidate in \
        "$(qtpaths6 --query QT_INSTALL_LIBEXECS 2>/dev/null || true)/svgtoqml" \
        "$(qtpaths6 --query QT_INSTALL_BINS 2>/dev/null || true)/svgtoqml" \
        /usr/lib/qt6/bin/svgtoqml
    do
        [[ -x "$candidate" ]] && { svgtoqml=$candidate; break; }
    done
fi
[[ -n "$svgtoqml" ]] || { echo "falta svgtoqml de Qt 6 (fija SVGTOQML)" >&2; exit 1; }

# nombre-de-la-suite:glifo-de-phosphor. El nombre de la izquierda es el que ya
# usan las apps y el que persiste Siderita en su configuración.
#
# La lista es corta a propósito: sólo *tipos de contenido*. Un icono de control
# —buscar, ordenar, ajustes— sigue siendo un trazo de Lucide, y meterlo aquí
# sería empezar a tener dos catálogos que dicen lo mismo.
shapes=(
    file:file
    file-text:file-text
    file-image:image
    file-music:file-audio
    file-video-camera:file-video
    file-code:file-code
    file-braces:brackets-curly
    file-archive:file-zip
    hard-drive:hard-drives
    phone:device-mobile
    monitor:monitor
    user-trash:trash
    symlink:link
)

{
    echo "pragma Singleton"
    echo
    echo "import QtQuick"
    echo
    echo "// ─── CelestinaIconShapes ─────────────────────────────────────────────────────"
    echo "// GENERADO — no editar a mano. Se regenera con"
    echo "// \`scripts/sync-phosphor-shapes.sh\`, fijado a Phosphor ${phosphor_version}."
    echo "//"
    echo "// Los caminos vectoriales de los iconos de contenido, en la rejilla de 256 que"
    echo "// trae Phosphor. \`CelestinaFileIcon\` los pinta con el lavado de color del tema;"
    echo "// aquí no hay ni un color, sólo geometría."
    echo "//"
    echo "// Phosphor Icons — MIT © Phosphor Icons. La licencia viaja en icons/LICENSE."
    echo "// ──────────────────────────────────────────────────────────────────────────────"
    echo "QtObject {"
    echo "    // Todos los glifos comparten la rejilla, así que el escalado es uno."
    echo "    readonly property int viewBox: 256"
    echo
    echo "    readonly property var paths: ({"

    first=1
    for entry in "${shapes[@]}"; do
        local_name=${entry%%:*}
        upstream=${entry#*:}
        svg="$work_dir/$upstream.svg"
        if ! curl -sSfL --max-time 20 -o "$svg" "$phosphor_base/$upstream-fill.svg"; then
            echo "no se pudo descargar $upstream" >&2
            exit 1
        fi
        "$svgtoqml" -c "$svg" "$work_dir/$upstream.qml"
        # De la conversión sólo interesan los datos de camino: el color lo pone
        # el tema, y la geometría del `Item` la pone el consumidor.
        mapfile -t paths < <(grep -oE 'PathSvg \{ path: "[^"]*"' "$work_dir/$upstream.qml" \
                             | sed -E 's/PathSvg \{ path: "//; s/"$//')
        [[ ${#paths[@]} -gt 0 ]] || { echo "$upstream no produjo caminos" >&2; exit 1; }
        [[ $first -eq 1 ]] || echo ","
        first=0
        printf '        "%s": [\n' "$local_name"
        for index in "${!paths[@]}"; do
            printf '            "%s"' "${paths[$index]}"
            [[ $index -lt $((${#paths[@]} - 1)) ]] && printf ','
            printf '\n'
        done
        printf '        ]'
    done
    echo
    echo "    })"
    echo
    echo "    function has(name) {"
    echo "        return paths[name] !== undefined"
    echo "    }"
    echo
    echo "    // Los caminos de un nombre, o una lista vacía: un consumidor pregunta"
    echo "    // por cualquier nombre del catálogo semántico y decide si dibuja la"
    echo "    // forma o se queda con el glifo de trazo."
    echo "    function pathsFor(name) {"
    echo "        return paths[name] !== undefined ? paths[name] : []"
    echo "    }"
    echo "}"
} > "$out_file"

echo "CelestinaIconShapes.qml regenerado con ${#shapes[@]} formas (Phosphor ${phosphor_version})"
