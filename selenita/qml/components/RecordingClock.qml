import QtQuick
import org.celestina.selenita 1.0

// The recording's elapsed time, as the recording card and the recording bar
// both show it: whole seconds since the controller's `recordingStartedAt`
// while `active`, 0 otherwise; `text` is `m:ss`, with the hours only once
// there are some. A new origin (the next recording's start) resets it at
// once rather than on the next second. It looks four times a second: a
// timer may fire a little early, and a one-second tick that lands just
// before a second turns would skip that second on the next.
QtObject {
    id: clock

    // Whether a recording is under way.
    required property bool active
    // Whole seconds since the recording began.
    property int elapsed: 0
    readonly property string text: clock.format(clock.elapsed)

    // `m:ss`, the hours only once there are some.
    function format(seconds: int): string {
        const hours = Math.floor(seconds / 3600)
        const minutes = Math.floor((seconds % 3600) / 60)
        const rest = seconds % 60
        const two = value => (value < 10 ? "0" : "") + value
        return hours > 0 ? hours + ":" + two(minutes) + ":" + two(rest)
                         : minutes + ":" + two(rest)
    }

    function tick() {
        clock.elapsed = clock.active
                        ? Math.max(0, Math.floor(
                              (Date.now() - SelenitaController.recordingStartedAt) / 1000))
                        : 0
    }

    onActiveChanged: clock.tick()
    Component.onCompleted: clock.tick()

    readonly property Connections origin: Connections {
        target: SelenitaController

        function onRecordingStartedAtChanged() {
            if (clock.active)
                clock.tick()
        }
    }

    readonly property Timer ticker: Timer {
        interval: 250
        repeat: true
        running: clock.active
        onTriggered: clock.tick()
    }
}
