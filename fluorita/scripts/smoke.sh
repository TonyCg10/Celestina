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
#  6) Automatic thumbnails (the author's decision of 2026-10-07): a library
#     holding only a photo gets its thumbnail from the shared provider with no
#     engine thread; once a clip is added, the background pass gives it a
#     poster without being asked.
#
# HOME points into the scratch directory too: the library seeds its roots from
# $HOME/Pictures and friends, and a smoke that walked the real ones would
# depend on — and, for posters, write about — whatever the machine holds.
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

# 1b) No layer may follow an Image's loading state. On a scale change (moving
# the window to a monitor with another scale) Qt reloads every sized picture
# while it walks the item tree; a `layer.enabled` that follows `status` (or a
# `visible` built on it) switches off right there and deletes the effect it
# placed beside the item, which the walk then visits: a segfault in
# QQuickWindow::physicalDpiChanged. Offscreen never changes scale, so this is
# caught by pattern, like the auto-bindings above.
layers=$(grep -rnE 'layer\.enabled:.*([.]visible|[.]status|\bvisible\b|\bstatus\b)' \
    "$root/qml" || true)
if [ -n "$layers" ]; then
    echo "smoke: layer.enabled follows visibility or loading state:" >&2
    echo "$layers" >&2
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
        HOME=$scratch \
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
        HOME=$scratch \
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

# ── 5b) `--edit`: the floating editor alone ─────────────────────────────────
# `fluorita --edit PATH` (PRV-1) starts with one edit window and no library:
# the process stays up, the QML loads, no engine thread starts (an edit is the
# toolkit's) and the library is not scanned, so no folder configuration is
# written. A path that is not a regular file is refused before any window.
isolated() {
    HOME=$scratch \
    XDG_CONFIG_HOME=$scratch/config \
    XDG_DATA_HOME=$scratch/data \
    XDG_CACHE_HOME=$scratch/cache \
    XDG_STATE_HOME=$scratch/state \
    XDG_RUNTIME_DIR=$scratch/run \
    DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
    QT_QPA_PLATFORM=offscreen \
    QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        "$bin" "$@"
}

# The same launch in the background, as the process `$!` names: `isolated &`
# forks a shell that runs Fluorita as its child, so `$!` was that shell, the
# threads read were the shell's, and the kill left Fluorita running. Here the
# forked shell becomes Fluorita.
isolated_background() {
    HOME=$scratch \
    XDG_CONFIG_HOME=$scratch/config \
    XDG_DATA_HOME=$scratch/data \
    XDG_CACHE_HOME=$scratch/cache \
    XDG_STATE_HOME=$scratch/state \
    XDG_RUNTIME_DIR=$scratch/run \
    DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
    QT_QPA_PLATFORM=offscreen \
    QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        exec "$bin" "$@"
}

mv "$sources" "$scratch/sources.kept"
isolated_background --edit "$scratch/foto.png" >"$scratch/edit.log" 2>&1 &
pid=$!
sleep 5
kill -0 "$pid" 2>/dev/null || fail "an edit launch ended by itself" "$scratch/edit.log"
editing=$(cat /proc/"$pid"/task/*/comm 2>/dev/null | sort -u | tr '\n' ' ')
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true
errores=$(qml_errors "$scratch/edit.log")
[ -z "$errores" ] || fail "QML errors in an edit launch: $errores" "$scratch/edit.log"
case "$editing" in
    *core*|*fluorita-player*)
        fail "an edit launch started the media engine (threads: $editing)" "$scratch/edit.log" ;;
esac
[ ! -e "$sources" ] || fail "an edit launch scanned the library" "$scratch/edit.log"
mv "$scratch/sources.kept" "$sources"
if isolated --edit "$scratch" >"$scratch/edit-folder.log" 2>&1; then
    fail "--edit accepted a folder" "$scratch/edit-folder.log"
fi
grep -q 'cannot edit' "$scratch/edit-folder.log" || \
    fail "--edit refused a folder without saying why" "$scratch/edit-folder.log"

# ── 5c) `--edit` on a film: the trim ────────────────────────────────────────
# A video opens in the same window for the trim (PRV-1, FLU-P1-B): the QML
# loads, the film plays in a session off the GUI thread, the library is not
# scanned, and nothing is written beside the film until a save is asked for.
cp "$media" "$scratch/film.mp4"
mv "$sources" "$scratch/sources.kept"
isolated_background --edit "$scratch/film.mp4" >"$scratch/trim.log" 2>&1 &
pid=$!
sleep 5
kill -0 "$pid" 2>/dev/null || fail "a trim launch ended by itself" "$scratch/trim.log"
trimming=$(cat /proc/"$pid"/task/*/comm 2>/dev/null | sort -u | tr '\n' ' ')
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true
errores=$(qml_errors "$scratch/trim.log")
[ -z "$errores" ] || fail "QML errors in a trim launch: $errores" "$scratch/trim.log"
case "$trimming" in
    *fluorita-player*) ;;
    *) fail "a trim launch has no playback session off the GUI thread (threads: $trimming)" "$scratch/trim.log" ;;
