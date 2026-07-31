#!/bin/sh
set -u

# Humo de Fluorita: la puerta rápida sin ventana.
#
#  1) Chequeo estático compartido del auto-binding `x: x`: al instanciar un
#     componente, una propiedad inyectada con el mismo nombre que el id
#     sombreado se resuelve a sí misma y queda undefined. Es legal para el motor
#     y para qmllint, así que se caza por patrón.
#  2) Arranque offscreen con un archivo de media real: el binario sigue vivo, el
#     QML *carga* y el motor abre sesión de verdad.
#  3) Arranque offscreen con un archivo que no es media: no debe existir ningún
#     hilo del backend. Navegar no arranca decodificadores.
#  4) Arranque sin argumento: la biblioteca explora en el worker y tampoco
#     arranca el motor — navegar es leer nombres y caché, no decodificar.
#  5) Arranque offscreen con una imagen: tampoco. Mirar una foto la decodifica
#     el toolkit; que aquí aparezca un hilo del motor significa que la promesa
#     de peso perezoso se rompió.
#
# Dos aprendizajes de esta puerta, que explican por qué mira lo que mira:
#   · medir `$!` de `timeout` inspeccionaba al proceso equivocado, así que la
#     comprobación del motor no miraba nada;
#   · buscar sólo TypeError/ReferenceError dejó pasar un QML que no cargaba en
#     absoluto ("failed to load component"), con la ventana nunca creada.
#
# Ojo: esto sólo caza errores de *arranque*. Imagen, pacing, teclado, foco y
# accesibilidad exigen una sesión Wayland real.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=$root/target/release/fluorita
scanner=$root/../scripts/architecture_scanners.py
media=$root/../celestina-rs/crates/fluorita-engine/tests/fixtures/clip.mp4

fail() {
    echo "smoke: $1" >&2
    [ -n "${2:-}" ] && tail -20 "$2" >&2
    exit 1
}

if ! autos=$(python3 "$scanner" qml-auto-bindings "$root/qml"); then
    fail "el scanner de auto-bindings no pudo completar la inspección"
fi
if [ -n "$autos" ]; then
    echo "smoke: auto-binding 'x: x' (la propiedad sombrea al id):" >&2
    echo "$autos" >&2
    exit 1
fi

[ -x "$bin" ] || fail "falta $bin — compila antes (cargo build --release)"
[ -f "$media" ] || fail "falta el fixture de media: $media"

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

# Arranca el binario, espera, y devuelve por eco los hilos del proceso *real*
# (no los del envoltorio, que fue el error original de esta puerta).
threads_for() {
    argument=$1
    log=$2
    XDG_CONFIG_HOME=$scratch QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        "$bin" "$argument" >"$log" 2>&1 &
    pid=$!
    sleep 5
    if ! kill -0 "$pid" 2>/dev/null; then
        fail "el binario terminó solo con $argument" "$log"
    fi
    cat /proc/"$pid"/task/*/comm 2>/dev/null | sort -u | tr '\n' ' '
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
}

qml_errors() {
    grep -E 'TypeError|ReferenceError|SyntaxError|failed to load component|failed to create component|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|already been used for type registration|Required property [A-Za-z_][A-Za-z0-9_]* was not initialized|Binding loop detected' "$1" || true
}

# ── 2) Un vídeo real: carga, vive y abre sesión ──────────────────────────────
playing=$(threads_for "$media" "$scratch/media.log")
errores=$(qml_errors "$scratch/media.log")
if [ -n "$errores" ]; then
    echo "smoke: errores QML al abrir media:" >&2
    echo "$errores" | sort | uniq -c | sort -rn >&2
    exit 1
fi
case "$playing" in
    *core*) ;;
    *) fail "abrir un vídeo no arrancó el backend (hilos: $playing)" "$scratch/media.log" ;;
esac
case "$playing" in
    *fluorita-player*) ;;
    *) fail "no hay hilo de reproducción: la sesión corre en el hilo GUI" "$scratch/media.log" ;;
esac

# ── 3) Algo que no es media: ni un hilo del backend ──────────────────────────
printf 'no soy media\n' > "$scratch/nota.txt"
idle=$(threads_for "$scratch/nota.txt" "$scratch/texto.log")
errores=$(qml_errors "$scratch/texto.log")
if [ -n "$errores" ]; then
    echo "smoke: errores QML con un archivo no reconocido:" >&2
    echo "$errores" | sort | uniq -c | sort -rn >&2
    exit 1
fi
case "$idle" in
    *core*|*fluorita-player*)
        fail "un archivo que no es media arrancó el motor (hilos: $idle)" "$scratch/texto.log" ;;
esac

# ── 4) Sin argumento: la biblioteca explora sin decodificar ─────────────────
browsing=$(threads_for "" "$scratch/biblioteca.log")
errores=$(qml_errors "$scratch/biblioteca.log")
if [ -n "$errores" ]; then
    echo "smoke: errores QML en la biblioteca:" >&2
    echo "$errores" | sort | uniq -c | sort -rn >&2
    exit 1
fi
case "$browsing" in
    *core*|*fluorita-player*)
        fail "explorar la biblioteca arrancó el motor (hilos: $browsing)" "$scratch/biblioteca.log" ;;
esac

# ── 5) Una imagen: la decodifica el toolkit, no el motor ────────────────────
python3 - "$scratch/foto.png" <<'PNG'
import struct, sys, zlib

# Un PNG 8x8 en escala de grises, escrito a mano para no depender de ffmpeg.
width = height = 8


def chunk(kind, payload):
    body = kind + payload
    return struct.pack(">I", len(payload)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


raw = b"".join(b"\x00" + bytes(range(width)) for _ in range(height))
png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 0, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(raw, 9))
    + chunk(b"IEND", b"")
)
open(sys.argv[1], "wb").write(png)
PNG

still=$(threads_for "$scratch/foto.png" "$scratch/imagen.log")
errores=$(qml_errors "$scratch/imagen.log")
if [ -n "$errores" ]; then
    echo "smoke: errores QML con una imagen:" >&2
    echo "$errores" | sort | uniq -c | sort -rn >&2
    exit 1
fi
case "$still" in
    *core*|*fluorita-player*)
        fail "una imagen arrancó el motor multimedia (hilos: $still)" "$scratch/imagen.log" ;;
esac

echo "smoke: OK — QML carga, un vídeo abre sesión fuera del hilo GUI, y ni la biblioteca ni una imagen ni un archivo desconocido arrancan el motor"
