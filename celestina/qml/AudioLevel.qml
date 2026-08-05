// What the session is playing through, and whether it is silenced.
//
// It reads as text rather than a glyph on purpose: the suite's icon catalogue
// is closed and vendored, and inventing a speaker for it would put
// non-canonical artwork into a set that is canonical everywhere else. A number
// beside CPU and RAM is also the language this panel already speaks.
//
// Volume and the microphone are two separate controls, side by side in one
// row, each with its own click area — they used to share one full-width
// `MouseArea`, so clicking the "micro" label toggled the speaker instead of
// the microphone the label names.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    // The `audio` provider's fields, or `undefined` when no default device can
    // be read. `var` is necessary because QML has no typed map.
    required property var reading
    signal muteToggled()
    signal micMuteToggled()
    signal mixerRequested()
    // One step of the session's own volume step, up or down.
    signal stepRequested(int direction)

    readonly property bool hasReading: reading !== undefined
                                       && reading.volume !== undefined
    readonly property bool muted: hasReading && reading.muted === true
    // Present whenever the provider could read a default source at all —
    // shown either way, since a toggle that only appears in one of its two
    // states is a toggle with no way back to the other from the panel.
    readonly property bool hasMic: hasReading && reading.micVolume !== undefined
    readonly property bool micMuted: hasMic && reading.micMuted === true

    // What a screen reader is told about the speaker. Named here, and read
    // through `hasReading`, because an `Accessible` binding is evaluated even
    // while the widget is hidden: reaching into an absent reading from one is
    // what threw on every frame the helper missed.
    readonly property string spokenVolume: !hasReading
            ? qsTr("Volumen sin lectura")
            : muted
              ? qsTr("Volumen silenciado, %1 %").arg(reading.volume)
              : qsTr("Volumen %1 %").arg(reading.volume)
    readonly property string spokenMic: !hasMic
            ? qsTr("Micrófono sin lectura")
            : micMuted ? qsTr("Micrófono silenciado") : qsTr("Micrófono activo")

    implicitWidth: hasReading ? readings.implicitWidth : 0
    implicitHeight: 26
    visible: hasReading

    WheelHandler {
        // A wheel notch is 120 eighths of a degree; a touchpad's finer scroll
        // accumulates instead of being dropped. Scrolling anywhere on the
        // widget still steps the volume, regardless of which control is under
        // the cursor.
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

    Row {
        id: readings

        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceSm

        Item {
            id: volumeTile

            width: volumeText.implicitWidth
            height: 26
            Accessible.role: Accessible.Button
            Accessible.name: root.spokenVolume
            Accessible.onPressAction: root.muteToggled()
            Accessible.onScrollUpAction: root.stepRequested(1)
            Accessible.onScrollDownAction: root.stepRequested(-1)

            Text {
                id: volumeText

                anchors.verticalCenter: parent.verticalCenter
                text: root.hasReading ? qsTr("%1 %").arg(root.reading.volume) : ""
                // A muted device still remembers its level, so the number stays
                // and says it is not being heard.
                color: root.muted ? CelestinaTheme.textMuted : CelestinaTheme.text
                font.strikeout: root.muted
                font.family: CelestinaTheme.sansFamily
                font.features: CelestinaTheme.fontFeaturesTabular
                font.pixelSize: CelestinaTheme.fontCaption
            }

            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                cursorShape: Qt.PointingHandCursor
                onClicked: (mouse) => {
                    if (mouse.button === Qt.MiddleButton) {
                        root.mixerRequested();
                        return;
                    }
                    root.muteToggled();
                }
            }
        }

        Item {
            id: micTile

            visible: root.hasMic
            width: micText.implicitWidth
            height: 26
            Accessible.role: Accessible.Button
            Accessible.name: root.spokenMic
            Accessible.onPressAction: root.micMuteToggled()

            Text {
                id: micText

                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("micro")
                // Muted reads exactly like a muted volume — struck through and
                // in the danger colour; live is the same quiet grey the volume
                // number uses, present but not asking for attention.
                color: root.micMuted ? CelestinaTheme.danger : CelestinaTheme.textMuted
                font.strikeout: root.micMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }

            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.micMuteToggled()
            }
        }
    }
}
