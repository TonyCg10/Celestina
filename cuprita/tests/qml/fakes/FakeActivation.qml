import QtQuick

// Stands in for `CupritaActivation`: the signal the Rust adapter emits on the
// Qt thread once a queued `Activate` or `Open` arrives.
QtObject {
    signal raiseRequested()
}
