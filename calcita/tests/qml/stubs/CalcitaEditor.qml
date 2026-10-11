import QtQuick

// Stands in for the Rust `CalcitaEditor` under qmltestrunner: the same
// properties, signals and invokables over a model kept in JavaScript, with
// undo and redo as snapshots. Coordinates are taken as the displayed
// page's points and published back unchanged (the conversion is Rust's).
// The form's widgets (`fields`) are a JSON array the test seeds
// (`seedFields`); a fill names its field by id and sets the value of every
// widget of it; text outside Latin-1 is refused with a notice, as the Rust
// object refuses text outside WinAnsi.
// What it was asked is recorded for the tests (`calls`, `saves`);
// `saveLands` decides whether a save reports `saved` at once.
QtObject {
    id: editor

    property string documentKey: ""
    property bool editing: false
    property bool preparing: false
    property bool saving: false
    property bool edited: false
    property bool canUndo: false
    property bool canRedo: false
    property int revision: 0
    property string annotations: "[]"
    property string fields: "[]"
    readonly property real textPadding: 2
    readonly property real lineHeight: 1.2

    property var items: []
    property var widgets: []
    property var past: []
    property var future: []
    property int nextId: 1
    property var calls: []
    property var saves: []
    property bool saveLands: true
    // What `begin` finds in the document.
    property var seed: []
    property var seedFields: []

    signal notice(string kind, string text)
    signal saved(string key, bool replaced)

    function record(name) {
        calls = calls.concat([name])
    }

    function snapshot() {
        return JSON.stringify({ "items": items, "widgets": widgets })
    }

    function restore(text) {
        const state = JSON.parse(text)
        items = state.items
        widgets = state.widgets
    }

    function publish() {
        annotations = JSON.stringify(items)
        fields = JSON.stringify(widgets)
        edited = past.length > 0
        canUndo = past.length > 0
        canRedo = future.length > 0
    }

    function change(next) {
        past = past.concat([snapshot()])
        future = []
        items = next
        publish()
    }

    function changeFields(next) {
        past = past.concat([snapshot()])
        future = []
        widgets = next
        publish()
    }

    function begin() {
        record("begin")
        if (editing)
            return
        items = JSON.parse(JSON.stringify(seed))
        widgets = JSON.parse(JSON.stringify(seedFields))
        past = []
        future = []
        revision = 7
        // Published before `editing` turns, as the Rust object does.
        publish()
        editing = true
    }

    function finish() {
        record("finish")
        items = []
        widgets = []
        past = []
        future = []
        revision = 0
        editing = false
        publish()
    }

    function add(entry) {
        const id = "n" + nextId
        nextId += 1
        entry.id = id
        change(items.concat([entry]))
        return id
    }

    function addText(page, at, width, text, color, size) {
        record("addText")
        return add({ "kind": "text", "page": page, "x": at.x, "y": at.y, "w": width, "h": size * 1.2 + 4,
                     "color": color, "text": text, "size": size, "rich": false, "laidOut": true })
    }

    // QtPdf's outlines arrive as polygons JavaScript cannot read; the
    // stand-in records how many and places a fixed highlight.
    function addHighlight(page, outlines, color) {
        record("addHighlight:" + outlines.length)
        if (outlines.length === 0)
            return ""
        return add({ "kind": "highlight", "page": page, "x": 72, "y": 62, "w": 100, "h": 26,
                     "color": color, "shape": [[72, 62, 172, 62, 172, 88, 72, 88]] })
    }

    function addInk(page, points, color, width) {
        record("addInk")
        const flat = points.split(" ").map(pair => pair.split(",").map(Number))
        const xs = flat.map(p => p[0])
        const ys = flat.map(p => p[1])
        const left = Math.min(...xs)
        const top = Math.min(...ys)
        return add({ "kind": "ink", "page": page, "x": left, "y": top, "w": Math.max(...xs) - left,
                     "h": Math.max(...ys) - top, "color": color, "size": width,
                     "shape": [flat.reduce((all, p) => all.concat(p), [])] })
    }

    function edit(id, update) {
        if (!items.some(entry => entry.id === id))
            return false
        change(items.map(entry => entry.id === id ? update(Object.assign({}, entry)) : entry))
        return true
    }

    function moveTo(id, x, y, width, height) {
        record("moveTo:" + id + ":" + Math.round(x) + "," + Math.round(y) + "," + Math.round(width))
        return edit(id, entry => Object.assign(entry, { "x": x, "y": y, "w": width, "h": height }))
    }

    function recolor(id, color) {
        record("recolor:" + id)
        return edit(id, entry => Object.assign(entry, { "color": color }))
    }

    function setText(id, text) {
        record("setText:" + id + ":" + text)
        return edit(id, entry => Object.assign(entry, { "text": text }))
    }

    function remove(id) {
        record("remove:" + id)
        if (!items.some(entry => entry.id === id))
            return false
        change(items.filter(entry => entry.id !== id))
        return true
    }

    // A field's value, on each of its widgets; false for a field that is
    // not there, or not of that kind.
    function fill(field, kind, update) {
        if (!widgets.some(entry => entry.field === field && entry.kind === kind))
            return false
        changeFields(widgets.map(entry => entry.field === field ? update(Object.assign({}, entry)) : entry))
        return true
    }

    function fillText(field, text) {
        record("fillText:" + field + ":" + text)
        if (/[^\u0000-\u00ff]/.test(text)) {
            notice("error", "unencodable")
            return false
        }
        return fill(field, "text", entry => Object.assign(entry, { "value": text }))
    }

    function setChecked(field, checked) {
        record("setChecked:" + field + ":" + checked)
        return fill(field, "checkbox", entry => Object.assign(entry, { "checked": checked }))
    }

    function undo() {
        if (past.length === 0)
            return
        future = future.concat([snapshot()])
        restore(past[past.length - 1])
        past = past.slice(0, past.length - 1)
        publish()
    }

    function redo() {
        if (future.length === 0)
            return
        past = past.concat([snapshot()])
        restore(future[future.length - 1])
        future = future.slice(0, future.length - 1)
        publish()
    }

    function save(replace) {
        saves = saves.concat([replace])
        if (!saveLands)
            return
        const key = replace ? documentKey : documentKey.replace(/\.pdf$/, " (editado).pdf")
        finish()
        saved(key, replace)
    }
}
