pragma Singleton

import QtQuick

// Which XDG user directory gets its own folder-type glyph instead of the
// generic one — shared so the main folder view and the picker's read-only
// grid draw the exact same icon for "this is Documents/Downloads/…".  Each
// window still resolves its own `controller.placePath(key)` to build its
// path→icon map; only the key→icon-name table lives here.
QtObject {
    readonly property var defs: ({
        DESKTOP: "folder-desktop",
        DOCUMENTS: "folder-documents",
        DOWNLOAD: "folder-download",
        MUSIC: "folder-music",
        PICTURES: "folder-pictures",
        VIDEOS: "folder-videos",
        PUBLICSHARE: "folder-publicshare",
        TEMPLATES: "folder-templates"
    })
}
