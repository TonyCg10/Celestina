#!/bin/sh

set -eu

# build-patched-niri.sh — build the niri Celestina's dense glass wants.
#
# The shell's material needs one blur strength for its veil and a stronger one
# for its dark sections. A compositor grants one strength per surface, and
# niri's per-surface rule accepts overrides for noise and saturation but not
# for the strength itself; `packaging/niri/per-layer-blur-strength.patch` adds
# the two missing ones. See that directory's README for why stacking surfaces
# is not an alternative.
#
# Nothing outside the install directory is touched. The distribution's own
# /usr/bin/niri is left exactly as it is, and the login session keeps using it
# until someone deliberately overrides the systemd unit.

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
patch_file=$here/../packaging/niri/per-layer-blur-strength.patch
install_dir=${CELESTINA_NIRI_PREFIX:-$HOME/.local/lib/celestina}
work_dir=${CELESTINA_NIRI_WORKDIR:-$HOME/.cache/celestina/niri-src}

for tool in git cargo; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "build-patched-niri: $tool is missing" >&2
        exit 1
    }
done
[ -f "$patch_file" ] || {
    echo "build-patched-niri: $patch_file is missing" >&2
    exit 1
}

# The tag of the niri already installed, so the patched build tracks the
# session's own version rather than whatever upstream's tip happens to be.
version=$(niri --version 2>/dev/null | awk '{print $2}')
[ -n "$version" ] || version=main
echo ">> building niri $version with the per-layer blur strength patch" >&2

if [ -d "$work_dir/.git" ]; then
    git -C "$work_dir" fetch --tags --depth 1 origin "$version" 2>/dev/null || true
    git -C "$work_dir" checkout --force FETCH_HEAD 2>/dev/null \
        || git -C "$work_dir" checkout --force "$version"
    git -C "$work_dir" reset --hard
else
    mkdir -p "$(dirname "$work_dir")"
    git clone --depth 1 --branch "$version" \
        https://github.com/YaLTeR/niri.git "$work_dir" 2>/dev/null \
        || git clone --depth 1 https://github.com/YaLTeR/niri.git "$work_dir"
fi

# Idempotent: a re-run over an already-patched tree is not an error.
if ! git -C "$work_dir" apply --check "$patch_file" 2>/dev/null; then
    if git -C "$work_dir" apply --check --reverse "$patch_file" 2>/dev/null; then
        echo ">> the tree already carries the patch" >&2
    else
        echo "build-patched-niri: the patch does not apply to niri $version." >&2
        echo "   Upstream moved; rebase packaging/niri/per-layer-blur-strength.patch." >&2
        exit 1
    fi
else
    git -C "$work_dir" apply "$patch_file"
fi

( cd "$work_dir" && cargo build --release )

mkdir -p "$install_dir"
install -m 0755 "$work_dir/target/release/niri" "$install_dir/niri"

echo >&2
echo ">> installed $install_dir/niri" >&2
echo "   dev-session.sh picks it up automatically." >&2
echo "   To run the login session on it, see packaging/niri/README.md." >&2
