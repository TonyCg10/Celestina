import QtQuick

// A stand-in for the Rust `FluoritaTrim`: the same properties and
// invokables, the domain's span rule in a few lines, and a save that lands
// on the next turn of the event loop the way the worker's answer arrives
// through the queue. What a real trim writes is proved by the crate's tests.
QtObject {
    id: trim

    property string key: ""
    property real lengthSeconds: 0
    property real trimStart: 0
    property real trimEnd: 0
    property real minimumSeconds: 1 / 30
    property bool edited: false
    property bool saving: false
    property real progress: 0
    property string notice: ""
    property string containerNotice: ""
    property string savedKey: ""
    property string savedUrl: ""

    // Set by a test: the next save stays in flight until it is cancelled.
    property bool holdNextSave: false

    // What the tests read back.
    property var opened: []
    property var saves: []
    property int cancels: 0

    function open(key) {
        trim.close()
        trim.opened = trim.opened.concat([key])
        trim.key = key
        // The container by name, as the Rust trim says it before probing.
        trim.containerNotice = /\.mp4$/i.test(key) ? "" : "container changes"
    }

    function setLength(seconds) {
        if (trim.saving || !(seconds > 0))
            return
        const first = trim.lengthSeconds <= 0
        trim.lengthSeconds = seconds
        trim.setSpan(first ? 0 : trim.trimStart, first ? seconds : trim.trimEnd)
    }

    function setSpan(start, end) {
        if (trim.saving)
            return
        trim.trimStart = Math.max(0, Math.min(start, trim.lengthSeconds))
        trim.trimEnd = Math.max(0, Math.min(end, trim.lengthSeconds))
        const valid = trim.trimEnd - trim.trimStart >= trim.minimumSeconds
        const whole = trim.trimStart === 0 && trim.trimEnd >= trim.lengthSeconds
        trim.edited = valid && !whole
    }

    function saveTrim(replace) {
        if (trim.saving)
            return
        trim.saves = trim.saves.concat([replace])
        trim.saving = true
        trim.progress = 0
        trim.notice = ""
        if (trim.holdNextSave) {
            trim.holdNextSave = false
            trim.progress = 0.5
            return
        }
        landing.replace = replace
        landing.start()
    }

    function cancel() {
        trim.cancels += 1
        if (!trim.saving)
            return
        trim.saving = false
        trim.progress = 0
        trim.notice = "cancelled"
    }

    function close() {
        trim.key = ""
        trim.lengthSeconds = 0
        trim.trimStart = 0
        trim.trimEnd = 0
        trim.edited = false
        trim.saving = false
        trim.progress = 0
        trim.notice = ""
        trim.containerNotice = ""
        trim.savedKey = ""
        trim.savedUrl = ""
    }

    property Timer landing: Timer {
        property bool replace: false

        interval: 0
        onTriggered: {
            const dot = trim.key.lastIndexOf(".")
            const written = replace
                ? trim.key
                : trim.key.substring(0, dot) + " (editado)" + trim.key.substring(dot)
            trim.saving = false
            trim.progress = 1
            trim.notice = "saved"
            trim.savedKey = written
            trim.savedUrl = "file://" + encodeURI(written)
        }
    }
}
