import QtQuick
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// Play/pause, the playhead and the clock — and only for media that has them.
// An image would get no transport at all, which is why every control here is
// bound to what the player says the item supports.
RowLayout {
    id: transport

    required property FluoritaPlayer player

    // An image has no transport at all: the player says whether time means
    // anything for this item, and that is the only thing this asks.
    readonly property bool timed: transport.player.timed
    readonly property bool playing: transport.player.state === "reproduciendo"

    visible: transport.timed
    spacing: CelestinaTheme.spaceMd

    CelestinaIconButton {
        iconName: transport.playing ? "media-pause" : "media-play"
        enabled: transport.player.state !== "error"
        onClicked: transport.player.toggle()

        Accessible.role: Accessible.Button
        Accessible.name: transport.playing ? qsTr("Pausar") : qsTr("Reproducir")
        Accessible.onPressAction: transport.player.toggle()
    }

    SeekBar {
        id: seekBar

        Layout.fillWidth: true
        position: transport.player.positionSeconds
        duration: transport.player.durationSeconds
        enabled: transport.player.seekable && transport.player.durationSeconds > 0
        onSeekRequested: function(seconds) { transport.player.seek(seconds) }
    }

    Text {
        text: transport.clock(transport.player.positionSeconds)
            + " / " + transport.clock(transport.player.durationSeconds)
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    VolumeBar {
        level: transport.player.volumeLevel
        onVolumeRequested: function(level) { transport.player.setVolume(level) }
    }

    // Lets a host give keyboard seeking focus the moment a session starts,
    // instead of leaving arrow keys aimed at whatever the library last
    // focused until someone clicks the bar by hand.
    function focusSeek() {
        seekBar.forceActiveFocus(Qt.OtherFocusReason)
    }

    // `m:ss`, or `h:mm:ss` once there are hours. An unknown duration shows
    // `--:--` rather than a zero that would read as "no length".
    function clock(seconds) {
        if (!(seconds > 0))
            return "--:--"
        const total = Math.floor(seconds)
        const s = total % 60
        const m = Math.floor(total / 60) % 60
        const h = Math.floor(total / 3600)
        const pad = (value) => value < 10 ? "0" + value : "" + value
        return h > 0 ? h + ":" + pad(m) + ":" + pad(s) : m + ":" + pad(s)
    }
}
