pragma Singleton

import QtQuick

// Closed icon catalogue for the suite. Callers keep semantic/freedesktop
// names, including values already persisted by Siderita, while this singleton
// resolves every one of them to a vendored Lucide SVG. No desktop icon theme
// participates in rendering, so changing the host theme cannot restyle an app.
QtObject {
    id: catalog

    readonly property string iconRoot:
            Qt.resolvedUrl(".").toString().startsWith("file:")
            ? Qt.resolvedUrl("icons/").toString()
            : "qrc:/qt/qml/CelestinaStyle/icons/"

    // Both tables are built without a prototype, so a subscript answers only
    // with a catalogue entry. The names that reach `resolve` are not this
    // module's: a consumer supplies them and, through
    // ~/.config/siderita/icons.conf, they ultimately come from a hand-editable
    // file. On an ordinary object literal `"toString"` and `"constructor"`
    // resolve up the prototype chain to inherited functions rather than to
    // `undefined`, which is a lookup succeeding against something that is not
    // an icon. The guard belongs on the table, not on each reader of it.
    readonly property var available: Object.assign(Object.create(null), {
        "app-window": true,
        "arrow-down": true,
        "arrow-right": true,
        "battery-charging": true,
        "bell": true,
        "bell-off": true,
        "binary": true,
        "bluetooth": true,
        "bookmark-plus": true,
        "check": true,
        "chevron-down": true,
        "chevron-right": true,
        "circle-alert": true,
        "clipboard-paste": true,
        "clock-arrow-up": true,
        "cloud": true,
        "copy": true,
        "cpu": true,
        "eraser": true,
        "eye": true,
        "eye-off": true,
        "file": true,
        "file-archive": true,
        "file-braces": true,
        "file-code": true,
        "file-image": true,
        "file-music": true,
        "file-plus": true,
        "file-text": true,
        "file-video-camera": true,
        "files": true,
        "film": true,
        "folder": true,
        "folder-code": true,
        "folder-down": true,
        "folder-git-2": true,
        "folder-heart": true,
        "folder-open": true,
        "folder-plus": true,
        "folder-sync": true,
        "gamepad-2": true,
        "gauge": true,
        "go-home": true,
        "go-next": true,
        "go-previous": true,
        "go-up": true,
        "hard-drive": true,
        "image": true,
        "info": true,
        "key": true,
        "layout-template": true,
        "leaf": true,
        "list-x": true,
        "mail": true,
        "media-pause": true,
        "media-play": true,
        "media-skip-back": true,
        "media-skip-forward": true,
        "media-volume": true,
        "media-volume-muted": true,
        "memory-stick": true,
        "mic": true,
        "mic-off": true,
        "monitor": true,
        "music": true,
        "paintbrush": true,
        "pencil": true,
        "phone": true,
        "pin": true,
        "plus": true,
        "power": true,
        "printer": true,
        "rotate-ccw": true,
        "scissors": true,
        "search": true,
        "settings": true,
        "share-2": true,
        "star": true,
        "star-outline": true,
        "sun": true,
        "symlink": true,
        "system-tray": true,
        "terminal": true,
        "toolbox": true,
        "type": true,
        "unplug": true,
        "user-trash": true,
        "view-details": true,
        "view-grid": true,
        "view-list": true,
        "view-refresh": true,
        "view-sort-ascending": true,
        "view-sort-descending": true,
        "wifi": true,
        "x": true,
        "zap": true
    })

    // Compatibility aliases deliberately retain the old public identifiers.
    // In particular, values stored in ~/.config/siderita/icons.conf continue
    // to resolve without an eager config migration.
    readonly property var aliases: Object.assign(Object.create(null), {
        "arrow-left": "go-previous",
        "audio-x-generic": "file-music",
        "application-json": "file-braces",
        "application-pdf": "file-text",
        "application-x-archive": "file-archive",
        "application-x-desktop": "app-window",
        "application-x-executable": "binary",
        "bookmark-new": "bookmark-plus",
        "dialog-error": "circle-alert",
        "document-new": "file-plus",
        "document-open-recent": "clock-arrow-up",
        "document-properties": "info",
        "drive-removable-media": "hard-drive",
        "edit-clear": "eraser",
        "edit-copy": "copy",
        "edit-cut": "scissors",
        "edit-delete": "user-trash",
        "edit-find": "search",
        "edit-paste": "clipboard-paste",
        "edit-rename": "pencil",
        "edit-undo": "rotate-ccw",
        "emblem-symbolic-link": "symlink",
        "emblem-synchronizing": "view-refresh",
        "file-symlink": "symlink",
        "folder-cloud": "cloud",
        "folder-desktop": "monitor",
        "folder-development": "folder-code",
        "folder-documents": "files",
        "folder-download": "folder-down",
        "folder-favorites": "folder-heart",
        "folder-games": "gamepad-2",
        "folder-git": "folder-git-2",
        "folder-github": "folder-git-2",
        "folder-image": "image",
        "folder-important": "folder-heart",
        "folder-mail": "mail",
        "folder-new": "folder-plus",
        "folder-music": "music",
        "folder-pictures": "image",
        "folder-print": "printer",
        "folder-publicshare": "share-2",
        "folder-script": "folder-code",
        "folder-templates": "layout-template",
        "folder-text": "file-text",
        "folder-video": "file-video-camera",
        "folder-videos": "film",
        "font-x-generic": "type",
        "go-down": "arrow-down",
        "image-x-generic": "file-image",
        "list-remove": "list-x",
        "media-eject": "unplug",
        "media-playback-pause": "media-pause",
        "media-playback-start": "media-play",
        "media-skip-backward": "media-skip-back",
        "phone": "phone",
        "preferences-desktop-icons": "paintbrush",
        "preferences-system": "settings",
        "system-run": "media-play",
        "tab-new": "plus",
        "text-html": "file-code",
        "text-x-generic": "file",
        "text-x-python": "file-code",
        "text-x-script": "file-code",
        "user-desktop": "monitor",
        "user-home": "go-home",
        "utilities-terminal": "terminal",
        "video-x-generic": "file-video-camera"
    })

    function resolve(name, fallbackName) {
        const requested = name || ""
        const fallback = fallbackName || ""
        const candidate = aliases[requested] || requested
        if (candidate.length > 0 && available[candidate] === true)
            return candidate

        const fallbackCandidate = aliases[fallback] || fallback
        if (fallbackCandidate.length > 0
                && available[fallbackCandidate] === true)
            return fallbackCandidate

        if (requested.startsWith("folder") || fallback === "folder")
            return "folder"
        if (requested.length > 0 || fallback.length > 0)
            return "file"
        return ""
    }

    function source(name, fallbackName) {
        const resolved = resolve(name, fallbackName)
        return resolved.length > 0 ? iconRoot + resolved + ".svg" : ""
    }

    function keyFromSource(sourceUrl) {
        const raw = sourceUrl ? sourceUrl.toString() : ""
        if (raw.length === 0)
            return ""
        const slash = raw.lastIndexOf("/")
        const fileName = slash >= 0 ? raw.substring(slash + 1) : raw
        return fileName.endsWith(".svg")
                ? fileName.substring(0, fileName.length - 4) : fileName
    }
}
