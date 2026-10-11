import QtQuick

// A stand-in for the Rust `FluoritaPlayer`, as far as the trim uses it: a
// film that "opens" paused at its start, three seconds long, with a render
// handle the stand-in video surface "releases" on close; seeks and plays are
// recorded, and a seek moves the confirmed position at once.
QtObject {
    id: player

    property string state: "inactivo"
    property real renderHandle: 0
    property real positionSeconds: 0
    property real durationSeconds: 0
    property bool hasVideo: false
    property bool pending: false
    property string errorMessage: ""
    property bool announced: true

    property var opened: []
    property var seeks: []
    property int plays: 0
    property int pauses: 0
    property int closes: 0
    property int released: 0

    function open(key) {
        player.opened = player.opened.concat([key])
        player.hasVideo = true
        player.renderHandle = 1
        player.durationSeconds = 3
        player.positionSeconds = 0
        player.state = "pausado"
    }

    function play() {
        player.plays += 1
        player.state = "reproduciendo"
    }

    function pause() {
        player.pauses += 1
        player.state = "pausado"
    }

    function toggle() {
        if (player.state === "reproduciendo")
            player.pause()
        else if (player.state === "pausado")
            player.play()
    }

    function seek(seconds) {
        player.seeks = player.seeks.concat([seconds])
        player.positionSeconds = seconds
    }

    function close() {
        player.closes += 1
        player.renderHandle = 0
    }

    function surfaceReady() {}
    function surfaceFailed() {}

    function surfaceReleased() {
        player.released += 1
        player.state = "inactivo"
        player.durationSeconds = 0
        player.positionSeconds = 0
    }
}
