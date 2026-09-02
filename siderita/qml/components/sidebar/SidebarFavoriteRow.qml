import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property var hostWindow
    required property Item overlayParent
    required property var entry

    signal contextMenuRequested(string path, real popupX, real popupY)

    readonly property bool missing: entry.kind === "missing"
    readonly property bool current: !missing
                                    && entry.path === (hostWindow.activeController
                                                       ? hostWindow.activeController.markedKey : "")

    height: hostWindow.sidebarRowHeight
    // The same keyboard path the phone rows have: Tab reaches the row and
    // Return opens it. A missing favourite has nothing to open.
    activeFocusOnTab: !root.missing
    Accessible.role: Accessible.Button
    Accessible.name: root.entry.name
    Accessible.onPressAction: root.activate()

    function activate() {
        const controller = root.hostWindow.activeController
        if (!controller || root.missing)
            return
        if (root.entry.kind === "directory")
            controller.openKey(root.entry.path)
        else
            controller.revealPath(root.entry.path)
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.activate()
            event.accepted = true
        }
    }

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 2
        anchors.rightMargin: 2
        radius: CelestinaTheme.radiusSm
        border.width: root.activeFocus ? CelestinaTheme.borderFocus : 0
        border.color: CelestinaTheme.focusRing
        color: root.current
               ? CelestinaTheme.badgeAccentFill
               : (rowMouse.containsMouse && !root.missing)
                 ? CelestinaTheme.surfaceHover
                 : CelestinaTheme.clear

        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }
    }

    CelestinaIcon {
        id: entryIcon
        x: 12
        anchors.verticalCenter: parent.verticalCenter
        width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
        height: width
        opacity: root.missing ? CelestinaTheme.missingContentOpacity : 1
        name: root.entry.kind === "directory" ? "folder" : "text-x-generic"
        fallbackName: root.entry.kind === "directory" ? "folder" : "file"
        tone: CelestinaIcon.Favorite
    }

    Text {
        x: entryIcon.x + entryIcon.width + 10
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - x - 12
        text: root.entry.name
        color: root.missing ? CelestinaTheme.textMuted
               : root.current ? CelestinaTheme.accent
               : CelestinaTheme.text
        font.strikeout: root.missing
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.sidebarTextScale)
        font.weight: root.current ? CelestinaTheme.weightMedium
                                  : CelestinaTheme.weightRegular
        elide: Text.ElideMiddle
    }

    MouseArea {
        id: rowMouse
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        hoverEnabled: true
        // A missing favourite is not dead: its menu is where "Quitar de
        // favoritos" lives, so the right button stays live and only the
        // opening gestures — and the hand that promises them — are withheld.
        cursorShape: root.missing ? Qt.ArrowCursor : Qt.PointingHandCursor
        onClicked: function(mouse) {
            const controller = root.hostWindow.activeController
            if (!controller)
                return

            if (mouse.button === Qt.RightButton) {
                const point = root.mapToItem(root.overlayParent, mouse.x, mouse.y)
                root.contextMenuRequested(root.entry.path, point.x, point.y)
            } else if (root.missing) {
                return
            } else if (mouse.button === Qt.MiddleButton
                       && root.entry.kind === "directory") {
                root.hostWindow.openTab(root.entry.path, false)
            } else {
                root.activate()
            }
        }
    }
}
