// This output's monitor brightness, over DDC, and the way in to every other's.
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
//
// The wheel still steps this output's own monitor, because that is the one the
// pointer is over. Clicking opens `BrightnessMenu`, which is the session's
// whole list: the control stays clickable even when this output offers nothing,
// since the monitor that has no DDC is exactly the case where the author needs
// to be told which ones do.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

PanelActionButton {
    id: root

    // The `brightness` provider's payload: one entry per monitor that speaks
    // DDC, keyed by output name. `var` is necessary: QML has no typed map.
    required property var reading
    required property string outputName
    signal stepRequested(int direction)

    readonly property bool offered: root.reading !== undefined
                                    && root.reading[root.outputName] !== undefined
    readonly property bool known: root.offered
                                  && root.reading[root.outputName] !== null
    readonly property int level: root.known ? root.reading[root.outputName] : 0

    objectName: "celestina-brightness-button"
    iconName: "sun"
    fallbackIcon: "sun"
    helpText: root.known
              ? qsTr("Brillo de %1: %2 %").arg(root.outputName).arg(root.level)
              : root.offered
                ? qsTr("Brillo de %1: desconocido").arg(root.outputName)
                : qsTr("Brillo no disponible para %1").arg(root.outputName)
    Accessible.description: qsTr("Abre el menú de brillo")
    Accessible.onPressAction: root.requestMenu()
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
}
