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
    echo "smoke: falta $bin — compila antes (cargo build --release)" >&2
    exit 1
fi

# El binario tiene que ser posterior a lo que dice probar. Sin esto, un build
# que falla deja el ejecutable anterior en su sitio y el humo pasa alegremente
# sobre código que ya no existe: pasó en esta misma sesión, y el fallo real
# —una app que no compilaba— quedó tapado por un OK.
newer=$(find "$root/src" "$root/qml" "$root/cpp" "$root/build.rs" "$root/Cargo.toml" \
    -type f -newer "$bin" -print -quit 2>/dev/null)
if [ -n "$newer" ]; then
    echo "smoke: $bin es anterior a $newer — recompila antes de fiarte de esto" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
log=$scratch/salida.log
# Un documento con terminadores CRLF: si el humo alguna vez los reescribe, el
# fallo aparece aquí y no en el archivo de alguien.
printf 'primera\r\nsegunda\r\n' > "$scratch/documento.txt"

XDG_CONFIG_HOME=$scratch QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
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

if ! cmp -s "$scratch/documento.txt" - <<'EOF'
primera
segunda
EOF
then
    :  # los bytes en disco llevan CRLF; el heredoc es LF, así que difieren
else
    echo "smoke: el documento perdió sus terminadores CRLF sólo con abrirlo" >&2
    exit 1
fi

echo "smoke: OK — binario vivo 8 s, sin errores QML, sin auto-bindings"
