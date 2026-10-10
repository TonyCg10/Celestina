#!/bin/sh

set -eu

# build-portal.sh — build the ScreenCast backend Selenita records through.
#
# Under niri the session's ScreenCast portal is xdg-desktop-portal-wlr over
# wlr-screencopy. Every release up to 0.8.4 stalls there (one frame, then
# nothing), and upstream's master still loses whole recordings to a buffer
# returned with a zero timestamp. This script builds upstream at a pinned
# commit plus the patches in `../packaging/xdg-desktop-portal-wlr/` (see its
# README) and installs the binary beside the user's own programs. The
# distribution's /usr/lib/xdg-desktop-portal-wlr is never touched.
#
# The session only runs the build once a systemd user drop-in points the
# service at it; `--install-override` writes that drop-in and restarts the
# service, because it changes what the login session runs.
#
# Variables:
#   SELENITA_PORTAL_PREFIX   install folder (default ~/.local/libexec)
#   SELENITA_PORTAL_WORKDIR  source checkout (default ~/.cache/selenita/xdpw-src)
#   SELENITA_PORTAL_TOOLS    venv with meson and ninja, used when the host
#                            has none (default: the existing
#                            ~/.cache/celestina/xdpw-tools, else
#                            ~/.cache/selenita/xdpw-tools)
#   SELENITA_PORTAL_REF      upstream commit to build

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch_dir=$here/../packaging/xdg-desktop-portal-wlr
upstream=https://github.com/emersion/xdg-desktop-portal-wlr.git
ref=${SELENITA_PORTAL_REF:-c0255d7b047b7263629ab5a314661045ff7f65e3}
install_dir=${SELENITA_PORTAL_PREFIX:-$HOME/.local/libexec}
work_dir=${SELENITA_PORTAL_WORKDIR:-$HOME/.cache/selenita/xdpw-src}
if [ -n "${SELENITA_PORTAL_TOOLS:-}" ]; then
    tools_dir=$SELENITA_PORTAL_TOOLS
elif [ -x "$HOME/.cache/celestina/xdpw-tools/bin/meson" ]; then
    tools_dir=$HOME/.cache/celestina/xdpw-tools
else
    tools_dir=$HOME/.cache/selenita/xdpw-tools
fi
override_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/xdg-desktop-portal-wlr.service.d

usage() {
    echo "usage: build-portal.sh [--install-override] [--help]" >&2
}

install_override=false
for argument in "$@"; do
    case "$argument" in
    --install-override) install_override=true ;;
    --help | -h)
        usage
        exit 0
        ;;
    *)
        usage
        exit 2
        ;;
    esac
done

for tool in git cc pkg-config python3; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "build-portal: $tool is missing" >&2
        exit 1
    }
done

# meson and ninja are needed only to build. When the host has neither, a
# private virtual environment carries them.
meson=$(command -v meson || true)
ninja=$(command -v ninja || true)
if [ -z "$meson" ] || [ -z "$ninja" ]; then
    if [ ! -x "$tools_dir/bin/meson" ]; then
        echo ">> meson/ninja are not on the host; installing them into $tools_dir" >&2
        python3 -m venv "$tools_dir"
        "$tools_dir/bin/pip" install --quiet meson ninja
    fi
    PATH=$tools_dir/bin:$PATH
    export PATH
fi

if [ -d "$work_dir/.git" ]; then
    git -C "$work_dir" fetch --quiet origin
else
    mkdir -p "$(dirname -- "$work_dir")"
    git clone --quiet "$upstream" "$work_dir"
fi
git -C "$work_dir" checkout --quiet --force --detach "$ref"
git -C "$work_dir" clean --quiet -fdx -e build

for patch in "$patch_dir"/*.patch; do
    [ -f "$patch" ] || continue
    git -C "$work_dir" apply --check "$patch"
    git -C "$work_dir" apply "$patch"
    echo ">> applied $(basename -- "$patch")" >&2
done

if [ -d "$work_dir/build" ]; then
    meson setup --reconfigure "$work_dir/build" "$work_dir" --buildtype=release \
        -Dsd-bus-provider=libsystemd >/dev/null
else
    meson setup "$work_dir/build" "$work_dir" --buildtype=release \
        -Dsd-bus-provider=libsystemd >/dev/null
fi
ninja -C "$work_dir/build" >/dev/null

built=$work_dir/build/xdg-desktop-portal-wlr
target=$install_dir/xdg-desktop-portal-wlr
mkdir -p "$install_dir"
if [ -f "$target" ]; then
    cp -f "$target" "$target.prev"
fi
cp -f "$built" "$target.new"
mv -f "$target.new" "$target"
echo ">> installed $target" >&2
sha256sum "$target"

if [ "$install_override" = true ]; then
    mkdir -p "$override_dir"
    cat >"$override_dir/override.conf" <<EOF
# Written by selenita/scripts/build-portal.sh: run the suite's build of the
# ScreenCast backend instead of the distribution's.
[Service]
ExecStart=
ExecStart=$target
EOF
    systemctl --user daemon-reload
    systemctl --user restart xdg-desktop-portal-wlr.service
    echo ">> the session's ScreenCast backend now runs $target" >&2
else
    echo ">> the session runs this build only through the systemd drop-in;" >&2
    echo "   run again with --install-override, or restart the service if the" >&2
    echo "   drop-in already points at $target:" >&2
    echo "   systemctl --user restart xdg-desktop-portal-wlr.service" >&2
fi
