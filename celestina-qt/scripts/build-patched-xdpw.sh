#!/bin/sh

set -eu

# build-patched-xdpw.sh — build the screen-sharing backend the session needs.
#
# The session's ScreenCast portal is xdg-desktop-portal-wlr, and niri offers it
# only wlr-screencopy. Release 0.8.x drives the PipeWire graph itself
# (PW_STREAM_FLAG_DRIVER) but only the ext-image-copy-capture path asks for the
# next cycle, so on this compositor every shared screen delivered one frame
# and froze. Upstream fixed it in c0255d7b (2026-08-11) without a release;
# `packaging/xdg-desktop-portal-wlr/trigger-process-after-screencopy.patch`
# carries that fix onto the released tag. See that directory's README.
#
# Nothing outside the install directory is touched: the distribution's own
# /usr/lib/xdg-desktop-portal-wlr stays as it is, and the session keeps using
# it until the systemd user override below is installed. The override is
# written only when asked (`--install-override`), because it changes what the
# login session runs.

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch_file=$here/../packaging/xdg-desktop-portal-wlr/trigger-process-after-screencopy.patch
install_dir=${CELESTINA_XDPW_PREFIX:-$HOME/.local/libexec}
work_dir=${CELESTINA_XDPW_WORKDIR:-$HOME/.cache/celestina/xdpw-src}
tools_dir=${CELESTINA_XDPW_TOOLS:-$HOME/.cache/celestina/xdpw-tools}
override_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/xdg-desktop-portal-wlr.service.d

install_override=false
for argument in "$@"; do
    case "$argument" in
    --install-override) install_override=true ;;
    *)
        echo "usage: build-patched-xdpw.sh [--install-override]" >&2
        exit 2
        ;;
    esac
done

for tool in git cc pkg-config python3; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "build-patched-xdpw: $tool is missing" >&2
        exit 1
    }
done
[ -f "$patch_file" ] || {
    echo "build-patched-xdpw: $patch_file is missing" >&2
    exit 1
}

# The version of the backend the distribution installed, so the patched build
# tracks the session's own release rather than upstream's tip. The package
# manager is the only place that version is written down: the binary itself
# prints none.
version=${CELESTINA_XDPW_VERSION:-}
if [ -z "$version" ] && command -v pacman >/dev/null 2>&1; then
    version=$(pacman -Q xdg-desktop-portal-wlr 2>/dev/null \
        | awk '{print $2}' | sed 's/-[0-9.]*$//')
fi
if [ -z "$version" ]; then
    echo "build-patched-xdpw: cannot read the installed xdg-desktop-portal-wlr" >&2
    echo "   version. Set CELESTINA_XDPW_VERSION (for example 0.8.4)." >&2
    exit 1
fi
echo ">> building xdg-desktop-portal-wlr $version with the screencopy trigger patch" >&2

# meson and ninja are build-time only. When the host does not carry them, a
# private virtual environment does, so the build asks nothing of the system.
meson=$(command -v meson || true)
ninja=$(command -v ninja || true)
if [ -z "$meson" ] || [ -z "$ninja" ]; then
    [ -x "$tools_dir/bin/meson" ] || {
        echo ">> meson/ninja are not on the host; installing them privately" >&2
        python3 -m venv "$tools_dir"
        "$tools_dir/bin/pip" install --quiet meson ninja
    }
    meson=$tools_dir/bin/meson
    ninja=$tools_dir/bin/ninja
    PATH=$tools_dir/bin:$PATH
    export PATH
fi

mkdir -p "$(dirname "$work_dir")"
[ -d "$work_dir/.git" ] || git init -q "$work_dir"
git -C "$work_dir" remote get-url origin >/dev/null 2>&1 \
    || git -C "$work_dir" remote add origin \
        https://github.com/emersion/xdg-desktop-portal-wlr.git

fetched=""
for ref in "v$version" "$version"; do
    if git -C "$work_dir" fetch --depth 1 --force origin \
        "refs/tags/$ref:refs/tags/$ref" 2>/dev/null; then
        fetched=$ref
        break
    fi
done
if [ -z "$fetched" ]; then
    echo "build-patched-xdpw: no tag v$version or $version upstream." >&2
    exit 1
fi

git -C "$work_dir" checkout --force -q "refs/tags/$fetched"
git -C "$work_dir" reset --hard -q

# Idempotent: a re-run over an already-patched tree is not an error.
if ! git -C "$work_dir" apply --check "$patch_file" 2>/dev/null; then
    if git -C "$work_dir" apply --check --reverse "$patch_file" 2>/dev/null; then
        echo ">> the tree already carries the patch" >&2
    else
        echo "build-patched-xdpw: the patch does not apply to $version." >&2
        echo "   Upstream moved; check whether c0255d7b is already in this" >&2
        echo "   release, and if so drop the override instead of rebuilding." >&2
        exit 1
    fi
else
    git -C "$work_dir" apply "$patch_file"
fi

# Man pages need scdoc and the unit file is not installed at all: the session
# keeps the distribution's unit and only points its ExecStart elsewhere.
[ -d "$work_dir/build" ] || "$meson" setup "$work_dir/build" "$work_dir" \
    -Dsd-bus-provider=libsystemd -Dsystemd=disabled -Dman-pages=disabled >&2
"$ninja" -C "$work_dir/build" >&2

mkdir -p "$install_dir"
install -m 0755 "$work_dir/build/xdg-desktop-portal-wlr" \
    "$install_dir/xdg-desktop-portal-wlr"
echo >&2
echo ">> installed $install_dir/xdg-desktop-portal-wlr" >&2

if [ "$install_override" = true ]; then
    mkdir -p "$override_dir"
    cat > "$override_dir/override.conf" <<EOF
# Written by celestina/scripts/build-patched-xdpw.sh: the released backend
# freezes every shared screen on its first frame under niri (upstream
# c0255d7b, unreleased). Delete this file, daemon-reload and restart the
# service to return to the distribution's binary.
[Service]
ExecStart=
ExecStart=$install_dir/xdg-desktop-portal-wlr
EOF
    systemctl --user daemon-reload
    systemctl --user restart xdg-desktop-portal-wlr.service
    echo ">> the session now runs the patched backend ($override_dir/override.conf)" >&2
else
    echo "   To run the session on it: $0 --install-override" >&2
    echo "   (or see packaging/xdg-desktop-portal-wlr/README.md)." >&2
fi
