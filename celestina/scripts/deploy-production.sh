#!/bin/sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
suite_root=$(CDPATH= cd -- "$project_root/.." && pwd)
. "$suite_root/scripts/production-common.sh"

prefix=${HOME}/.local
while [ "$#" -gt 0 ]; do
    case "$1" in
        -h|--help)
            echo "usage: scripts/deploy-production.sh [--prefix DIR]" >&2
            exit 0
            ;;
        --prefix)
            shift
            prefix=${1:?--prefix requires a directory}
            ;;
        *)
            echo "deploy-production: unknown option: $1" >&2
            exit 2
            ;;
    esac
    shift
done

production_require_verified "$suite_root" celestina
bundle=$prefix/libexec/celestina
production_install_file "$project_root/build/celestina" "$bundle/celestina" 0755
production_install_file \
    "$project_root/build/rust-target/release/celestina-niri-adapter" \
    "$bundle/celestina-niri-adapter" 0755
production_install_file \
    "$project_root/build/rust-target/release/celestina-provider-adapter" \
    "$bundle/celestina-provider-adapter" 0755
production_install_file \
    "$suite_root/celestina-style/build/libcelestina-style.so" \
    "$bundle/libcelestina-style.so" 0755
production_install_tree \
    "$suite_root/celestina-style/build/CelestinaStyle" \
    "$bundle/CelestinaStyle"
production_install_file \
    "$project_root/scripts/celestina-launcher.sh" "$prefix/bin/celestina" 0755
# Not a launcher entry — it is `NoDisplay` — but the application information the
# desktop needs to answer for the running shell. Without it the portal declines
# to register the host at all.
production_install_file \
    "$project_root/celestina.desktop" \
    "$prefix/share/applications/celestina.desktop" 0644

echo ">> Celestina bundle deployed to $prefix; the session was not activated"
