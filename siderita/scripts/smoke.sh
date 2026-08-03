#!/bin/sh
set -u

# Humo de Siderita: la puerta rápida sin ventana.
#
#  1) Chequeo estático compartido del auto-binding `x: x`: al instanciar un componente,
#     una propiedad inyectada con el mismo nombre que el id sombreado se
#     resuelve a sí misma y queda undefined (la clase de bug del fix de
#     clics, 9e19b6d). Es legal para el motor y para qmllint, así que se caza
#     por patrón.
#  2) Arranque offscreen de 8 s con config de usar y tirar: el binario debe
#     seguir vivo (timeout devuelve 124) y el runtime QML no debe escupir
#     TypeError/ReferenceError. Ojo: esto solo caza errores de *arranque*;
#     los bindings que se evalúan al interactuar exigen sesión real.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=$root/target/release/siderita
scanner=$root/../scripts/architecture_scanners.py
if [ "${1:-}" = "--binary" ]; then
    shift
    bin=${1:?--binary necesita una ruta}
    shift
fi
if [ "$#" -gt 1 ]; then
    echo "uso: scripts/smoke.sh [--binary RUTA] [RUTA_A_ABRIR]" >&2
    exit 2
fi
requested_path=${1:-}

# El guard y este humo usan el mismo scanner y sus fixtures; así no divergen dos
# aproximaciones de gawk al distinguir bindings de literales de objeto JS.
if ! autos=$(python3 "$scanner" qml-auto-bindings "$root/qml"); then
    echo "smoke: el scanner de auto-bindings no pudo completar la inspección" >&2
    exit 1
fi
if [ -n "$autos" ]; then
    echo "smoke: auto-binding 'x: x' (la propiedad sombrea al id):" >&2
    echo "$autos" >&2
    exit 1
fi

if [ ! -x "$bin" ]; then
    echo "smoke: falta el binario indicado: $bin" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/config" "$scratch/data" "$scratch/cache" \
    "$scratch/state" "$scratch/run" "$scratch/home"
chmod 0700 "$scratch/run"
log=$scratch/salida.log
open_path=${requested_path:-$scratch/home}

XDG_CONFIG_HOME=$scratch/config \
XDG_DATA_HOME=$scratch/data \
XDG_CACHE_HOME=$scratch/cache \
XDG_STATE_HOME=$scratch/state \
XDG_RUNTIME_DIR=$scratch/run \
DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
QT_QPA_PLATFORM=offscreen \
QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    timeout 8 "$bin" "$open_path" >"$log" 2>&1
rc=$?
if [ "$rc" -ne 124 ]; then
    echo "smoke: el binario terminó solo (rc=$rc); últimas líneas:" >&2
    tail -20 "$log" >&2
    exit 1
fi

# Buscar sólo TypeError/ReferenceError dejaba pasar lo más grave: un fallo de
# *construcción*. Qt lo anuncia con otras palabras ("Cannot create delegate",
# "Cannot set properties on X as it is null", "Type X unavailable") y sigue
# corriendo, así que el binario seguía vivo 8 s y el humo daba OK mientras la
# vista principal no llegaba a existir. Un objeto que no se crea no es un aviso
# de estilo: es la pantalla entera ausente.
errores=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Binding loop detected' "$log" || true)
if [ -n "$errores" ]; then
    echo "smoke: errores QML en el arranque:" >&2
    echo "$errores" | sort | uniq -c | sort -rn >&2
    exit 1
fi

echo "smoke: OK — binario vivo 8 s, sin errores QML, sin auto-bindings"
