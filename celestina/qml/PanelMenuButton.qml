// The interaction contract for a panel control that opens a transient surface.
//
// CelestinaButton owns the common hover, pressed and keyboard-focus anatomy.
// This specialization snapshots both the complete control that was invoked
// and the exact icon inside it. The former places the contextual body; the
// latter positions the narrow waist of the background-only membrane.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

BackdropButton {
    id: root

    required property Item attachmentAnchor
    readonly property bool isPanelAttachmentSource: true
    // Set only by the tokened attachment lease that owns the currently mapped
    // contextual surface. The inherited background then keeps the ordinary
    // hover circle visible until that exact surface retires.
    property bool menuOpen: false

    signal menuRequested(rect openerRect, rect attachmentAnchorRect)

    function globalRect(item) {
        const topLeft = item.mapToGlobal(0, 0);
        const bottomRight = item.mapToGlobal(item.width, item.height);
        return Qt.rect(Math.min(topLeft.x, bottomRight.x),
                       Math.min(topLeft.y, bottomRight.y),
                       Math.abs(bottomRight.x - topLeft.x),
                       Math.abs(bottomRight.y - topLeft.y));
    }

    function attachmentAnchorGlobalRectNow() {
        return root.globalRect(root.attachmentAnchor);
    }

    function requestMenu() {
        root.menuRequested(root.globalRect(root),
                           root.attachmentAnchorGlobalRectNow());
    }

    height: CelestinaTheme.controlHeightXs
    density: CelestinaButton.Compact
    role: CelestinaButton.Ghost
    holdHoverFeedback: root.menuOpen
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
