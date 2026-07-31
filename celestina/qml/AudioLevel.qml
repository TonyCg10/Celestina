// What the session is playing through, and whether it is silenced.
//
// It reads as text rather than a glyph on purpose: the suite's icon catalogue
// is closed and vendored, and inventing a speaker for it would put
// non-canonical artwork into a set that is canonical everywhere else. A number
// beside CPU and RAM is also the language this panel already speaks.
import CelestinaStyle
import QtQuick

Item {
    id: root

    // The `audio` provider's fields, or `undefined` when no default device can
    // be read. `var` is necessary because QML has no typed map.
    required property var reading
    signal muteToggled()
    signal mixerRequested()
    // One step of the session's own volume step, up or down.
    signal stepRequested(int direction)

    readonly property bool hasReading: reading !== undefined
                                       && reading.volume !== undefined
    readonly property bool muted: hasReading && reading.muted === true
    // A microphone is only news when it is silenced.
    readonly property bool micMuted: hasReading && reading.micMuted === true

    implicitWidth: hasReading ? readings.implicitWidth : 0
    implicitHeight: 26
    visible: hasReading
    Accessible.role: Accessible.Button
    Accessible.name: {
        if (!hasReading)
            return "";

        return muted ? qsTr("Volumen silenciado, %1 %").arg(reading.volume)
                     : qsTr("Volumen %1 %").arg(reading.volume);
    }
    Accessible.description: micMuted ? qsTr("El micrófono está silenciado") : ""
    Accessible.onPressAction: root.muteToggled()
    Accessible.onScrollUpAction: root.stepRequested(1)
    Accessible.onScrollDownAction: root.stepRequested(-1)

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

    Row {
        id: readings

        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceSm

        Text {
            text: root.hasReading ? qsTr("%1 %").arg(root.reading.volume) : ""
            // A muted device still remembers its level, so the number stays and
            // says it is not being heard.
            color: root.muted ? CelestinaTheme.textMuted : CelestinaTheme.text
            font.strikeout: root.muted
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontCaption
        }

        Text {
            visible: root.micMuted
            text: qsTr("micro")
            color: CelestinaTheme.danger
            font.strikeout: true
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }

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
