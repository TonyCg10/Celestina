import QtQuick

// Stands in for the Rust `SelenitaActivation`: the signals a queued
// `Activate`, `Capture`, `ToggleRecording`, `StopRecording` or `Adopt`
// becomes on the Qt thread, and a `start` that records its call.
QtObject {
    property int starts: 0
    signal raiseRequested()
    signal captureRequested(string target)
    signal recordingRequested(string action)
    signal adoptRequested(string key)
    function start() { starts += 1 }
}
