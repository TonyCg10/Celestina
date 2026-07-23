#!/bin/sh

set -eu

# stage-i1.sh — stage siderita-i1 into a self-contained prefix that runs without
# the system's Qt on QML_IMPORT_PATH, copying ONLY an allowlist of Qt QML modules
# and plugins (Basic style + what the app actually imports) plus the transitive
# Qt shared-library closure of the binary and every copied plugin .so.
#
# The point is the first-install closure the roadmap budgets at 250 MiB: after
# staging, measure it with
#   SIDERITA_CLOSURE_STAGE=<stage> scripts/measure-i1.sh <stage>/bin/siderita-i1
#
# It does not vendor the system C/C++ runtime or the graphics stack (libwayland,
# Mesa, fontconfig …) — those are assumed present, as on any target desktop.

usage() {
    cat >&2 <<'EOF'
uso: scripts/stage-i1.sh [--build] [STAGE_DIR]

Copia siderita-i1 y su cierre Qt (lista blanca) a STAGE_DIR (por defecto
target/stage). Con --build compila release antes de copiar.

entorno:
  QT_QML_DIR      raíz de módulos QML de Qt (por defecto: qmake6 -query)
  QT_PLUGIN_DIR   raíz de plugins de Qt (por defecto: qmake6 -query)
EOF
}

build=0
stage=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help) usage; exit 0 ;;
        --build) build=1 ;;
        -*) echo "error: opción desconocida: $1" >&2; usage; exit 2 ;;
        *) stage=$1 ;;
    esac
    shift
done

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
stage=${stage:-$repo_root/target/stage}
binary=$repo_root/target/release/siderita-i1

if [ "$build" -eq 1 ]; then
    ( cd "$repo_root" && cargo build --release --locked )
fi
if [ ! -x "$binary" ]; then
    echo "error: no existe el binario release: $binary (usa --build)" >&2
    exit 1
fi

qt_qml=${QT_QML_DIR:-$(qmake6 -query QT_INSTALL_QML 2>/dev/null || true)}
qt_plugins=${QT_PLUGIN_DIR:-$(qmake6 -query QT_INSTALL_PLUGINS 2>/dev/null || true)}
if [ -z "$qt_qml" ] || [ -z "$qt_plugins" ]; then
    echo "error: no se hallaron las rutas de Qt; define QT_QML_DIR y QT_PLUGIN_DIR" >&2
    exit 1
fi

# ── Allowlist ────────────────────────────────────────────────────────────────
# QML modules the app imports (directly or transitively). Everything else in the
# system QML dir is intentionally excluded.
qml_modules="
QtQml
QtQuick
QtQuick/Controls
QtQuick/Controls/Basic
QtQuick/Controls/impl
QtQuick/Templates
QtQuick/Effects
QtQuick/Layouts
QtQuick/Window
"
# Plugin files (relative to the Qt plugin dir). The Wayland platform (+ its
# client integrations) and an X11 fallback; the SVG image format (icon
# fallbacks) and the SVG icon engine (themed icons). Nothing else.
plugin_globs="
platforms/libqwayland.so
platforms/libqxcb.so
platforms/libqoffscreen.so
wayland-shell-integration
wayland-graphics-integration-client
wayland-decoration-client
imageformats/libqsvg.so
iconengines/libqsvgicon.so
"

rm -rf "$stage"
mkdir -p "$stage/bin" "$stage/lib" "$stage/qml" "$stage/plugins"

cp "$binary" "$stage/bin/siderita-i1"
cp "$repo_root/siderita-adma.desktop" "$stage/" 2>/dev/null || true

echo ">> copiando módulos QML (lista blanca)" >&2
for module in $qml_modules; do
    src=$qt_qml/$module
    if [ -d "$src" ]; then
        mkdir -p "$stage/qml/$module"
        # Copy the module's files but not nested sub-modules already listed.
        find "$src" -maxdepth 1 -type f -exec cp {} "$stage/qml/$module/" \;
    else
        echo "   aviso: módulo QML ausente: $module" >&2
    fi
done

echo ">> copiando plugins (lista blanca)" >&2
for entry in $plugin_globs; do
    src=$qt_plugins/$entry
    dest=$stage/plugins/$entry
    if [ -e "$src" ]; then
        mkdir -p "$(dirname "$dest")"
        cp -r "$src" "$dest"
    else
        echo "   aviso: plugin ausente: $entry" >&2
    fi
done

# ── Qt shared-library closure ────────────────────────────────────────────────
# Union of the libQt6*.so the binary and every copied .so need, resolved with
# ldd and copied into lib/. Non-Qt system libs are left to the target.
echo ">> resolviendo el cierre de bibliotecas Qt" >&2
resolve_qt_libs() {
    for obj in "$@"; do
        ldd "$obj" 2>/dev/null | awk '/=>/ && /libQt6/ { print $3 }'
    done
}

# Iterate to a fixed point: copied Qt libs can pull in more Qt libs.
scan_objects=$(find "$stage/bin" "$stage/plugins" "$stage/qml" -type f \
    \( -name '*.so' -o -name 'siderita-i1' \) 2>/dev/null)
copied=""
while : ; do
    changed=0
    # shellcheck disable=SC2086
    for lib in $(resolve_qt_libs $scan_objects $(find "$stage/lib" -type f 2>/dev/null) | sort -u); do
        base=$(basename "$lib")
        if [ ! -e "$stage/lib/$base" ] && [ -e "$lib" ]; then
            cp -L "$lib" "$stage/lib/$base"
            copied="$copied $base"
            changed=1
        fi
    done
    [ "$changed" -eq 0 ] && break
done

# ── Launcher ─────────────────────────────────────────────────────────────────
cat > "$stage/siderita-i1" <<'LAUNCH'
#!/bin/sh
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export LD_LIBRARY_PATH="$here/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export QML_IMPORT_PATH="$here/qml"
export QML2_IMPORT_PATH="$here/qml"
export QT_PLUGIN_PATH="$here/plugins"
export QT_QPA_PLATFORM_PLUGIN_PATH="$here/plugins/platforms"
export QT_QUICK_CONTROLS_STYLE="${QT_QUICK_CONTROLS_STYLE:-Basic}"
exec "$here/bin/siderita-i1" "$@"
LAUNCH
chmod +x "$stage/siderita-i1"

size=$(du -sh "$stage" | awk '{print $1}')
echo ">> escenificado en $stage ($size)" >&2
echo "   ejecuta: $stage/siderita-i1 [RUTA]" >&2
echo "   mide el cierre: SIDERITA_CLOSURE_STAGE=$stage scripts/measure-i1.sh $stage/bin/siderita-i1" >&2
