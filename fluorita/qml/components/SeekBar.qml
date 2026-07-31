import QtQuick
import org.celestina.fluorita 1.0

// The playhead, as a control rather than a decoration.
//
// It is local rather than shared: the suite has no slider yet, and a component
// enters `celestina-style` when a second application demonstrates the same
// semantics, not before. It is also not a Qt `Slider`: the ratchet keeps new
// raw Qt Controls out, and this needs to show *two* different things anyway —
// where the engine says the playhead is, and where a click asked it to go.
Item {
    id: bar

    required property real position
    required property real duration
    // A seek that has been asked for but not confirmed. The fill stays where
    // the engine last reported; this is drawn separately so the interface never
    // claims the playhead already moved.
    property real pendingPosition: -1

    signal seekRequested(real seconds)

    readonly property real fraction: bar.duration > 0
        ? Math.max(0, Math.min(1, bar.position / bar.duration))
        : 0
    readonly property real pendingFraction: bar.duration > 0 && bar.pendingPosition >= 0
        ? Math.max(0, Math.min(1, bar.pendingPosition / bar.duration))
        : -1
    // Arrow keys move by a step a person can feel without losing their place.
    readonly property real step: 5
    // Qt reports the reason focus arrived; a mouse click must not raise a ring.
    property int focusReason: Qt.OtherFocusReason

    // `enabled` is Item's own: a seek bar with no duration is disabled by the
    // consumer, and redeclaring it here would shadow the base property.
    implicitHeight: CelestinaTheme.spaceLg
    activeFocusOnTab: bar.enabled

    Accessible.role: Accessible.Slider
    Accessible.name: qsTr("Posición")
    Accessible.description: qsTr("Flechas para avanzar o retroceder cinco segundos")
    Accessible.focusable: bar.enabled

    Rectangle {
        id: track

        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        height: CelestinaTheme.spaceXs
        radius: CelestinaTheme.radiusPill
        color: CelestinaTheme.divider

        Rectangle {
            width: track.width * bar.fraction
            height: track.height
            radius: track.radius
            color: bar.enabled ? CelestinaTheme.accent : CelestinaTheme.textFaint
        }

        // Where the seek was asked to go, while it is still pending.
        Rectangle {
            visible: bar.pendingFraction >= 0
            x: track.width * Math.max(0, bar.pendingFraction) - width / 2
            anchors.verticalCenter: track.verticalCenter
            width: CelestinaTheme.spaceSm
            height: CelestinaTheme.spaceSm
            radius: CelestinaTheme.radiusPill
            color: CelestinaTheme.textMuted
        }
    }

    CelestinaFocusRing {
        target: track
        cornerRadius: track.radius
        // The ring follows keyboard focus, not a click: `visualFocus` is what
        // the contract asks new controls to key off.
        shown: bar.activeFocus && bar.focusReason !== Qt.MouseFocusReason
    }

    MouseArea {
        anchors.fill: parent
        enabled: bar.enabled
        onClicked: function(mouse) {
            bar.focusReason = Qt.MouseFocusReason
            bar.forceActiveFocus(Qt.MouseFocusReason)
            bar.seekTo(mouse.x / Math.max(1, bar.width) * bar.duration)
        }
    }

    onActiveFocusChanged: if (!bar.activeFocus) bar.focusReason = Qt.OtherFocusReason

    Keys.onLeftPressed: bar.seekTo(bar.position - bar.step)
    Keys.onRightPressed: bar.seekTo(bar.position + bar.step)
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Home) {
            bar.seekTo(0)
            event.accepted = true
        } else if (event.key === Qt.Key_End && bar.duration > 0) {
            bar.seekTo(bar.duration)
            event.accepted = true
        }
    }

    function seekTo(seconds) {
        if (!bar.enabled || bar.duration <= 0)
            return
        bar.seekRequested(Math.max(0, Math.min(bar.duration, seconds)))
    }
}
