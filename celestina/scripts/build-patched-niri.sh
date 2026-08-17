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
# The session's compositor, by version *and* by commit.
#
# The commit is what matters and the version alone was not enough. This script
# used to ask for the version tag and, when that failed, silently clone the
# default branch — and it failed every time, because niri's release tag is
# `v26.04` while `niri --version` prints `26.04`. The nest therefore ran niri
# `main` for months while the session ran the April release, so every visual
# check of the glass, the membrane and the blur was made against a compositor
# carrying post-release fixes the session does not have. The first live
# migration is where that was discovered, by crashing.
#
# `niri --version` prints `niri <version> (<commit>)`, and a patched build
# appends `-modified` to its commit; both are parsed off.
version=$(niri --version 2>/dev/null | awk '{print $2}')
session_commit=$(niri --version 2>/dev/null \
    | sed -n 's/.*(\([0-9a-f]\{7,\}\).*/\1/p')
if [ -z "$version" ] || [ -z "$session_commit" ]; then
    echo "build-patched-niri: cannot read the session's niri version." >&2
    echo "   Refusing rather than guessing which compositor to build." >&2
    exit 1
fi
echo ">> building niri $version ($session_commit) with the per-layer blur strength patch" >&2

mkdir -p "$(dirname "$work_dir")"
[ -d "$work_dir/.git" ] || git init -q "$work_dir"
git -C "$work_dir" remote get-url origin >/dev/null 2>&1 \
    || git -C "$work_dir" remote add origin https://github.com/YaLTeR/niri.git

# The tag first, because a shallow fetch of an arbitrary short commit is not
# something the smart protocol serves. `v<version>` is niri's actual tag
# spelling; the bare version is tried too in case that ever changes.
fetched=""
for ref in "v$version" "$version"; do
    if git -C "$work_dir" fetch --depth 1 --force origin \
        "refs/tags/$ref:refs/tags/$ref" 2>/dev/null; then
        fetched=$ref
        break
    fi
done
if [ -z "$fetched" ]; then
    echo "build-patched-niri: no tag v$version or $version upstream." >&2
    echo "   The nest must be the session's compositor; refusing to build a" >&2
    echo "   different one. Check what \`niri --version\` reports." >&2
    exit 1
fi

git -C "$work_dir" checkout --force -q "refs/tags/$fetched"
git -C "$work_dir" reset --hard -q

# The invariant this whole block exists for: what was checked out is the commit
# the session actually runs. Anything else is a nest that cannot predict the
# session, which is the defect being fixed here — so it is a refusal, never a
# warning.
built_commit=$(git -C "$work_dir" rev-parse --short=40 HEAD)
case "$built_commit" in
"$session_commit"*) ;;
*)
    echo "build-patched-niri: tag $fetched is $built_commit, but this session" >&2
    echo "   runs $session_commit. The nest would not be the session's" >&2
    echo "   compositor, so nothing is built." >&2
    exit 1
    ;;
esac

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
