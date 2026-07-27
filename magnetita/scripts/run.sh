#!/bin/sh

set -eu

# run.sh — build Magnetita in release and install it into the user's XDG prefix
# (~/.local): a binary on PATH, the desktop entry the launcher lists, and the
# icon in the hicolor theme. The one script Magnetita needs — run it to build and
# ship the current tree to the launcher. (The daemon, magnetitad, is a separate
# systemd user service and is not touched here.)

usage() {
    cat >&2 <<'EOF'
uso: scripts/run.sh [--uninstall] [--prefix DIR]

Compila Magnetita en release y la instala en el prefijo XDG (por defecto ~/.local):
  bin/magnetita, share/applications/, share/icons/hicolor/…

opciones:
  --uninstall   elimina lo instalado y sale (no compila)
  --prefix DIR  prefijo alternativo (por defecto ~/.local)
EOF
}

uninstall=0
prefix=${HOME}/.local

while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        --uninstall) uninstall=1 ;;
        --prefix) shift; prefix=${1:?--prefix necesita un directorio} ;;
        *) echo "error: opción desconocida: $1" >&2; usage; exit 2 ;;
    esac
    shift
done

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
style_icons=$repo_root/../celestina-style/icons/apps

app_id=org.celestina.Magnetita
bin_dir=$prefix/bin
apps_dir=$prefix/share/applications
icons_dir=$prefix/share/icons/hicolor
sizes="16 22 24 32 48 64 128 256 512"

if [ "$uninstall" -eq 1 ]; then
    rm -f "$bin_dir/magnetita" "$apps_dir/$app_id.desktop"
    rm -f "$icons_dir/scalable/apps/$app_id.svg"
    for size in $sizes; do
        rm -f "$icons_dir/${size}x${size}/apps/$app_id.png"
    done
    update-desktop-database "$apps_dir" 2>/dev/null || true
    gtk-update-icon-cache -f -t "$icons_dir" 2>/dev/null || true
    echo ">> desinstalado de $prefix" >&2
    exit 0
fi

# ── Build ────────────────────────────────────────────────────────────────────
( cd "$repo_root" && cargo build --release --locked )

binary=$repo_root/target/release/magnetita
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
install -m 0755 "$binary" "$bin_dir/magnetita"

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

echo ">> Magnetita compilada e instalada en $prefix" >&2
echo "   binario: $bin_dir/magnetita" >&2
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "   aviso: $bin_dir no está en PATH" >&2 ;;
esac
echo "   ábrela desde el launcher (ciérrala antes si estaba abierta)." >&2
