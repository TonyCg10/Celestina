pragma Singleton
import QtQuick

// Stands in for the Rust `SelenitaController` singleton under qmltestrunner,
// where the binary's types do not exist: the same properties, signals and
// invokables, with the fakes on as `SELENITA_FAKE=1` sets them. A capture
// answers at once as the fake backend does: a history row whose thumbnail
// is a 1×1 PNG beside this file. A recording toggles between `idle` and
// `recording` at once, as the fake recorder does, and its stop lands a
// recording row. With `previewOnResult`, a capture saved to a file and a
// finished recording show the corner preview, as the capture worker's
// preview report does: the picture itself, or for a recording its poster
// (the same PNG) and its duration. It is off unless a test asks: the
// offscreen platform activates every window it shows, so the preview would
// take the keyboard focus from the main window, which on the session niri's
// rule (`open-focused false`) prevents. `calls` records every invokable,
// and `reset()` puts everything back between tests.
QtObject {
    id: stub

    property bool appearanceReducedMotion: false
    property real appearanceTextScale: 1.0
    readonly property bool smokeReport: false
    readonly property bool smokeClose: false
    readonly property bool fake: true
    // Writable here so a test can build the window as a key-binding launch.
    property bool launchCapture: false

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

    property string recordingState: "idle"
    property bool withAudio: false
    property real recordingStartedAt: 0
    property string lastRecordingId: ""
    property string lastRecordingName: ""
    property string recorderMissing: ""
    property string recordingStem: ""

    property bool previewOnResult: false
    property bool previewVisible: false
    property string previewKey: ""
    property string previewKind: ""
    property url previewSource: ""
    property string previewDuration: ""
    property string previewFileUri: ""
    property real previewAspect: 0
    property string previewNotice: ""

    property var calls: []
    property int taken: 0

    signal notice(string kind, string text)
    signal countdown(int seconds)
    signal captured(string entryId)
    signal showWindowRequested()
    signal recordingFinished(string entryId)
    signal previewShown()
    signal launchFinished()

    function reset() {
        target = "screen"
        delay = 0
        toClipboard = true
        toFile = true
        screenOutput = ""
        outputs = []
        history = []
        busy = false
        recordingState = "idle"
        withAudio = false
        recordingStartedAt = 0
        lastRecordingId = ""
        lastRecordingName = ""
        recorderMissing = ""
        previewVisible = false
        previewKey = ""
        previewKind = ""
        previewSource = ""
        previewDuration = ""
        previewFileUri = ""
        previewAspect = 0
        previewNotice = ""
        launchCapture = false
        previewOnResult = false
        calls = []
        taken = 0
    }

    // What the controller does on the worker's preview report.
    function showPreview(key, kind, source, duration, fileUri, aspect) {
        previewKey = key
        previewKind = kind
        previewSource = source
        previewDuration = duration
        previewFileUri = fileUri
        previewAspect = aspect
        previewNotice = ""
        previewVisible = true
        previewShown()
    }

    function dismissPreview() {
        record("dismissPreview", previewKey)
        previewVisible = false
    }

    function editPreview() {
        record("editPreview", previewKey)
        previewVisible = false
    }

    function record(name, argument) {
        calls = calls.concat([name + ":" + argument])
    }

    function capture(accent, background) {
        record("capture", target + "," + delay + "," + toClipboard + "," + toFile
               + "," + accent.length + "," + background.length)
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
        if (previewOnResult)
            showPreview(id, "screenshot", row.url, "", "file://" + encodeURI(id), 1)
    }

    function toggleRecording() {
        record("toggleRecording", withAudio)
        if (recordingState === "recording") {
            finishRecording()
            return
        }
        recordingStartedAt = Date.now()
        recordingState = "recording"
    }

    function stopRecording() {
        record("stopRecording", recordingState)
        if (recordingState === "recording")
            finishRecording()
    }

    function finishRecording() {
        taken += 1
        const id = "/fake/" + recordingStem + " " + taken + ".mp4"
        const row = {
            id: id,
            name: recordingStem + " " + taken + ".mp4",
            url: "file://" + id,
            kind: "recording",
            size: 17,
            takenAt: recordingStartedAt
        }
        history = [row].concat(history)
        recordingState = "idle"
        lastRecordingId = id
        lastRecordingName = row.name
        recordingFinished(id)
        if (previewOnResult)
            showPreview(id, "recording", Qt.resolvedUrl("pixel.png"), "0:01", row.url, 0.5625)
    }

    function adopt(key) { record("adopt", key) }
    function openInFluorita(id) { record("openInFluorita", id) }
    function copy(id) { record("copy", id) }
    function showInSiderita(id) { record("showInSiderita", id) }
    // As the controller does: the last recording's line follows its row.
    function deleteEntry(id) {
        record("deleteEntry", id)
        history = history.filter(row => row.id !== id)
        if (lastRecordingId === id) {
            lastRecordingId = ""
            lastRecordingName = ""
        }
        if (previewKey === id)
            previewVisible = false
    }
}
