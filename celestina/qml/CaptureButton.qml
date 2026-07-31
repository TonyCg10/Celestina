// The panel's screenshot button.
//
// It asks the compositor to open its own screenshot UI, which saves where the
// session's `screenshot-path` already points. Nothing here captures anything:
// a shell that reimplemented capture would be a second, worse screenshot tool.
//
// There is nothing to confirm afterwards — Niri takes over the screen and the
// panel cannot see what happened next — so the only thing it reports is a
// request it could not make.
import CelestinaStyle
import QtQuick

CelestinaIconButton {
    id: root

    signal captureRequested()

    // Shown only while a refusal is fresh; the rest of the time the button has
    // nothing to say about captures it cannot observe.
    property bool failed: false

    function reportFailure() {
        failed = true;
        failureHold.restart();
    }

    iconName: "scissors"
    role: failed ? CelestinaButton.Destructive : CelestinaButton.Ghost
    helpText: failed ? qsTr("No se pudo pedir la captura")
                     : qsTr("Captura de pantalla")
    onClicked: root.captureRequested()

    Timer {
        id: failureHold

        interval: 2500
        onTriggered: root.failed = false
    }

}
