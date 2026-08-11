// The interaction contract for a panel control that opens a transient surface.
//
// CelestinaButton owns the common hover, pressed and keyboard-focus anatomy.
// This specialization contributes only the opener rectangle, so every caller
// sends the same real geometry instead of reconstructing it beside the button.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

BackdropButton {
    id: root

    signal menuRequested(int globalX, int globalY,
                         int openerWidth, int openerHeight)

    function requestMenu() {
        const at = root.mapToGlobal(0, 0);
        root.menuRequested(at.x, at.y, root.width, root.height);
    }

    height: CelestinaTheme.controlHeightXs
    density: CelestinaButton.Compact
    role: CelestinaButton.Ghost
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0
    activeFocusOnTab: true

    onClicked: root.requestMenu()
    Keys.onReturnPressed: function(event) {
        root.requestMenu();
        event.accepted = true;
    }
    Keys.onEnterPressed: function(event) {
        root.requestMenu();
        event.accepted = true;
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.NoButton
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
    }
}
