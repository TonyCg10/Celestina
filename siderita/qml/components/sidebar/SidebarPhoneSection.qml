pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

Column {
    id: root

    required property var hostWindow

    // El menú vive en la barra lateral, no aquí: este componente sólo dice
    // dónde y para qué ruta hay que abrirlo.
    signal contextMenuRequested(string mountPath, point where)
    readonly property var controller: hostWindow.activeController
    readonly property bool hasPhones:
            controller && controller.phoneNames.length > 0

    visible: hasPhones
    height: visible ? implicitHeight : 0

    SidebarSectionHeader {
        width: root.width
        title: "MÓVIL"
        textScale: root.hostWindow.sidebarTextScale
        iconScale: root.hostWindow.sidebarIconScale
        collapsible: false
        interactive: false
    }

    Repeater {
        model: root.controller ? root.controller.phoneNames : []

        delegate: Item {
            id: phoneRow

            required property int index
            required property string modelData
            readonly property var info: {
                if (!root.controller)
                    return []
                root.controller.phoneRevision
                return root.controller.phoneInfo(phoneRow.index)
            }
            readonly property bool connected:
                    info.length > 3 && info[3] === "1"
            readonly property bool mounted:
                    info.length > 5 && info[4] === "1"
                    && info[5].length > 0
            readonly property string mountPath: mounted ? info[5] : ""
            readonly property bool current: mounted
                    && mountPath === (root.controller
                                      ? root.controller.currentPathKey : "")

            width: root.width
            height: root.hostWindow.sidebarRowHeight
            activeFocusOnTab: mounted
            Accessible.role: Accessible.Button
            Accessible.name: phoneRow.modelData
                             + (phoneRow.connected
                                ? phoneRow.mounted
                                  ? ", conectado" : ", conectado, preparando archivos"
                                : ", desconectado")
            Accessible.onPressAction: phoneRow.activate()

            function activate() {
                if (root.controller && phoneRow.mounted)
                    root.controller.openPhone(phoneRow.index)
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    phoneRow.activate()
                    event.accepted = true
                }
            }

            Rectangle {
                anchors.fill: parent
                anchors.leftMargin: 2
                anchors.rightMargin: 2
                radius: CelestinaTheme.radiusSm
                border.width: phoneRow.activeFocus
                              ? CelestinaTheme.borderFocus : 0
                border.color: CelestinaTheme.focusRing
                color: phoneRow.current
                       ? CelestinaTheme.badgeAccentFill
                       : (phoneRow.mounted && phoneMouse.containsMouse)
                         ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
            }

            CelestinaIcon {
                id: phoneIcon

                x: 12
                anchors.verticalCenter: parent.verticalCenter
                width: Math.round(CelestinaTheme.iconSm
                                  * root.hostWindow.sidebarIconScale)
                height: width
                name: "phone"
                fallbackName: "phone"
                tone: phoneRow.current
                      ? CelestinaIcon.Accent : CelestinaIcon.Device
                opacity: phoneRow.connected
                         ? 1 : CelestinaTheme.disabledOpacity
            }

            Row {
                x: phoneIcon.x + phoneIcon.width + 10
                width: parent.width - x - 10
                height: parent.height
                spacing: CelestinaTheme.spaceXs

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth,
                                    parent.width - connectionDot.width
                                    - parent.spacing)
                    text: phoneRow.modelData
                    color: phoneRow.current
                           ? CelestinaTheme.accent : CelestinaTheme.text
                    opacity: phoneRow.connected
                             ? 1 : CelestinaTheme.disabledOpacity
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontBody
                                               * root.hostWindow.sidebarTextScale)
                    elide: Text.ElideRight
                }

                Rectangle {
                    id: connectionDot

                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.max(6, Math.round(7 * root.hostWindow.sidebarIconScale))
                    height: width
                    radius: width / 2
                    color: phoneRow.connected
                           ? CelestinaTheme.success : CelestinaTheme.danger
                    Accessible.ignored: true
                }
            }

            MouseArea {
                id: phoneMouse

                anchors.fill: parent
                // Los tres botones, como cualquier otro lugar de la barra: el
                // central abre en pestaña de fondo y el derecho trae el menú.
                // Faltaban los dos, así que del móvil no se podía sacar pestaña.
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                hoverEnabled: true
                enabled: phoneRow.mounted
                cursorShape: phoneRow.mounted ? Qt.PointingHandCursor
                                              : Qt.ArrowCursor
                onClicked: function(mouse) {
                    if (mouse.button === Qt.MiddleButton)
                        root.hostWindow.openTab(phoneRow.mountPath, false)
                    else if (mouse.button === Qt.RightButton)
                        root.contextMenuRequested(
                                phoneRow.mountPath,
                                phoneMouse.mapToItem(null, mouse.x, mouse.y))
                    else
                        phoneRow.activate()
                }
            }
        }
    }
}
