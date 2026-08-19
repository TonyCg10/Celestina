pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

// ─── EntryIconRules ───────────────────────────────────────────────────────────
// What gets drawn for an entry: the family its name earns, the icon the person
// picked by hand, the tint, and the face some entries carry of their own. It
// lives outside the panel because these are rules with their own reason to
// change — and because the panel coordinates, it is not a catalogue.
//
// Delegates ask through `panel.icons`, so one place decides and none repeats
// it.
// ──────────────────────────────────────────────────────────────────────────────
QtObject {
    id: rules

    required property var controller
    required property var hostWindow

    // An entry's own face, when it carries one. Asked once per delegate:
    // the key does not change while the row lives.
    function entryOwnIcon(key) {
        return rules.controller ? rules.controller.ownIconUrl(key) : ""
    }
    function mediaKind(n) {
        if (/\.(png|jpe?g|gif|webp|bmp|ico|tiff?|avif|jxl|heic|heif)$/i.test(n))
            return "image"
        if (/\.(mp4|mkv|webm|mov|avi|m4v|mpe?g|wmv|flv|3gp|ogv|ts)$/i.test(n))
            return "video"
        if (/\.(mp3|flac|ogg|oga|opus|m4a|aac|wav|wma|aiff?|mka)$/i.test(n))
            return "audio"
        return ""
    }
    // Map each XDG user directory's PATH to its freedesktop folder-type
    // icon, so the user directories show their own glyph
    // in the content view, not the generic folder. Rebuilt on open; the
    // paths are user-level and stable.
    property var folderTypeIcons: ({})
    function rebuildFolderTypeIcons() {
        var defs = CelestinaFolderTypeIcons.defs
        var m = {}
        for (var k in defs) {
            var p = controller.placePath(k)
            if (p.length > 0)
                m[p] = defs[k]
        }
        folderTypeIcons = m
    }
    // The display name of a path, for the rules that read extensions.
    function nameOf(path) {
        const cut = path.lastIndexOf("/")
        return cut >= 0 ? path.substring(cut + 1) : path
    }

    function folderIcon(path) {
        return (path && folderTypeIcons[path]) ? folderTypeIcons[path] : "folder"
    }
    // User-chosen per-path appearance (shape + optional colour), folded
    // from one atomic `path\ticon\taccent` record. The two-column parser is
    // deliberately retained for configurations written by older builds.
    readonly property var customAppearances: {
        var m = {}
        var entries = controller.customIconEntries
        for (var i = 0; i < entries.length; i++) {
            var first = entries[i].indexOf("\t")
            if (first <= 0)
                continue
            var second = entries[i].indexOf("\t", first + 1)
            var path = entries[i].substring(0, first)
            m[path] = {
                "icon": second < 0
                        ? entries[i].substring(first + 1)
                        : entries[i].substring(first + 1, second),
                "accent": second < 0 ? "" : entries[i].substring(second + 1)
            }
        }
        return m
    }
    // Starred paths, folded into a set for O(1)
    // lookup from every delegate. Same shape as customIcons: a binding,
    // so a star appears the moment it is set.
    readonly property var favorites: {
        var s = {}
        var entries = controller.favoriteEntries
        for (var i = 0; i < entries.length; i++) {
            var cut = entries[i].indexOf("\t")
            s[cut > 0 ? entries[i].substring(0, cut) : entries[i]] = true
        }
        return s
    }
    function isFavorite(path) {
        return path.length > 0 && favorites[path] === true
    }
    function customIcon(path) {
        var appearance = path ? customAppearances[path] : undefined
        return appearance ? appearance.icon : ""
    }
    function customIconAccent(path) {
        var appearance = path ? customAppearances[path] : undefined
        return appearance ? appearance.accent : ""
    }
    // The tint an entry is drawn with: the accent a person chose for it if
    // there is one, and otherwise the accent its own kind earns — which is how
    // a `.py` and a `.go` are told apart while sharing one page. A chosen
    // accent always wins: it was chosen.
    function iconTint(path) {
        const chosen = rules.customIconAccent(path)
        if (chosen.length > 0)
            return CelestinaTheme.iconAccentColor(chosen)
        const earned = rules.controller
                       ? rules.controller.glyphAccentForName(rules.nameOf(path)) : ""
        return CelestinaTheme.iconAccentColor(earned)
    }
    function entryIconTone(kind) {
        return kind === "directory" ? CelestinaIcon.Folder
             : kind === "symlink" ? CelestinaIcon.Symlink
             : CelestinaIcon.File
    }

    // The Lucide icon a non-thumbnailed entry shows — a user override if
    // set, else a media-type icon (video/audio/image), a type-specific
    // folder, else generic.
    function mediaIconName(kind, media, path) {
        var custom = rules.customIcon(path)
        if (custom.length > 0)
            return custom
        if (kind === "directory")
            return rules.folderIcon(path)
        if (kind === "symlink")
            return "emblem-symbolic-link"
        // Media keeps its own three names, which the preview machinery also
        // speaks; everything else asks the controller, which reads the
        // extension and knows far more families than this view should.
        if (media === "image")
            return "image-x-generic"
        if (media === "video")
            return "video-x-generic"
        if (media === "audio")
            return "audio-x-generic"
        return rules.controller
               ? rules.controller.glyphForName(rules.nameOf(path))
               : "text-x-generic"
    }
}
