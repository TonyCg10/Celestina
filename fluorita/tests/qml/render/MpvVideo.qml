import QtQuick

// A stand-in for the C++ `MpvVideo` surface: a renderer is "live" while it
// holds a handle, and letting go of one is confirmed a turn later, as the
// render thread confirms it.
Item {
    id: video

    property real handle: 0
    readonly property bool rendererLive: video.live
    property bool live: false

    signal contextCreated()
    signal contextReleased()
    signal contextFailed()

    onHandleChanged: {
        if (video.handle !== 0 && !video.live) {
            video.live = true
            video.contextCreated()
        } else if (video.handle === 0 && video.live) {
            release.start()
        }
    }

    property Timer release: Timer {
        interval: 0
        onTriggered: {
            video.live = false
            video.contextReleased()
        }
    }
}
