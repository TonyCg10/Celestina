pragma Singleton
import QtQuick

// Stands in for the Rust `SelenitaController` singleton under qmltestrunner,
// where the binary's types do not exist: the same properties, signals and
// invokables, with the fakes on as `SELENITA_FAKE=1` sets them. A capture
// answers at once as the fake backend does: a history row whose thumbnail
// is a 1×1 PNG beside this file. `calls` records every invokable, and
// `reset()` puts everything back between tests.
QtObject {
    id: stub

    property bool appearanceReducedMotion: false
    property real appearanceTextScale: 1.0
    readonly property bool smokeReport: false
    readonly property bool fake: true
    readonly property bool launchCapture: false

    property string target: "screen"
    property int delay: 0
    property bool toClipboard: true
    property bool toFile: true
    property string screenOutput: ""
    property string fileStem: ""
    property string folderName: ""
    property var outputs: []
    property var history: []
    property bool busy: false

    property var calls: []
    property int taken: 0

    signal notice(string kind, string text)
    signal countdown(int seconds)
    signal captured(string entryId)
    signal hideWindowRequested()
    signal showWindowRequested()

    function reset() {
        target = "screen"
        delay = 0
        toClipboard = true
        toFile = true
        screenOutput = ""
        outputs = []
        history = []
        busy = false
        calls = []
        taken = 0
    }

    function record(name, argument) {
        calls = calls.concat([name + ":" + argument])
    }

    function capture(windowShown, accent, background) {
        record("capture", target + "," + delay + "," + toClipboard + "," + toFile
               + "," + windowShown + "," + accent.length + "," + background.length)
        if (!toFile)
            return
        taken += 1
        const id = "/fake/Captura " + taken + ".png"
        const row = {
            id: id,
            name: fileStem + " " + taken + ".png",
            url: Qt.resolvedUrl("pixel.png").toString(),
            kind: "screenshot",
            size: 70,
            takenAt: Date.UTC(2026, 9, 9, 14, 32, 5)
        }
        history = [row].concat(history)
        captured(id)
    }

    function openInFluorita(id) { record("openInFluorita", id) }
    function copy(id) { record("copy", id) }
    function showInSiderita(id) { record("showInSiderita", id) }
    function deleteEntry(id) {
        record("deleteEntry", id)
        history = history.filter(row => row.id !== id)
    }
}
