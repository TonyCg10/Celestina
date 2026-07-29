#!/usr/bin/env bash

set -euo pipefail

readonly lucide_version="1.27.0"
readonly lucide_base="https://raw.githubusercontent.com/lucide-icons/lucide/${lucide_version}/icons"

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
icon_dir=$(cd -- "$script_dir/../icons" && pwd)
work_dir=$(mktemp -d /tmp/celestina-lucide-XXXXXX)
trap 'rm -rf -- "$work_dir"' EXIT

# Every shipped filename is reproducibly tied to an upstream Lucide glyph.
# Compatibility filenames stay stable for QML and persisted Siderita settings;
# the value after ':' is the canonical filename in the pinned Lucide tag.
icons=(
    app-window:app-window arrow-down:arrow-down arrow-right:arrow-right
    battery-charging:battery-charging binary:binary bookmark-plus:bookmark-plus
    check:check chevron-down:chevron-down chevron-right:chevron-right
    circle-alert:circle-alert clipboard-paste:clipboard-paste
    clock-arrow-up:clock-arrow-up cloud:cloud copy:copy eraser:eraser file:file
    file-archive:file-archive file-braces:file-braces file-code:file-code
    file-image:file-image file-music:file-music file-plus:file-plus
    file-text:file-text file-video-camera:file-video-camera files:files film:film
    folder:folder folder-code:folder-code folder-down:folder-down
    folder-git-2:folder-git-2 folder-heart:folder-heart folder-open:folder-open
    folder-plus:folder-plus folder-sync:folder-sync gamepad-2:gamepad-2
    go-home:house go-next:arrow-right go-previous:arrow-left go-up:arrow-up
    hard-drive:hard-drive image:image info:info key:key
    layout-template:layout-template list-x:list-x mail:mail
    media-pause:pause media-play:play media-skip-back:skip-back
    media-skip-forward:skip-forward monitor:monitor music:music
    paintbrush:paintbrush pencil:pencil phone:smartphone plus:plus printer:printer
    rotate-ccw:rotate-ccw scissors:scissors search:search settings:sliders-horizontal
    share-2:share-2 star:star star-outline:star symlink:link-2 terminal:terminal
    type:type unplug:unplug user-trash:trash-2 view-details:table-properties
    view-grid:layout-grid view-list:list view-refresh:refresh-cw
    view-sort-ascending:arrow-up-narrow-wide
    view-sort-descending:arrow-down-wide-narrow x:x
)

for mapping in "${icons[@]}"; do
    destination=${mapping%%:*}
    upstream=${mapping#*:}
    source_file="$work_dir/$destination.svg"
    curl --fail --silent --show-error --location \
        "$lucide_base/$upstream.svg" --output "$source_file"
    # Qt's IconImage applies semantic colour reliably to a white source.
    sed -i 's/currentColor/#fff/g' "$source_file"
    # Favorites use the official star geometry as a filled semantic variant.
    if [[ $destination == star ]]; then
        sed -i 's/fill="none"/fill="#fff"/' "$source_file"
    fi
    install -m 0644 "$source_file" "$icon_dir/$destination.svg"
done

printf 'Lucide %s: %d iconos sincronizados.\n' \
    "$lucide_version" "${#icons[@]}"