esac
[ ! -e "$sources" ] || fail "a trim launch scanned the library" "$scratch/trim.log"
mv "$scratch/sources.kept" "$sources"
written=$(find "$scratch" -maxdepth 1 -name '*film*' ! -name 'film.mp4' | head -n 1)
[ -z "$written" ] || fail "a trim launch wrote $written without a save" "$scratch/trim.log"

# ── 6) Automatic thumbnails ─────────────────────────────────────────────────
# The cache key the provider and the engine share: MD5 of the file:// URI as
# Qt spells it (celestina_core::percent::encode_qt_path).
cache_entry() {
    python3 - "$1" "$scratch/cache/thumbnails/large" <<'KEY'
import hashlib, sys, urllib.parse

uri = "file://" + urllib.parse.quote(sys.argv[1], safe="/-._~!$&'()*+,;=:@")
print(sys.argv[2] + "/" + hashlib.md5(uri.encode()).hexdigest() + ".png")
KEY
}

# Starts the library with no argument and waits up to 20 s for every entry
# named after the log, then reports the process's threads and stops it.
library_until() {
    log=$1
    shift
    HOME=$scratch \
    XDG_CONFIG_HOME=$scratch/config \
    XDG_DATA_HOME=$scratch/data \
    XDG_CACHE_HOME=$scratch/cache \
    XDG_STATE_HOME=$scratch/state \
    XDG_RUNTIME_DIR=$scratch/run \
    DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
    QT_QPA_PLATFORM=offscreen \
    QT_ASSUME_STDERR_HAS_CONSOLE=1 \
        "$bin" >"$log" 2>&1 &
    pid=$!
    waited=0
    while [ "$waited" -lt 40 ]; do
        missing=0
        for entry in "$@"; do
            [ -s "$entry" ] || missing=1
        done
        [ "$missing" -eq 0 ] && break
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.5
        waited=$((waited + 1))
    done
    seen=$(cat /proc/"$pid"/task/*/comm 2>/dev/null | sort -u | tr '\n' ' ')
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    for entry in "$@"; do
        [ -s "$entry" ] || fail "no thumbnail appeared at $entry" "$log"
    done
    echo "$seen"
}

mv "$scratch/foto.png" "$scratch/Pictures/foto.png"
photo_entry=$(cache_entry "$scratch/Pictures/foto.png")
photos=$(library_until "$scratch/fotos.log" "$photo_entry")
errores=$(qml_errors "$scratch/fotos.log")
[ -z "$errores" ] || fail "QML errors in a library of photos: $errores"
case "$photos" in
    *core*|*fluorita-player*)
        fail "a photo's thumbnail started the media engine (threads: $photos)" "$scratch/fotos.log" ;;
esac

cp "$media" "$scratch/Videos/clip.mp4"
clip_entry=$(cache_entry "$scratch/Videos/clip.mp4")
library_until "$scratch/poster.log" "$photo_entry" "$clip_entry" >/dev/null
errores=$(qml_errors "$scratch/poster.log")
[ -z "$errores" ] || fail "QML errors in a library with a clip: $errores"

echo "smoke: OK — QML carga, un vídeo abre sesión fuera del hilo GUI, ni la biblioteca ni una imagen ni un archivo desconocido arrancan el motor, --edit abre solo el editor (y el recorte de un vídeo), y las miniaturas llegan solas"
