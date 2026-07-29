import QtQuick
import QtQuick.Controls
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
                                                       ? hostWindow.activeController.currentPath : "")

    height: hostWindow.sidebarRowHeight

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 2
        anchors.rightMargin: 2
        radius: CelestinaTheme.radiusSm
        color: root.current
               ? CelestinaTheme.badgeAccentFill
               : rowMouse.containsMouse
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
        cursorShape: Qt.PointingHandCursor
        ToolTip.visible: containsMouse
        ToolTip.delay: 600
        ToolTip.text: root.entry.path

        onClicked: function(mouse) {
            const controller = root.hostWindow.activeController
            if (!controller || root.missing)
                return

            if (mouse.button === Qt.RightButton) {
                const point = root.mapToItem(root.overlayParent, mouse.x, mouse.y)
                root.contextMenuRequested(root.entry.path, point.x, point.y)
            } else if (root.entry.kind === "directory") {
                if (mouse.button === Qt.MiddleButton)
                    root.hostWindow.openTab(root.entry.path, false)
                else
                    controller.openLocation(root.entry.path)
            } else {
                controller.revealPath(root.entry.path)
            }
        }
    }
}
