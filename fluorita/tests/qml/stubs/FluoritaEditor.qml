import QtQuick

// A stand-in for the Rust `FluoritaEditor`: the same properties and
// invokables, a picture that "opens" at once as the 32 × 24 fixture, and a
// save that lands on the next turn of the event loop the way the worker's
// answer arrives through the queue. It records what it is asked; the files a
// real save writes, trashes and adopts are proved by the crate's own tests.
QtObject {
    id: editor

    property bool open: false
    property bool video: false
    property string key: ""
    property int canvasWidth: 0
    property int canvasHeight: 0
    property bool canUndo: false
    property bool canRedo: false
    property bool edited: false
    property bool lossless: false
    property string classLabel: ""
    property string containerNotice: ""
    property bool saving: false
    property string notice: ""
    property int baseWidth: 0
    property int baseHeight: 0
    property string sourceUrl: ""
    property string previewSource: ""
    property int previewQuarters: 0
    property bool previewMirrored: false
    property int selected: 0
    property int revision: 0
    property var objectIds: []
    property var objectKinds: []
    property var objectGeometry: []
    property var objectInks: []
    property var objectWidths: []
    property var objectDetails: []
    property string savedKey: ""
    property string savedUrl: ""

    // Set by a test: the next save fails, as the worker's answer would,
    // leaving the editor open with its edit and a notice.
    property bool failNextSave: false

    // What the tests read back.
    property var opened: []
    property var saves: []
    property int closes: 0

    function nameOf(key) {
        return decodeURIComponent(key.substring(key.lastIndexOf("/") + 1))
    }

    function admits(key) {
        return true
    }

    function openItem(key) {
        editor.opened = editor.opened.concat([key])
        editor.savedKey = ""
        editor.savedUrl = ""
        editor.key = key
        // A film opens for the trim, as the opener's measurement decides.
        if (/\.(mp4|mkv|webm|mov)$/i.test(key)) {
            editor.open = false
            editor.video = true
            return
        }
        editor.video = false
        editor.sourceUrl = "file://" + key
        editor.baseWidth = 32
        editor.baseHeight = 24
        editor.canvasWidth = 32
        editor.canvasHeight = 24
        editor.previewSource = "0,0,32,24"
        editor.edited = false
        editor.notice = ""
        editor.open = true
        editor.revision += 1
    }

    function close() {
        editor.closes += 1
        editor.open = false
        editor.video = false
        editor.key = ""
        editor.sourceUrl = ""
        editor.edited = false
        editor.canUndo = false
        editor.canvasWidth = 0
        editor.canvasHeight = 0
        editor.saving = false
        editor.revision += 1
    }

    function rotate(clockwise) {
        const width = editor.canvasWidth
        editor.canvasWidth = editor.canvasHeight
        editor.canvasHeight = width
        editor.previewQuarters = (editor.previewQuarters + (clockwise ? 1 : 3)) % 4
        editor.edited = true
        editor.canUndo = true
        editor.revision += 1
    }

    function flip(horizontal) { editor.edited = true }
    function crop(area) { editor.edited = true }
    function resize(width, height) { editor.edited = true }
    function addText(area, size, ink, backdrop, text) { editor.edited = true }
    function addStroke(points, width, ink) { editor.edited = true }
    function addLine(ends, width, ink, arrow) { editor.edited = true }
    function addShape(ellipse, area, stroke, ink, fill) { editor.edited = true }
    function addHighlight(area, ink) { editor.edited = true }
    function addRedaction(area) { editor.edited = true }
    function selectObject(id) { editor.selected = id }
    function selectNextObject(forward) {}
    function moveObject(id, dx, dy) {}
    function resizeObject(id, size) {}
    function removeObject(id) {}
    function undo() {}
    function redo() {}

    // What `save(replace)` lands as: the copy beside, or the original's name.
    function landedKey(replace) {
        if (replace)
            return editor.key
        const dot = editor.key.lastIndexOf(".")
        return editor.key.substring(0, dot) + " (editado)" + editor.key.substring(dot)
    }

    function save(replace) {
        if (editor.saving || !editor.open)
            return
        editor.saves = editor.saves.concat([replace])
        editor.saving = true
        landing.replace = replace
        landing.start()
    }

    property Timer landing: Timer {
        property bool replace: false

        interval: 0
        onTriggered: {
            if (editor.failNextSave) {
                editor.failNextSave = false
                editor.saving = false
                editor.notice = "failed"
                return
            }
            const written = editor.landedKey(replace)
            editor.saving = false
            editor.notice = "saved"
            editor.close()
            editor.savedKey = written
            editor.savedUrl = "file://" + encodeURI(written)
        }
    }
}
