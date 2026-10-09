import QtQuick

// Stands in for the Rust `CalcitaActivation`: the signal a queued `Activate`
// or `Open` becomes on the Qt thread, and a `start` that records its call.
QtObject {
    property int starts: 0
    signal raiseRequested()
    function start() { starts += 1 }
}
