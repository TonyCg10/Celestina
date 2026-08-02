// This output's monitor brightness, over DDC.
//
// It draws a small gauge rather than a percentage: the panel already shows a
// number for volume a few pixels away, and two bare percentages side by side
// say nothing about which is which. The number appears on hover, where it is
// asked for.
//
// Three states, deliberately distinct. No entry at all means this monitor does
// not speak DDC and has no brightness to offer. A null entry means it does and
// has not answered — unknown, which is not the same as dark. A number is a
// value that was read back from the monitor itself.
import CelestinaStyle
import QtQuick

Item {
    id: root

    // The `brightness` provider's payload: one entry per monitor that speaks
    // DDC, keyed by output name. `var` is necessary: QML has no typed map.
    required property var reading
    required property string outputName
    signal stepRequested(int direction)

    readonly property bool offered: reading !== undefined
                                    && reading[outputName] !== undefined
    readonly property bool known: offered && reading[outputName] !== null
    readonly property int level: known ? reading[outputName] : 0

    implicitWidth: offered ? (hover.hovered ? value.implicitWidth : gauge.width) : 0
    implicitHeight: 26
    visible: offered
    Accessible.role: Accessible.Button
    Accessible.name: known ? qsTr("Brillo de %1: %2 %").arg(outputName).arg(level)
                           : qsTr("Brillo de %1: desconocido").arg(outputName)
    Accessible.onScrollUpAction: root.stepRequested(1)
    Accessible.onScrollDownAction: root.stepRequested(-1)

    HoverHandler {
        id: hover
    }

    WheelHandler {
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

    Rectangle {
        id: gauge

        anchors.verticalCenter: parent.verticalCenter
        width: 28
        height: 4
        radius: height / 2
        visible: !hover.hovered
        color: CelestinaTheme.surfaceSelected

        Rectangle {
            width: parent.width * (root.known ? root.level / 100 : 0)
            height: parent.height
            radius: parent.radius
            color: CelestinaTheme.textMuted
        }

    }

    Text {
        id: value

        anchors.verticalCenter: parent.verticalCenter
        visible: hover.hovered
        // An unknown brightness says so rather than showing a number the
        // monitor never gave.
        text: root.known ? qsTr("%1 %").arg(root.level) : qsTr("— %")
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.features: CelestinaTheme.fontFeaturesTabular
        font.pixelSize: CelestinaTheme.fontCaption
    }

}
