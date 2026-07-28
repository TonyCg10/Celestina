import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── TabStrip ───────────────────────────────────────────────────────────────
// La tira de pestañas que flota bajo las migas cuando hay dos o más. El modelo
// de pestañas y sus acciones son de la ventana, así que `hostWindow`, el
// `controller` de la pestaña activa, la barra superior (`topBar`, para el
// cristal) y si esta pestaña está activa llegan por propiedad. La vista de
// carpeta la posiciona y decide cuándo se ve.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property var controller
    property var hostWindow
    property var topBar        // barra superior: floating / glassTick / activeView
    property bool active: false

    ListView {
        id: tabList
        anchors.left: parent.left
        anchors.right: newTabButton.left
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        height: parent.height
        orientation: ListView.Horizontal
        spacing: 8
        clip: true
        model: root.hostWindow ? root.hostWindow.tabsModel : null
        currentIndex: root.hostWindow ? root.hostWindow.currentTabIndex : 0
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.HorizontalFlick

        Connections {
            target: root.hostWindow
            function onCurrentTabIndexChanged() {
                tabList.positionViewAtIndex(root.hostWindow.currentTabIndex,
                                            ListView.Contain)
            }
        }
        // Chips move relative to the backdrop when the strip scrolls.
        onContentXChanged: if (root.topBar.floating) root.topBar.glassTick()

        delegate: Item {
            id: chip

            required property int index
            required property string title

            readonly property bool activeTab: root.hostWindow
                    && index === root.hostWindow.currentTabIndex
            readonly property int tabCount: root.hostWindow
                    ? root.hostWindow.tabsModel.count : 1

            // Tabs flex to share the strip's width (clamped), so they
            // shrink to fit as more open instead of overflowing off-edge;
            // only past the minimum does the strip start to scroll.
            width: Math.max(96, Math.min(200,
                    (tabList.width - (chip.tabCount - 1) * tabList.spacing)
                    / chip.tabCount))
            height: tabList.height

            // Solid pill at rest.
            Rectangle {
                id: chipFill
                anchors.fill: parent
                radius: CelestinaTheme.radiusSm
                color: chip.activeTab ? CelestinaTheme.surfaceSelected
                                      : chipMouse.containsMouse ? CelestinaTheme.surfaceHover
                                      : CelestinaTheme.inputFill
                border.width: chip.activeTab
                              ? CelestinaTheme.borderHairline
                              : (root.topBar.floating
                                 ? 0 : CelestinaTheme.borderHairline)
                border.color: chip.activeTab ? CelestinaTheme.dividerStrong
                                             : CelestinaTheme.inputBorder

                Behavior on color {
                    ColorAnimation { duration: CelestinaTheme.motionFast }
                }
            }

            // …fading to glass when content scrolls under the strip.
            GlassSurface {
                id: chipGlass
                anchors.fill: parent
                backdropSource: root.topBar.activeView
                captureEnabled: root.active && root.topBar.floating
                cornerRadius: CelestinaTheme.radiusSm
                opacity: (root.active && root.topBar.floating) ? 1 : 0
                Behavior on opacity {
                    NumberAnimation { duration: CelestinaTheme.motionNormal }
                }
                Connections {
                    target: root.topBar
                    function onGlassTick() { chipGlass.refreshBackdrop() }
                }
                Component.onCompleted: if (root.active && root.topBar.floating)
                                           Qt.callLater(chipGlass.refreshBackdrop)
            }

            CelestinaIcon {
                id: chipIcon
                x: 12
                anchors.verticalCenter: parent.verticalCenter
                width: Math.round(CelestinaTheme.iconSm * root.hostWindow.interfaceIconScale)
                height: CelestinaTheme.iconSm
                name: "folder"
                fallbackName: "folder"
                tone: chip.activeTab ? CelestinaIcon.Accent
                                     : CelestinaIcon.Secondary
            }

            Text {
                x: chipIcon.x + chipIcon.width + 8
                anchors.verticalCenter: parent.verticalCenter
                width: closeButton.x - x - 6
                text: chip.title.length > 0 ? chip.title : "…"
                color: chip.activeTab ? CelestinaTheme.text
                                      : CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary * root.hostWindow.interfaceTextScale)
                font.weight: chip.activeTab ? CelestinaTheme.weightMedium
                                            : CelestinaTheme.weightRegular
                elide: Text.ElideRight
            }

            Rectangle {
                id: closeButton
                anchors.verticalCenter: parent.verticalCenter
                x: parent.width - width - 8
                width: 20
                height: 20
                radius: CelestinaTheme.radiusSm
                color: closeMouse.containsMouse
                       ? CelestinaTheme.surfaceHover : CelestinaTheme.clear

                Text {
                    anchors.centerIn: parent
                    text: "×"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.interfaceTextScale)
                }

                MouseArea {
                    id: closeMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.hostWindow.closeTab(chip.index)
                }
            }

            MouseArea {
                id: chipMouse
                anchors.fill: parent
                anchors.rightMargin: 28   // leave the × its own handler
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

        ScrollBar.horizontal: ScrollBar {
            policy: ScrollBar.AsNeeded
            height: CelestinaTheme.compLinearTrackHeight
        }
    }

    CelestinaIconButton {
        id: newTabButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        iconName: "tab-new"
        fallbackIcon: "folder"
        helpText: "Nueva pestaña (Ctrl+T)"
        onClicked: root.hostWindow.openTab(root.controller.currentPath, true)
    }
}
