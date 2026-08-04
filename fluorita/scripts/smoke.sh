#!/bin/sh
set -eu

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
if [ "${1:-}" = "--binary" ]; then
    shift
    bin=${1:?--binary necesita una ruta}
    shift
fi
if [ "$#" -ne 0 ]; then
    echo "uso: scripts/smoke.sh [--binary RUTA]" >&2
    exit 2
fi

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

[ -x "$bin" ] || fail "falta el binario indicado: $bin"
[ -f "$media" ] || fail "falta el fixture de media: $media"

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/config" "$scratch/data" "$scratch/cache" \
    "$scratch/state" "$scratch/run" "$scratch/Pictures" "$scratch/Videos" \
    "$scratch/Music"
chmod 0700 "$scratch/run"
printf 'XDG_PICTURES_DIR="%s"\nXDG_VIDEOS_DIR="%s"\nXDG_MUSIC_DIR="%s"\n' \
    "$scratch/Pictures" "$scratch/Videos" "$scratch/Music" \
    > "$scratch/config/user-dirs.dirs"

# Arranca el binario, espera, y devuelve por eco los hilos del proceso *real*
# (no los del envoltorio, que fue el error original de esta puerta).
threads_for() {
    argument_mode=$1
    log=$2
    argument=${3:-}
    if [ "$argument_mode" = "with-argument" ]; then
        XDG_CONFIG_HOME=$scratch/config \
        XDG_DATA_HOME=$scratch/data \
        XDG_CACHE_HOME=$scratch/cache \
        XDG_STATE_HOME=$scratch/state \
        XDG_RUNTIME_DIR=$scratch/run \
        DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
        QT_QPA_PLATFORM=offscreen \
        QT_ASSUME_STDERR_HAS_CONSOLE=1 \
            "$bin" "$argument" >"$log" 2>&1 &
    else
        XDG_CONFIG_HOME=$scratch/config \
        XDG_DATA_HOME=$scratch/data \
        XDG_CACHE_HOME=$scratch/cache \
        XDG_STATE_HOME=$scratch/state \
        XDG_RUNTIME_DIR=$scratch/run \
        DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
        QT_QPA_PLATFORM=offscreen \
        QT_ASSUME_STDERR_HAS_CONSOLE=1 \
            "$bin" >"$log" 2>&1 &
    fi
    pid=$!
    sleep 5
    if ! kill -0 "$pid" 2>/dev/null; then
        fail "el binario terminó solo ($argument_mode $argument)" "$log"
    fi
    cat /proc/"$pid"/task/*/comm 2>/dev/null | sort -u | tr '\n' ' '
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
}

qml_errors() {
    grep -E 'TypeError|ReferenceError|SyntaxError|failed to load component|failed to create component|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|already been used for type registration|Required property [A-Za-z_][A-Za-z0-9_]* was not initialized|Binding loop detected' "$1" || true
}

# ── 2) Un vídeo real: carga, vive y abre sesión ──────────────────────────────
playing=$(threads_for with-argument "$scratch/media.log" "$media")
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
idle=$(threads_for with-argument "$scratch/texto.log" "$scratch/nota.txt")
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
browsing=$(threads_for no-argument "$scratch/biblioteca.log")
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

# 4b) The sidebar's data reached disk. The library is navigated by configured
# root, and the handles the stored catalogue keys its records by only mean
# anything if the configuration itself was written down. A run that produced no
# store either failed to resolve any root or silently kept the set in memory,
# where the next launch would reissue every handle.
sources=$scratch/config/fluorita/sources.tsv
[ -f "$sources" ] || \
    fail "browsing the library stored no folder configuration" "$scratch/biblioteca.log"
head -n 1 "$sources" | grep -qx 'fluorita-sources 1' || \
    fail "the stored folder configuration has an unrecognised header"
if [ "$(grep -c '' "$sources")" -lt 2 ]; then
    fail "the stored folder configuration lists no root"
fi

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

still=$(threads_for with-argument "$scratch/imagen.log" "$scratch/foto.png")
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
