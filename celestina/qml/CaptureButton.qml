// The panel entry point for the shell's extensible toolbox menu.
//
// The button owns only opener geometry and short-lived failure feedback. The
// real `CaptureMenu` owns the capture action, while Niri remains the sole owner
// of screenshot behavior and its configured destination.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

PanelActionButton {
    id: root

    // Shown only while a refusal is fresh; the rest of the time the button has
    // nothing to say about captures it cannot observe.
    property bool failed: false

    function reportFailure() {
        failed = true;
        failureHold.restart();
    }

    iconName: "toolbox"
    iconSize: CelestinaTheme.iconSm
    role: failed ? CelestinaButton.Destructive : CelestinaButton.Ghost
    // `helpText` names the button for AT-SPI. BackdropButton suppresses the
    // corresponding hover card for every shell-local control.
    helpText: failed ? qsTr("No se pudo pedir la captura")
                     : qsTr("Caja de herramientas")

    Timer {
        id: failureHold

        interval: 2500
        onTriggered: root.failed = false
    }

}
