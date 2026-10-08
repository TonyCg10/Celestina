#!/bin/sh
set -u

# Humo de Grafita: la puerta rápida sin ventana.
#
#  1) Chequeo estático compartido del auto-binding `x: x`: al instanciar un
#     componente, una propiedad inyectada con el mismo nombre que el id
#     sombreado se resuelve a sí misma y queda undefined. Es legal para el motor
#     y para qmllint, así que se caza por patrón.
#  2) Arranque offscreen de 8 s abriendo un documento de usar y tirar: el
#     binario debe seguir vivo (timeout devuelve 124) y el runtime QML no debe
#     escupir errores. Buscar sólo TypeError/ReferenceError dejaría pasar lo más
#     grave —un objeto que no se construye—, así que también fallan
#     "Cannot create delegate" y compañía.
#
# Ojo: esto sólo caza errores de *arranque*. Teclado, foco, IME y accesibilidad
# exigen una sesión Wayland real.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=$root/target/release/grafita
scanner=$root/../scripts/architecture_scanners.py
if [ "${1:-}" = "--binary" ]; then
    shift
    bin=${1:?--binary necesita una ruta}
    shift
fi
if [ "$#" -ne 0 ]; then
    echo "uso: scripts/smoke.sh [--binary RUTA]" >&2
    exit 2
fi

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
    "$scratch/state" "$scratch/run"
chmod 0700 "$scratch/run"
log=$scratch/salida.log
# Un documento con terminadores CRLF: si el humo alguna vez los reescribe, el
# fallo aparece aquí y no en el archivo de alguien.
printf 'primera\r\nsegunda\r\n' > "$scratch/documento.txt"
cp "$scratch/documento.txt" "$scratch/documento-original.txt"

XDG_CONFIG_HOME=$scratch/config \
XDG_DATA_HOME=$scratch/data \
XDG_CACHE_HOME=$scratch/cache \
XDG_STATE_HOME=$scratch/state \
XDG_RUNTIME_DIR=$scratch/run \
DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
QT_QPA_PLATFORM=offscreen \
QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    timeout 8 "$bin" "$scratch/documento.txt" >"$log" 2>&1
rc=$?
if [ "$rc" -ne 124 ]; then
    echo "smoke: el binario terminó solo (rc=$rc); últimas líneas:" >&2
    tail -20 "$log" >&2
    exit 1
fi

errores=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Binding loop detected' "$log" || true)
if [ -n "$errores" ]; then
    echo "smoke: errores QML en el arranque:" >&2
    echo "$errores" | sort | uniq -c | sort -rn >&2
    exit 1
fi

if ! cmp -s "$scratch/documento.txt" "$scratch/documento-original.txt"; then
    echo "smoke: el documento perdió sus terminadores CRLF sólo con abrirlo" >&2
    exit 1
fi

# A KDL document, so the start-up also loads KSyntaxHighlighting's repository,
# picks a definition by the file's name and paints it, and places the bracket
# pair beside the caret (which starts before the first `{`). A missing or
# broken library shows up here as an exit or a QML error, not on the author's
# first open.
printf '// smoke\nnode {\n    child "a" key=1\n}\n' > "$scratch/config.kdl"
cp "$scratch/config.kdl" "$scratch/config-original.kdl"
log=$scratch/kdl.log
XDG_CONFIG_HOME=$scratch/config \
XDG_DATA_HOME=$scratch/data \
XDG_CACHE_HOME=$scratch/cache \
XDG_STATE_HOME=$scratch/state \
XDG_RUNTIME_DIR=$scratch/run \
DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
QT_QPA_PLATFORM=offscreen \
QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    timeout 8 "$bin" "$scratch/config.kdl" >"$log" 2>&1
rc=$?
if [ "$rc" -ne 124 ]; then
    echo "smoke: the binary exited on its own with a KDL file (rc=$rc); last lines:" >&2
    tail -20 "$log" >&2
    exit 1
fi
errores=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Binding loop detected' "$log" || true)
if [ -n "$errores" ]; then
    echo "smoke: QML errors at start-up with a KDL file:" >&2
    echo "$errores" | sort | uniq -c | sort -rn >&2
    exit 1
fi
if ! cmp -s "$scratch/config.kdl" "$scratch/config-original.kdl"; then
    echo "smoke: colouring a KDL file changed its bytes" >&2
    exit 1
fi

echo "smoke: OK — binario vivo 8 s (texto y KDL), sin errores QML, sin auto-bindings"
