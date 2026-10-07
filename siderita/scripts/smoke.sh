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
#  3) Thumbnails through the shared provider: when no path is given, the
#     scratch folder holds one real 600x300 PNG, and the launch must leave its
#     256x128 entry in the scratch freedesktop cache, keyed on the file's URI
#     and carrying a proper `Thumb::URI` text chunk (the provider Siderita used
#     to compile lost that key).

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

# Step 3's fixture: a real image, encoded here so the smoke needs no tools
# beyond Python's standard library.
photo=
if [ -z "$requested_path" ]; then
    photo=$scratch/home/photo.png
    if ! python3 - "$photo" <<'PY'
import struct, sys, zlib
width, height = 600, 300
rows = b"".join(
    b"\x00" + bytes(v for x in range(width) for v in (x % 256, y % 256, 128))
    for y in range(height)
)
def chunk(kind, data):
    return (struct.pack(">I", len(data)) + kind + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF))
with open(sys.argv[1], "wb") as out:
    out.write(b"\x89PNG\r\n\x1a\n")
    out.write(chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)))
    out.write(chunk(b"IDAT", zlib.compress(rows)))
    out.write(chunk(b"IEND", b""))
PY
    then
        echo "smoke: could not write the thumbnail fixture" >&2
        exit 1
    fi
fi

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

# Step 3: the shared provider produced the folder's one image thumbnail.
if [ -n "$photo" ]; then
    if ! thumb=$(python3 - "$photo" "$scratch/cache/thumbnails/large" <<'PY'
import hashlib, struct, sys, urllib.parse, zlib
photo, large = sys.argv[1], sys.argv[2]
# The freedesktop key, spelled as `QUrl::fromLocalFile().toEncoded()` does.
uri = "file://" + urllib.parse.quote(photo, safe="!$&'()*+,;=:@/~")
entry = f"{large}/{hashlib.md5(uri.encode()).hexdigest()}.png"
try:
    data = open(entry, "rb").read()
except OSError:
    sys.exit(f"no cache entry for {uri}")
width, height = struct.unpack(">II", data[16:24])
if (width, height) != (256, 128):
    sys.exit(f"{entry} is {width}x{height}, not 256x128")
texts, at = {}, 8
while at < len(data):
    size, kind = struct.unpack(">I4s", data[at:at + 8])
    body = data[at + 8:at + 8 + size]
    if kind == b"tEXt":
        key, _, value = body.partition(b"\0")
        texts[key] = value
    elif kind == b"zTXt":
        key, _, rest = body.partition(b"\0")
        texts[key] = zlib.decompress(rest[1:])
    at += 12 + size
if texts.get(b"Thumb::URI") != uri.encode():
    sys.exit(f"{entry} does not carry Thumb::URI {uri}: {sorted(texts)}")
print(entry)
PY
    ); then
        echo "smoke: the image thumbnail check failed (above); last lines:" >&2
        tail -20 "$log" >&2
        exit 1
    fi
    echo "smoke: thumbnail produced through the shared provider: $thumb"
fi

echo "smoke: OK — binario vivo 8 s, sin errores QML, sin auto-bindings"
