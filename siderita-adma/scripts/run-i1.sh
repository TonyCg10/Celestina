#!/bin/sh

set -eu

# run-i1.sh — compila (si procede) y ejecuta el host Rust de la Primera
# Iteración de Siderita (siderita-i1). Complementa scripts/measure-i1.sh.
#
# El QML de i1 se compila dentro del binario, así que el binario se puede
# ejecutar desde cualquier directorio; RUTA es solo la carpeta a abrir.
# Los cambios de QML solo se ven tras recompilar (no con --no-build).

usage() {
    cat >&2 <<'EOF'
uso: scripts/run-i1.sh [opciones] [RUTA]

Compila (si procede) y ejecuta siderita-i1. Las opciones van antes de RUTA.
RUTA es una carpeta a abrir (por defecto: HOME).

opciones:
  --debug        compila/ejecuta el perfil debug (por defecto: release)
  --release      compila/ejecuta el perfil release (por defecto)
  --minimal      usa la feature qt-minimal (bootstrap de Qt; CI o sin Qt del sistema)
  --offscreen    ejecuta con QT_QPA_PLATFORM=offscreen (sin ventana; humo/headless)
  --no-build     no compila; ejecuta el binario existente (QML de la última build)
  -h, --help     muestra esta ayuda

entorno:
  QT_QPA_PLATFORM  plataforma Qt (por defecto: wayland; --offscreen la fuerza)

ejemplos:
  scripts/run-i1.sh                 # compila release y abre HOME en Wayland
  scripts/run-i1.sh ~/Descargas     # abre esa carpeta
  scripts/run-i1.sh --no-build      # reejecuta el binario ya compilado
  scripts/run-i1.sh --offscreen     # arranque headless de humo
  scripts/run-i1.sh --minimal       # build con bootstrap de Qt

para medir recursos: scripts/measure-i1.sh target/release/siderita-i1 [PID [SEG]]
EOF
}

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)

profile=release
features=
platform=
build=1

while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        --debug) profile=debug ;;
        --release) profile=release ;;
        --minimal|--qt-minimal) features="--features qt-minimal" ;;
        --offscreen) platform=offscreen ;;
        --no-build) build=0 ;;
        --) shift; break ;;
        -*) echo "error: opción desconocida: $1" >&2; usage; exit 2 ;;
        *) break ;;
    esac
    shift
done

binary="$repo_root/target/$profile/siderita-i1"

if [ "$build" -eq 1 ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        echo "error: cargo no está en PATH; instala Rust (rustup) o usa --no-build" >&2
        exit 127
    fi
    if [ -z "$features" ]; then
        if ! { command -v qmake6 >/dev/null 2>&1 \
               || command -v qmake >/dev/null 2>&1 \
               || [ -n "${QMAKE:-}" ]; }; then
            echo "aviso: no se encontró qmake (Qt) en PATH; si el build falla, usa --minimal o define QMAKE=/ruta/a/qmake" >&2
        fi
    fi

    # No usar `set --` aquí: "$@" guarda la RUTA a reenviar al binario.
    cargo_flags="--locked"
    if [ "$profile" = "release" ]; then
        cargo_flags="$cargo_flags --release"
    fi
    if [ -n "$features" ]; then
        cargo_flags="$cargo_flags $features"
    fi

    echo ">> cargo build $cargo_flags" >&2
    # $cargo_flags sin comillas a propósito, para separarlo en palabras.
    ( cd "$repo_root" && cargo build $cargo_flags )
fi

if [ ! -x "$binary" ]; then
    echo "error: no existe el binario: $binary" >&2
    echo "       compila primero (quita --no-build) o revisa el perfil" >&2
    exit 1
fi

if [ -z "$platform" ]; then
    platform="${QT_QPA_PLATFORM:-wayland}"
fi
if [ "$platform" = "wayland" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    echo "aviso: WAYLAND_DISPLAY vacío; usa --offscreen o exporta QT_QPA_PLATFORM" >&2
fi

echo ">> QT_QPA_PLATFORM=$platform $binary $*" >&2
exec env QT_QPA_PLATFORM="$platform" "$binary" "$@"
