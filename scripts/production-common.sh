#!/bin/sh

# Shared, source-only helpers for the per-project production entry points.
# This file never builds.  Deploy callers must validate the manifest first.

production_require_verified() {
    production_suite_root=$1
    production_project_id=$2
    python3 "$production_suite_root/scripts/production_artifact.py" \
        check "$production_project_id" --require-verified
}

production_install_file() (
    production_source=$1
    production_destination=$2
    production_mode=$3
    production_parent=$(dirname -- "$production_destination")
    mkdir -p "$production_parent" || exit 1
    production_temporary=
    production_cleanup_file() {
        if [ -n "$production_temporary" ]; then
            rm -f "$production_temporary"
        fi
    }
    trap production_cleanup_file EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM
    production_temporary=$(mktemp "$production_parent/.production-install.XXXXXX") || exit 1
    install -m "$production_mode" "$production_source" "$production_temporary" || exit 1
    mv -f "$production_temporary" "$production_destination" || exit 1
    production_temporary=
)

production_install_template() (
    production_template=$1
    production_destination=$2
    production_placeholder=$3
    production_replacement=$4
    production_parent=$(dirname -- "$production_destination")
    mkdir -p "$production_parent" || exit 1
    production_temporary=
    production_cleanup_template() {
        if [ -n "$production_temporary" ]; then
            rm -f "$production_temporary"
        fi
    }
    trap production_cleanup_template EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM
    production_temporary=$(mktemp "$production_parent/.production-template.XXXXXX") || exit 1
    production_escaped_replacement=$(
        printf '%s' "$production_replacement" | sed 's/[\\&|]/\\&/g'
    ) || exit 1
    sed "s|$production_placeholder|$production_escaped_replacement|g" \
        "$production_template" > "$production_temporary" || exit 1
    chmod 0644 "$production_temporary" || exit 1
    mv -f "$production_temporary" "$production_destination" || exit 1
    production_temporary=
)

production_install_tree() (
    production_source=$1
    production_destination=$2
    production_parent=$(dirname -- "$production_destination")
    mkdir -p "$production_parent" || exit 1
    production_temporary=
    production_backup=
    production_cleanup_tree() {
        if [ -n "$production_temporary" ]; then
            rm -rf "$production_temporary"
        fi
        if [ -n "$production_backup" ] && \
            { [ -e "$production_backup" ] || [ -L "$production_backup" ]; }; then
            if [ ! -e "$production_destination" ] && [ ! -L "$production_destination" ]; then
                mv "$production_backup" "$production_destination" || true
            else
                rm -rf "$production_backup"
            fi
        fi
    }
    trap production_cleanup_tree EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM
    production_temporary=$(mktemp -d "$production_parent/.production-tree.XXXXXX") || exit 1
    cp -a "$production_source/." "$production_temporary/" || exit 1

    if [ -e "$production_destination" ] || [ -L "$production_destination" ]; then
        production_backup=$(mktemp -d "$production_parent/.production-backup.XXXXXX") || exit 1
        rmdir "$production_backup" || exit 1
        mv "$production_destination" "$production_backup" || exit 1
    fi
    mv "$production_temporary" "$production_destination" || exit 1
    production_temporary=
    if [ -n "$production_backup" ]; then
        rm -rf "$production_backup" || exit 1
        production_backup=
    fi
)

production_install_xdg_application() (
    production_binary=$1
    production_binary_name=$2
    production_app_id=$3
    production_desktop=$4
    production_icon=$5
    production_prefix=$6

    if ! command -v rsvg-convert >/dev/null 2>&1; then
        echo "deploy-production: rsvg-convert (librsvg) is required" >&2
        return 1
    fi

    production_bin_dir=$production_prefix/bin
    production_apps_dir=$production_prefix/share/applications
    production_icons_dir=$production_prefix/share/icons/hicolor
    production_png_tmp=
    production_cleanup_icon() {
        if [ -n "$production_png_tmp" ]; then
            rm -f "$production_png_tmp"
        fi
    }
    trap production_cleanup_icon EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    production_install_file \
        "$production_binary" "$production_bin_dir/$production_binary_name" 0755 || exit 1
    production_install_file \
        "$production_desktop" "$production_apps_dir/$production_app_id.desktop" 0644 || exit 1
    production_install_file \
        "$production_icon" "$production_icons_dir/scalable/apps/$production_app_id.svg" 0644 || exit 1

    for production_size in 16 22 24 32 48 64 128 256 512; do
        production_png_dir=$production_icons_dir/${production_size}x${production_size}/apps
        mkdir -p "$production_png_dir" || exit 1
        production_png_tmp=$(mktemp "$production_png_dir/.production-icon.XXXXXX.png") || exit 1
        if ! rsvg-convert -w "$production_size" -h "$production_size" \
            "$production_icon" -o "$production_png_tmp"; then
            rm -f "$production_png_tmp"
            exit 1
        fi
        chmod 0644 "$production_png_tmp" || exit 1
        mv -f "$production_png_tmp" "$production_png_dir/$production_app_id.png" || exit 1
        production_png_tmp=
    done

    update-desktop-database "$production_apps_dir" >/dev/null 2>&1 || true
    gtk-update-icon-cache -f -t "$production_icons_dir" >/dev/null 2>&1 || true
)

production_status() {
    production_suite_root=$1
    production_project_id=$2
    shift 2
    python3 "$production_suite_root/scripts/production_artifact.py" \
        status "$production_project_id" "$@"
}
