// This output's monitor brightness, over DDC.
//
// The sun remains present even when this output offers no DDC brightness. A
// missing provider must not reflow the whole status row or make the control's
// meaning depend on a bare percentage. The exact reading remains available to
// assistive technology whenever the monitor answered.
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

    implicitWidth: CelestinaTheme.iconSm
    implicitHeight: 26
    visible: true
    Accessible.role: Accessible.Button
    Accessible.name: known ? qsTr("Brillo de %1: %2 %").arg(outputName).arg(level)
                           : offered
                             ? qsTr("Brillo de %1: desconocido").arg(outputName)
                             : qsTr("Brillo no disponible para %1").arg(outputName)
    Accessible.onScrollUpAction: {
        if (root.offered)
            root.stepRequested(1);
    }
    Accessible.onScrollDownAction: {
        if (root.offered)
            root.stepRequested(-1);
    }

    WheelHandler {
        property real steps: 0

        enabled: root.offered
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

    CelestinaIcon {
        objectName: "celestina-brightness-icon"
        anchors.centerIn: parent
        width: CelestinaTheme.iconSm
        height: CelestinaTheme.iconSm
        name: "sun"
        tone: CelestinaIcon.Primary
        Accessible.ignored: true
    }

}
