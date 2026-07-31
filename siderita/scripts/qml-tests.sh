#!/bin/sh
set -eu

# Pruebas de interacción QML de Siderita (`tests/qml`): pulsan, mueven y barren
# de verdad sobre los componentes reales con `qmltestrunner`.
#
# El módulo `org.celestina.siderita` normalmente lo publica el binario, así que
# fuera de la app sus tipos no existen. Aquí se arma uno equivalente y sin
# plugin: un `qmldir` generado a partir de los propios .qml del árbol (por eso
# lo que se prueba es el fuente, no una copia). Los tipos registrados desde Rust
# no entran — una prueba que los necesite pertenece al binario, no aquí.
#
# Cubre lo que un build o un humo no pueden probar: que el puntero de una
# superficie flotante no llegue al contenido que tapa. No sustituye a la sesión
# real para apariencia, cristal ni accesibilidad.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
src=$root/qml

runner=${QMLTESTRUNNER:-}
if [ -z "$runner" ]; then
    # `qmltestrunner` a secas puede ser el de Qt5 (Arch lo instala en /usr/bin),
    # y ése no entiende este árbol. Se pregunta primero por el de Qt6.
    for candidate in \
        "$(qtpaths6 --query QT_INSTALL_BINS 2>/dev/null || true)/qmltestrunner" \
        "$(qmake6 -query QT_INSTALL_BINS 2>/dev/null || true)/qmltestrunner" \
        /usr/lib/qt6/bin/qmltestrunner
    do
        if [ -x "$candidate" ]; then
            runner=$candidate
            break
        fi
    done
fi
if [ -z "$runner" ]; then
    echo "qml-tests: falta el qmltestrunner de Qt6 (fija QMLTESTRUNNER)" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
module=$scratch/imports/org/celestina/siderita
mkdir -p "$module"
ln -s "$src" "$module/qml"

{
    echo "module org.celestina.siderita"
    cd "$src"
    # Los .qml compartidos llegan por symlink desde celestina-style, así que hay
    # que aceptar enlaces además de ficheros.
    find . \( -type f -o -type l \) -name '*.qml' | sed 's|^\./||' | sort \
    | while read -r rel; do
        name=$(basename "$rel" .qml)
        if head -5 "$rel" | grep -q '^pragma Singleton'; then
            echo "singleton $name 1.0 qml/$rel"
        else
            echo "$name 1.0 qml/$rel"
        fi
    done
} > "$module/qmldir"

QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    "$runner" -input "$root/tests/qml" -import "$scratch/imports" "$@"
