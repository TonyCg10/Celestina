#!/bin/sh
set -u

# Humo de Siderita: la puerta rápida sin ventana.
#
#  1) Chequeo estático del auto-binding `x: x`: al instanciar un componente,
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

# gawk y no grep: hay que comparar los dos nombres capturados y saltarse los
# literales de objeto JS (`append({clave: clave})`), que viven dentro de
# paréntesis — un binding QML real siempre está a profundidad 0.
autos=$(find "$root/qml" -name '*.qml' -exec gawk '
    FNR == 1 { depth = 0 }
    { linea = $0; sub(/\/\/.*/, "", linea) }
    depth == 0 \
      && match(linea, /^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*):[[:space:]]*([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*$/, m) \
      && m[1] == m[2] && m[1] != "id" {
        printf "%s:%d: %s\n", FILENAME, FNR, $0
    }
    { depth += gsub(/\(/, "(", linea) - gsub(/\)/, ")", linea)
      if (depth < 0) depth = 0 }
' {} + || true)
if [ -n "$autos" ]; then
    echo "smoke: auto-binding 'x: x' (la propiedad sombrea al id):" >&2
    echo "$autos" >&2
    exit 1
fi

if [ ! -x "$bin" ]; then
    echo "smoke: falta $bin — compila antes (cargo build --release)" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
log=$scratch/salida.log

XDG_CONFIG_HOME=$scratch QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    timeout 8 "$bin" "${1:-$HOME}" >"$log" 2>&1
rc=$?
if [ "$rc" -ne 124 ]; then
    echo "smoke: el binario terminó solo (rc=$rc); últimas líneas:" >&2
    tail -20 "$log" >&2
    exit 1
fi

errores=$(grep -E 'TypeError|ReferenceError' "$log" || true)
if [ -n "$errores" ]; then
    echo "smoke: errores QML en el arranque:" >&2
    echo "$errores" | sort | uniq -c | sort -rn >&2
    exit 1
fi

echo "smoke: OK — binario vivo 8 s, sin errores QML, sin auto-bindings"
