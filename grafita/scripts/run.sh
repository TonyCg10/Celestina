#!/bin/sh

set -eu

# run.sh — build Grafita in release and install it into the user's XDG prefix
# (~/.local): a binary on PATH, the desktop entry the launcher and "Abrir con"
# list, and the icon in the hicolor theme.
#
# --prefix exists so the install can be exercised against a throwaway directory
# instead of the user's own: the point of an installer test is to prove the
# layout, not to require trusting it first.

usage() {
    cat >&2 <<'EOF'
uso: scripts/run.sh [--uninstall] [--no-build] [--prefix DIR]

Compila Grafita en release y la instala en el prefijo XDG (por defecto ~/.local):
  bin/grafita, share/applications/, share/icons/hicolor/…

opciones:
  --uninstall   elimina lo instalado y sale (no compila)
  --no-build    instala el binario ya compilado (para probar el instalador)
  --prefix DIR  prefijo alternativo (por defecto ~/.local)
EOF
}

uninstall=0
build=1
prefix=${HOME}/.local

while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        --uninstall) uninstall=1 ;;
        --no-build) build=0 ;;
        --prefix) shift; prefix=${1:?--prefix necesita un directorio} ;;
        *) echo "error: opción desconocida: $1" >&2; usage; exit 2 ;;
    esac
    shift
done

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
style_icons=$repo_root/../celestina-style/icons/apps

app_id=org.celestina.Grafita
bin_dir=$prefix/bin
apps_dir=$prefix/share/applications
icons_dir=$prefix/share/icons/hicolor
sizes="16 22 24 32 48 64 128 256 512"

if [ "$uninstall" -eq 1 ]; then
    rm -f "$bin_dir/grafita" "$apps_dir/$app_id.desktop"
    rm -f "$icons_dir/scalable/apps/$app_id.svg"
    for size in $sizes; do
        rm -f "$icons_dir/${size}x${size}/apps/$app_id.png"
    done
    update-desktop-database "$apps_dir" 2>/dev/null || true
    gtk-update-icon-cache -f -t "$icons_dir" 2>/dev/null || true
    echo ">> desinstalada de $prefix" >&2
    exit 0
fi

# ── Build ────────────────────────────────────────────────────────────────────
if [ "$build" -eq 1 ]; then
    ( cd "$repo_root" && cargo build --release --locked )
fi

binary=$repo_root/target/release/grafita
if [ ! -x "$binary" ]; then
    echo "error: falta $binary — compila antes (cargo build --release)" >&2
    exit 1
fi
if [ ! -f "$style_icons/$app_id.svg" ]; then
    echo "error: falta el icono: $style_icons/$app_id.svg" >&2
    exit 1
fi
if ! command -v rsvg-convert >/dev/null 2>&1; then
    echo "error: se necesita rsvg-convert (librsvg) para generar los PNG" >&2
    exit 1
fi

# ── Binary ───────────────────────────────────────────────────────────────────
mkdir -p "$bin_dir"
install -m 0755 "$binary" "$bin_dir/grafita"

# ── Icon ─────────────────────────────────────────────────────────────────────
mkdir -p "$icons_dir/scalable/apps"
install -m 0644 "$style_icons/$app_id.svg" "$icons_dir/scalable/apps/$app_id.svg"
for size in $sizes; do
    mkdir -p "$icons_dir/${size}x${size}/apps"
    rsvg-convert -w "$size" -h "$size" "$style_icons/$app_id.svg" \
        -o "$icons_dir/${size}x${size}/apps/$app_id.png"
done

# ── Desktop entry ────────────────────────────────────────────────────────────
mkdir -p "$apps_dir"
install -m 0644 "$repo_root/$app_id.desktop" "$apps_dir/$app_id.desktop"

update-desktop-database "$apps_dir" 2>/dev/null || true
gtk-update-icon-cache -f -t "$icons_dir" 2>/dev/null || true

echo ">> Grafita compilada e instalada en $prefix" >&2
echo "   binario: $bin_dir/grafita" >&2
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "   aviso: $bin_dir no está en PATH" >&2 ;;
esac
echo "   ábrela desde el launcher o con: grafita RUTA" >&2
