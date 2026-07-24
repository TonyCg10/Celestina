#!/bin/sh

set -eu

# output-chooser.sh — el selector de "¿qué pantalla comparto?" de la sesión.
#
# `xdg-desktop-portal-wlr` no trae diálogo propio: ejecuta el comando que se le
# indique en ~/.config/xdg-desktop-portal-wlr/config y se queda con el nombre de
# salida que ese comando imprima por stdout. Este envoltorio lanza la ventana
# QML (qml/OutputChooser.qml), que viste el lenguaje de CelestinaStyle, y
# traduce su respuesta.
#
# La traducción existe porque QML sólo escribe por el canal de diagnóstico
# (stderr): la ventana imprime `CELESTINA-OUTPUT:<nombre>` y aquí se extrae y se
# manda a stdout, que es lo único que el backend mira.
#
# Salidas: 0 con el nombre en stdout si se elige; 1 sin nada si se cancela.

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)

qml_file=$project_root/qml/OutputChooser.qml

# CelestinaStyle se importa por directorio: QML_IMPORT_PATH debe apuntar al
# *padre* de una carpeta llamada CelestinaStyle, así que se prepara un enlace en
# el directorio de runtime del usuario en lugar de exigir el módulo instalado.
# Cuando celestina-style tenga su release instalable (su CP0), esto sobra.
import_root=${XDG_RUNTIME_DIR:-/tmp}/celestina-style-import
mkdir -p "$import_root"
if [ ! -e "$import_root/CelestinaStyle" ]; then
    ln -sfn "$suite_root/celestina-style" "$import_root/CelestinaStyle"
fi

if ! command -v qml6 >/dev/null 2>&1; then
    echo "output-chooser: falta qml6 (qt6-declarative)" >&2
    exit 1
fi
if [ ! -f "$qml_file" ]; then
    echo "output-chooser: no existe $qml_file" >&2
    exit 1
fi

salida=$(QT_ASSUME_STDERR_HAS_CONSOLE=1 QML_IMPORT_PATH=$import_root \
    qml6 "$qml_file" 2>&1 >/dev/null | sed -n 's/.*CELESTINA-OUTPUT:\(.*\)$/\1/p' | head -1)

if [ -z "$salida" ]; then
    exit 1
fi
printf '%s\n' "$salida"
