pragma Singleton

import QtQuick

// How to draw each sidebar place key (label and glyph). The controller owns
// which places exist and in what order; this is only their presentation, and
// it lives here — not duplicated per window — because both Main.qml's Sidebar
// and PickerWindow's read-only PickerSidebar draw the same place keys.
QtObject {
    readonly property var defs: ({
        "HOME":      { name: "Inicio",     icon: "user-home",        fallback: "go-home" },
        "DESKTOP":   { name: "Escritorio", icon: "user-desktop",     fallback: "folder" },
        "DOCUMENTS": { name: "Documentos", icon: "folder-documents", fallback: "folder" },
        "DOWNLOAD":  { name: "Descargas",  icon: "folder-download",  fallback: "folder" },
        "MUSIC":     { name: "Música",     icon: "folder-music",     fallback: "folder" },
        "PICTURES":  { name: "Imágenes",   icon: "folder-pictures",  fallback: "folder" },
        "VIDEOS":    { name: "Vídeos",     icon: "folder-videos",    fallback: "folder" },
        "RECENT":    { name: "Recientes",  icon: "document-open-recent", fallback: "file" },
        "TRASH":     { name: "Papelera",   icon: "user-trash",       fallback: "user-trash" }
    })
}
