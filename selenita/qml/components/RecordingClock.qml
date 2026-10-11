import QtQuick
import org.celestina.selenita 1.0

// The recording's recorded time, as the recording card and the recording bar
// both show it: whole seconds since the controller's `recordingStartedAt`
// while it records (an origin a resume moves back by the time recorded
// before it), the controller's `recordingRecorded` held while it is paused,
// 0 otherwise; `text` is `m:ss`, with the hours only once there are some. A
// new origin (the next recording's start, a resume) resets it at once
// rather than on the next second. It looks four times a second while it
// records: a timer may fire a little early, and a one-second tick that lands
// just before a second turns would skip that second on the next.
QtObject {
    id: clock

    // Whether a recording is under way, recording or paused.
    required property bool active
    // Whether it is paused: the clock stands still.
    required property bool paused
    // Whole seconds recorded.
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
        const since = clock.paused
                      ? SelenitaController.recordingRecorded
                      : Date.now() - SelenitaController.recordingStartedAt
        clock.elapsed = clock.active ? Math.max(0, Math.floor(since / 1000)) : 0
    }

    onActiveChanged: clock.tick()
    onPausedChanged: clock.tick()
    Component.onCompleted: clock.tick()

    readonly property Connections origin: Connections {
        target: SelenitaController

        function onRecordingStartedAtChanged() {
            if (clock.active && !clock.paused)
                clock.tick()
        }

        function onRecordingRecordedChanged() {
            if (clock.paused)
                clock.tick()
        }
    }

    readonly property Timer ticker: Timer {
        interval: 250
        repeat: true
        running: clock.active && !clock.paused
        onTriggered: clock.tick()
    }
}
