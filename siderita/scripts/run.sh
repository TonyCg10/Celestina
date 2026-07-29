#!/bin/sh

set -eu

# run.sh — build Siderita in release and install it into the user's XDG prefix
# (~/.local), so the session treats it like a packaged app: a binary on PATH, a
# desktop entry the launcher lists, an icon in the hicolor theme, and the file-
# chooser portal backend. The one script Siderita needs — run it to build and
# ship the current tree to the launcher.
#
# Everything is named org.celestina.Siderita: the entry, the icon, and the
# Wayland app_id the binary reports (src/main.rs). If those three disagree, the
# launcher shows a generic icon for a window it cannot tie back to its entry.

usage() {
    cat >&2 <<'EOF'
uso: scripts/run.sh [--uninstall] [--prefix DIR] [--quick] [--no-deploy]

Compila Siderita en release y la instala en el prefijo XDG (por defecto ~/.local):
  bin/siderita, share/applications/, share/icons/hicolor/…, el portal de archivos

--quick:
  compila en modo debug (más rápido, incremental) y NO hace despliegue
  al sistema (sin iconos/desktop/portal/bus updates).
  Ideal para retoques rápidos de QML: ejecuta el binario desde target/debug/.

--no-deploy:
  compila en release y NO instala nada en ~/.local (solo deja el binario en
  target/..). útil para iterar sin tocar escritorio.

opciones:
  --uninstall   elimina lo instalado y sale (no compila)
  --prefix DIR  prefijo alternativo (por defecto ~/.local)
EOF
}

uninstall=0
quick=0
no_deploy=0
prefix=${HOME}/.local

while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        --uninstall) uninstall=1 ;;
        --quick) quick=1 ;;
        --no-deploy) no_deploy=1 ;;
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
portals_dir=$prefix/share/xdg-desktop-portal/portals
services_dir=$prefix/share/dbus-1/services
sizes="16 22 24 32 48 64 128 256 512"

if [ "$uninstall" -eq 1 ]; then
    rm -f "$bin_dir/siderita" "$apps_dir/$app_id.desktop"
    rm -f "$icons_dir/scalable/apps/$app_id.svg"
    rm -f "$portals_dir/celestina.portal"
    rm -f "$services_dir/org.freedesktop.impl.portal.desktop.celestina.service"
    for size in $sizes; do
        rm -f "$icons_dir/${size}x${size}/apps/$app_id.png"
    done
    update-desktop-database "$apps_dir" 2>/dev/null || true
    gtk-update-icon-cache -f -t "$icons_dir" 2>/dev/null || true
    echo ">> desinstalado de $prefix" >&2
    exit 0
fi

if [ "$quick" -eq 1 ]; then
    no_deploy=1
fi

if [ "$quick" -eq 1 ]; then
    # Debug + incremental is much faster for tight UI loops.
    ( cd "$repo_root" && CARGO_INCREMENTAL=1 cargo build --locked )
    binary=$repo_root/target/debug/siderita
else
    # ── Build ────────────────────────────────────────────────────────────────
    ( cd "$repo_root" && cargo build --release --locked )
    binary=$repo_root/target/release/siderita
fi

if [ "$no_deploy" -eq 1 ]; then
    echo ">> Siderita compilada en $binary"
    echo "   ejecuta: $binary [args]"
    echo "   (sin despliegue al sistema; ideal para ver cambios de UI)"
    exit 0
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
install -m 0755 "$binary" "$bin_dir/siderita"

# ── Icon ─────────────────────────────────────────────────────────────────────
# The SVG is the master (it lives with the shared visual language); the PNGs are
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

# ── Portal backend ───────────────────────────────────────────────────────────
# The D-Bus service file lets the file-chooser backend start on demand, so a
# file dialog works whether or not Siderita is already running; its Exec must be
# absolute, since the activation environment is not the user's shell.
mkdir -p "$portals_dir" "$services_dir"
install -m 0644 "$repo_root/portal/celestina.portal" "$portals_dir/celestina.portal"
sed "s|@BIN@|$bin_dir/siderita|" \
    "$repo_root/portal/org.freedesktop.impl.portal.desktop.celestina.service" \
    > "$services_dir/org.freedesktop.impl.portal.desktop.celestina.service"
chmod 0644 "$services_dir/org.freedesktop.impl.portal.desktop.celestina.service"

update-desktop-database "$apps_dir" 2>/dev/null || true
gtk-update-icon-cache -f -t "$icons_dir" 2>/dev/null || true
# The bus only learns about a new service file when it re-reads its directories.
busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus ReloadConfig >/dev/null 2>&1 || true

echo ">> Siderita compilada e instalada en $prefix" >&2
echo "   binario: $bin_dir/siderita" >&2
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "   aviso: $bin_dir no está en PATH" >&2 ;;
esac
echo "   ábrela desde el launcher (ciérrala antes si estaba abierta)." >&2
