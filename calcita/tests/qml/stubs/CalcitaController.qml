// language-contract: allow-non-english (the Spanish notices the window shows, compared verbatim)
pragma Singleton
import QtQuick

// Stands in for the Rust `CalcitaController` singleton under qmltestrunner,
// where the binary's types do not exist: the same properties, signals and
// invokables. Admission is by name, as in Rust (`.pdf`, `file://` only); keys
// are pathkeys, which for the fixtures' plain paths are the paths. `reset`
// exists only here, for the tests.
QtObject {
    id: controller

    property bool appearanceReducedMotion: false
    property real appearanceTextScale: 1.0
    readonly property bool smokeReport: false
    property var documents: []
    property var recents: []
    property var recentNames: []
    property var remembered: ({})
    // What `copySelection` and `openExternal` were given, for the tests.
    property string clipboard: ""
    property var openedLinks: []

    signal openDocument(string key)
    signal raiseDocument(string key)
    signal notice(string kind, string text, string origin)

    function reset() {
        documents = []
        recents = []
        recentNames = []
        remembered = ({})
        clipboard = ""
        openedLinks = []
    }

    function admit(key) {
        if (documents.indexOf(key) >= 0) {
            raiseDocument(key)
            return
        }
        documents = documents.concat([key])
        openDocument(key)
    }

    function openPath(key, origin) {
        if (key.toLowerCase().endsWith(".pdf"))
            admit(key)
        else
            notice("error", "Calcita solo abre documentos PDF.", origin)
    }

    function openDropped(uris, origin) {
        uris.forEach(uri => {
            if (!uri.startsWith("file:///"))
                notice("error", "Solo se pueden abrir archivos de este equipo.", origin)
            else
                openPath(uri.substring("file://".length), origin)
        })
    }

    function closeDocument(key) {
        documents = documents.filter(open => open !== key)
    }

    function remember(key, page, zoom) {
        const next = Object.assign({}, remembered)
        next[key] = { "page": page, "zoom": zoom }
        remembered = next
        recents = [key].concat(recents.filter(known => known !== key))
        recentNames = recents.map(known => known.substring(known.lastIndexOf("/") + 1))
    }

    function restoredPage(key) {
        return remembered[key] !== undefined ? remembered[key].page : 1
    }

    function restoredZoom(key) {
        return remembered[key] !== undefined ? remembered[key].zoom : "width"
    }

    function copySelection(text) {
        if (text.length > 0)
            clipboard = text
    }

    function openExternal(url, origin) {
        if (/^(https?|mailto):/i.test(url))
            openedLinks = openedLinks.concat([url])
        else
            notice("error", "Calcita solo abre enlaces web y de correo.", origin)
    }
}
