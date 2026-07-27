#!/bin/sh

set -eu

# output-chooser.sh — el selector de "¿qué pantalla comparto?" de la sesión.
#
# `xdg-desktop-portal-wlr` no trae diálogo propio: ejecuta el comando que se le
# indique en ~/.config/xdg-desktop-portal-wlr/config y se queda con el nombre de
# salida que ese comando imprima por stdout. Este envoltorio lanza el selector
# (`celestina --pick-output`, la ventana QML de qml/OutputChooser.qml vestida
# con CelestinaStyle); es el propio shell quien escribe el nombre elegido por
# stdout (main.cpp::runOutputChooser), que es lo único que el backend mira.
#
# Salidas: 0 con el nombre en stdout si se elige; 1 sin nada si se cancela.

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)

qml_file=$project_root/qml/OutputChooser.qml

# CelestinaStyle se importa por directorio: QML_IMPORT_PATH debe apuntar al
# *padre* de una carpeta llamada CelestinaStyle, así que se prepara un enlace en
# el directorio de runtime del usuario en lugar de exigir el módulo instalado.
# Cuando el shell compile CelestinaStyle dentro de su propio módulo QML (el
# patrón de las apps), esto sobra.
import_root=${XDG_RUNTIME_DIR:-/tmp}/celestina-style-import
mkdir -p "$import_root"
if [ ! -e "$import_root/CelestinaStyle" ]; then
    ln -sfn "$suite_root/celestina-style" "$import_root/CelestinaStyle"
fi

# El shell hospeda el selector (`celestina --pick-output`): así la ventana tiene
# un app_id estable — `celestina`, lo que una regla de niri necesita para
# flotarla en vez de tilearla — y responde por stdout de verdad.
binario=$project_root/build/celestina

if [ -x "$binario" ]; then
    QML_IMPORT_PATH=$import_root exec "$binario" --pick-output
fi

# Sin compilar: el runtime genérico sirve, pero la ventana se identifica como
# `org.qt-project.qml` y niri no puede distinguirla de cualquier otro QML.
if ! command -v qml6 >/dev/null 2>&1; then
    echo "output-chooser: ni $binario ni qml6" >&2
    exit 1
fi
echo "output-chooser: usando qml6 (compila el shell para el app_id correcto)" >&2
QML_IMPORT_PATH=$import_root exec qml6 "$qml_file"
