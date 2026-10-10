import QtQuick

// Stands in for the Rust `SelenitaActivation`: the signals a queued
// `Activate`, `Capture`, `ToggleRecording` or `StopRecording` becomes on the
// Qt thread, and a `start` that records its call.
QtObject {
    property int starts: 0
    signal raiseRequested()
    signal captureRequested(string target)
    signal recordingRequested(string action)
    function start() { starts += 1 }
}
