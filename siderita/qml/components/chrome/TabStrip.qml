import QtQuick
import org.celestina.siderita 1.0

// Contextual tab row. It only exists when the window has more than one tab;
// every document is an independent glass pill and the row itself paints no box.
Item {
    id: root

    property var controller
    property var hostWindow
    property var topBar
    property bool active: false

    ListView {
        id: tabList
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.rightMargin: newTabPill.width + 8
        anchors.verticalCenter: parent.verticalCenter
        height: parent.height
        orientation: ListView.Horizontal
        spacing: 8
        clip: true
        model: root.hostWindow ? root.hostWindow.tabsModel : null
        currentIndex: root.hostWindow ? root.hostWindow.currentTabIndex : 0
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.HorizontalFlick
        onContentXChanged: if (root.active && root.topBar)
                               root.topBar.glassTick()

        Connections {
            target: root.hostWindow
            function onCurrentTabIndexChanged() {
                tabList.positionViewAtIndex(root.hostWindow.currentTabIndex,
                                            ListView.Contain)
            }
        }

        delegate: Item {
            id: chip

            required property int index
            required property string title

            readonly property bool activeTab: root.hostWindow
                    && index === root.hostWindow.currentTabIndex
            readonly property int tabCount: root.hostWindow
                    ? root.hostWindow.tabsModel.count : 1
            readonly property string displayTitle: {
                if (title.length > 0 && title !== "…")
                    return title
                if (!activeTab || !root.controller)
                    return "…"
                const path = root.controller.currentPath.replace(/\/+$/, "")
                if (path.length === 0 || path === "/")
                    return "Inicio"
                const split = path.lastIndexOf("/")
                return split >= 0 ? path.substring(split + 1) : path
            }

            width: Math.max(110, Math.min(200,
                    (tabList.width - (chip.tabCount - 1) * tabList.spacing)
                    / chip.tabCount))
            height: tabList.height

            // Every tab is an opaque box over the listing. `chipMouse` only
            // takes left and middle: without this floor a right click opened
            // the menu of the file behind it and a sweep dragged that file.
            CelestinaInputShield { }

            GlassSurface {
                id: chipGlass
                anchors.fill: parent
                backdropSource: root.topBar.activeView
                captureEnabled: root.active
                cornerRadius: CelestinaTheme.radiusPill
                elevation: 2

                Connections {
                    target: root.topBar
                    function onGlassTick() { chipGlass.refreshBackdrop() }
                }

                Component.onCompleted: if (root.active && root.topBar)
                                           Qt.callLater(chipGlass.refreshBackdrop)
            }

            Rectangle {
                anchors.fill: parent
                radius: CelestinaTheme.radiusPill
                color: chip.activeTab ? CelestinaTheme.badgeAccentFill
                                      : chipMouse.containsMouse
                                        ? CelestinaTheme.surfaceHover
                                        : CelestinaTheme.clear
                border.width: chip.activeTab ? CelestinaTheme.borderHairline : 0
                border.color: CelestinaTheme.dividerStrong

                Behavior on color {
                    ColorAnimation { duration: CelestinaTheme.motionFast }
                }
            }

            CelestinaIcon {
                id: chipIcon
                x: 10
                anchors.verticalCenter: parent.verticalCenter
                width: Math.round(CelestinaTheme.iconSm
                                  * root.hostWindow.interfaceIconScale)
                height: width
                name: "folder"
                fallbackName: "folder"
                tone: chip.activeTab ? CelestinaIcon.Accent
                                     : CelestinaIcon.Navigation
            }

            Text {
                x: chipIcon.x + chipIcon.width + 8
                anchors.verticalCenter: parent.verticalCenter
                width: closeButton.x - x - 6
                text: chip.displayTitle
                color: chip.activeTab ? CelestinaTheme.text
                                      : CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary
                                           * root.hostWindow.interfaceTextScale)
                font.weight: chip.activeTab ? CelestinaTheme.weightMedium
                                            : CelestinaTheme.weightRegular
                elide: Text.ElideRight
            }

            CelestinaIconButton {
                id: closeButton
                anchors.verticalCenter: parent.verticalCenter
                x: parent.width - width - 5
                width: 24
                height: 24
                role: CelestinaButton.Ghost
                density: CelestinaButton.Compact
                iconName: ""
                fallbackIcon: "x"
                Accessible.name: "Cerrar " + chip.displayTitle
                onClicked: root.hostWindow.closeTab(chip.index)
            }

            MouseArea {
                id: chipMouse
                anchors.fill: parent
                anchors.rightMargin: closeButton.width + 6
                acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: function(mouse) {
                    if (mouse.button === Qt.MiddleButton)
                        root.hostWindow.closeTab(chip.index)
                    else
                        root.hostWindow.selectTab(chip.index)
                }
            }
        }

    }

    Item {
        id: newTabPill
        x: Math.min(parent.width - width,
                    tabList.x + tabList.contentWidth + 8)
        anchors.verticalCenter: parent.verticalCenter
        width: parent.height
        height: width

        CelestinaInputShield { }

        GlassSurface {
            id: newTabGlass
            anchors.fill: parent
            backdropSource: root.topBar.activeView
            captureEnabled: root.active
            cornerRadius: CelestinaTheme.radiusPill
            elevation: 2

            Connections {
                target: root.topBar
                function onGlassTick() { newTabGlass.refreshBackdrop() }
            }
        }

        CelestinaIconButton {
            anchors.fill: parent
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: ""
            fallbackIcon: "plus"
            Accessible.name: "Nueva pestaña (Ctrl+T)"
            onClicked: root.hostWindow.openTab(root.controller.currentPathKey, true)
        }
    }
}
