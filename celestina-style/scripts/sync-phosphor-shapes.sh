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
# Phosphor Icons, MIT © Phosphor Icons — el aviso viaja en la cabecera generada
# y el texto completo en icons/LICENSE-phosphor.txt.

readonly phosphor_version="v2.0.8"
readonly phosphor_base="https://raw.githubusercontent.com/phosphor-icons/core/${phosphor_version}/assets/fill"

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
style_dir=$(cd -- "$script_dir/.." && pwd)
out_file="$style_dir/CelestinaIconShapes.qml"
work_dir=$(mktemp -d /tmp/celestina-phosphor-XXXXXX)
trap 'rm -rf -- "$work_dir" "$out_file.tmp"' EXIT
# The generated singleton is assembled here and moved into place only once it
# is complete. Redirecting the assembling block straight onto the source file
# would truncate it the instant the block opens, and the block downloads
# thirteen icons over the network before it has written anything worth keeping:
# any failure — and `set -e` plus the explicit `exit 1`s make several — would
# leave the module's own source as a half-written singleton that fails to parse
# and takes the whole import down with it until someone runs `git checkout`.
staged_file="$work_dir/CelestinaIconShapes.qml"

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
    # One page per language and per document kind, from the same family and the
    # same grid — so a folder of source files reads at a glance without a single
    # glyph borrowed from anywhere else. Phosphor has no page for Python, C,
    # Markdown or JSON; those keep `file-code` and are told apart by their tint.
    file-rs:file-rs
    file-js:file-js
    file-ts:file-ts
    file-jsx:file-jsx
    file-tsx:file-tsx
    file-vue:file-vue
    file-html:file-html
    file-css:file-css
    file-sql:file-sql
    file-svg:file-svg
    file-csv:file-csv
    file-doc:file-doc
    file-xls:file-xls
    file-ppt:file-ppt
    file-pdf:file-pdf
    file-jpg:file-jpg
    file-png:file-png
    file-lock:file-lock
    file-cloud:file-cloud
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
    echo "// Phosphor Icons — MIT © Phosphor Icons. Licencia: icons/LICENSE-phosphor.txt."
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
} > "$staged_file"

# Every requested shape must be present before anything replaces the source.
# The loop bails out on a failed download or an empty conversion, but a
# truncated body that still parses would be a silently smaller catalogue, and
# the count is what proves it is not.
generated=$(grep -cE '^        "[^"]+": \[$' "$staged_file" || true)
if [[ $generated -ne ${#shapes[@]} ]]; then
    echo "se generaron $generated formas de ${#shapes[@]}; no se toca el fuente" >&2
    exit 1
fi

# Same filesystem as the destination is not guaranteed for a /tmp work area, so
# copy into place beside the target and rename: the readers of the source tree
# see either the previous file or the complete new one, never a partial write.
install -m 644 -- "$staged_file" "$out_file.tmp"
mv -f -- "$out_file.tmp" "$out_file"

echo "CelestinaIconShapes.qml regenerado con ${#shapes[@]} formas (Phosphor ${phosphor_version})"
