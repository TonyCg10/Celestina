#!/bin/sh

set -eu

# install-i1.sh — install Siderita into the user's XDG prefix so the session
# treats it exactly like a packaged application: a binary on PATH, a desktop
# entry the launcher lists, and an icon in the hicolor theme.
#
# This is the *user* install (~/.local), which needs no root and is what a
# personal session wants. It links against the system Qt, like a distro package
# would; scripts/stage-i1.sh is the other shape — a self-contained prefix that
# carries its own Qt closure for a machine that has none.
#
# Everything is named org.celestina.Siderita: the entry, the icon, and the
# Wayland app_id the binary reports (src/main.rs). If those three ever disagree,
# the launcher shows a generic icon for a window it cannot tie back to its entry.

usage() {
    cat >&2 <<'EOF'
uso: scripts/install-i1.sh [--build] [--uninstall] [--prefix DIR]

Instala Siderita en el prefijo XDG del usuario (por defecto ~/.local):
  bin/siderita, share/applications/, share/icons/hicolor/…

opciones:
  --build       compila release antes de instalar
  --uninstall   elimina lo instalado y sale
  --prefix DIR  prefijo alternativo (por defecto: $XDG_DATA_HOME/.. o ~/.local)
EOF
}

build=0
uninstall=0
prefix=${HOME}/.local

while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        --build) build=1 ;;
        --uninstall) uninstall=1 ;;
        --prefix) shift; prefix=${1:?--prefix necesita un directorio} ;;
        *) echo "error: opción desconocida: $1" >&2; usage; exit 2 ;;
    esac
    shift
done

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
style_icons=$repo_root/../celestina-style/icons/apps

app_id=org.celestina.Siderita
bin_dir=$prefix/bin
apps_dir=$prefix/share/applications
icons_dir=$prefix/share/icons/hicolor
sizes="16 22 24 32 48 64 128 256 512"

if [ "$uninstall" -eq 1 ]; then
    rm -f "$bin_dir/siderita" "$apps_dir/$app_id.desktop"
    rm -f "$icons_dir/scalable/apps/$app_id.svg"
    for size in $sizes; do
        rm -f "$icons_dir/${size}x${size}/apps/$app_id.png"
    done
    update-desktop-database "$apps_dir" 2>/dev/null || true
    gtk-update-icon-cache -f -t "$icons_dir" 2>/dev/null || true
    echo ">> desinstalado de $prefix" >&2
    exit 0
fi

if [ "$build" -eq 1 ]; then
    ( cd "$repo_root" && cargo build --release --locked )
fi

binary=$repo_root/target/release/siderita-i1
if [ ! -x "$binary" ]; then
    echo "error: no existe el binario release: $binary (usa --build)" >&2
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
# Installed as `siderita`; the "i1" in the build artifact is the iteration, not
# the command the user types.
mkdir -p "$bin_dir"
install -m 0755 "$binary" "$bin_dir/siderita"

# ── Icon ─────────────────────────────────────────────────────────────────────
# The SVG is the master and lives with the shared visual language; the PNGs are
# generated, because a theme lookup at 16 px should not rasterize an SVG.
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

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$apps_dir/$app_id.desktop" \
        || echo "aviso: la entrada .desktop no valida limpiamente" >&2
fi
update-desktop-database "$apps_dir" 2>/dev/null || true
gtk-update-icon-cache -f -t "$icons_dir" 2>/dev/null || true

echo ">> instalado en $prefix" >&2
echo "   binario:  $bin_dir/siderita" >&2
echo "   entrada:  $apps_dir/$app_id.desktop" >&2
echo "   icono:    $icons_dir/{scalable,NxN}/apps/$app_id.{svg,png}" >&2
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "   aviso: $bin_dir no está en PATH" >&2 ;;
esac
echo "   para que sea el gestor por omisión:" >&2
echo "     xdg-mime default $app_id.desktop inode/directory" >&2
