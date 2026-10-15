// What the session is playing through, and the way in to everything it could
// play through instead.
//
// Speaker and microphone keep the suite's canonical glyphs and their exact
// values stay in accessible names. Clicking opens the audio menu — the same
// gesture every other panel opener answers — and muting, the microphone and
// the mixer live inside it as rows. The author chose that trade (2026-08-12):
// the one-click mute is gone, and in exchange the control behaves like every
// neighbour it has. The wheel still steps the volume without opening anything,
// because a wheel is not a click.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

PanelMenuButton {
    id: root

    // The `audio` provider's fields, or `undefined` when no default device can
    // be read. `var` is necessary because QML has no typed map.
    required property var reading
    // One step of the session's own volume step, up or down.
    signal stepRequested(int direction)

    readonly property bool hasReading: reading !== undefined
                                       && reading.volume !== undefined
    readonly property bool muted: hasReading && reading.muted === true
    // Present whenever the provider could read a default source at all: a
    // silenced microphone is news the panel must not sit on.
    readonly property bool hasMic: hasReading && reading.micVolume !== undefined
    readonly property bool micMuted: hasMic && reading.micMuted === true

    // Named here and read through the guards, because an `Accessible` binding
    // is evaluated even while the widget is hidden: reaching into an absent
    // reading from one is what threw on every frame the helper missed.
    readonly property string spokenVolume: !hasReading
            ? qsTr("Volumen sin lectura")
            : muted
              ? qsTr("Volumen silenciado, %1 %").arg(reading.volume)
              : qsTr("Volumen %1 %").arg(reading.volume)
    readonly property string spokenMic: !hasMic
            ? qsTr("Micrófono sin lectura")
            : micMuted ? qsTr("Micrófono silenciado") : qsTr("Micrófono activo")

    attachmentAnchor: volumeIcon
    visible: root.hasReading
    // Two icons in one capsule, with room to breathe at both ends: the shared
    // opener anatomy stretches its circle into a stadium for a control that is
    // wider than it is tall, which is exactly the shape the author asked the
    // speaker-and-microphone pair to read as.
    leftPadding: CelestinaTheme.spaceSm
    rightPadding: CelestinaTheme.spaceSm
    // An absent reading reserves nothing: the row must close over the gap
    // rather than hold space for a device that is not there.
    implicitWidth: root.hasReading
                   ? readings.implicitWidth + leftPadding + rightPadding : 0
    Accessible.name: root.hasMic
                     ? qsTr("%1. %2").arg(root.spokenVolume).arg(root.spokenMic)
                     : root.spokenVolume
    Accessible.description: qsTr("Abre el menú de audio")

    WheelHandler {
        // A wheel notch is 120 eighths of a degree; a touchpad's finer scroll
        // accumulates instead of being dropped.
        property real steps: 0

        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: (event) => {
            steps += event.angleDelta.y / 120;
            while (steps >= 1) {
                steps -= 1;
                root.stepRequested(1);
            }
            while (steps <= -1) {
                steps += 1;
                root.stepRequested(-1);
            }
        }
    }

    contentItem: Row {
        id: readings

        spacing: CelestinaTheme.spaceSm

        CelestinaIcon {
            id: volumeIcon
            objectName: "celestina-volume-icon"

            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            name: root.muted ? "media-volume-muted" : "media-volume"
            tone: CelestinaIcon.Primary
            tintOverride: root.ink.primary
            Accessible.ignored: true
        }

        CelestinaIcon {
            objectName: "celestina-mic-icon"

            anchors.verticalCenter: parent.verticalCenter
            visible: root.hasMic
            width: visible ? CelestinaTheme.iconSm : 0
            height: CelestinaTheme.iconSm
            name: root.micMuted ? "mic-off" : "mic"
            tone: CelestinaIcon.Primary
            tintOverride: root.ink.primary
            Accessible.ignored: true
        }
    }
}
