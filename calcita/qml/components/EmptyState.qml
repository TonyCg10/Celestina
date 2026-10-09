pragma ComponentBehavior: Bound
import QtQuick
import org.celestina.calcita 1.0

// What the window shows while no document is open: one grouped card with the
// state in words and the way out of it, and, when there are any, a second
// card with the recent documents. `openRequested` asks for the file chooser;
// `recentChosen(key)` reopens a recent document (a click, Enter or Space on
// its row); the row's menu (a right click, the Menu key or Shift+F10) offers
// «Quitar de recientes», which asks `recentForgotten(key)`.
Column {
    id: empty

    signal openRequested()
    signal recentChosen(string key)
    signal recentForgotten(string key)

    // What the row menu's glass blurs: the window's content.
    required property Item backdropSource
    readonly property alias recentMenu: rowMenu

    // Opens the row menu of the recent document at `index` beside `row`.
    function openRecentMenu(index, row) {
        rowMenu.key = empty.recentKeys[index]
        rowMenu.popupBeside(row, false)
    }

    // The recent documents as the controller lists them: pathkeys and the
    // names shown for them, in the same order.
    property var recentKeys: []
    property var recentNames: []
    // How many recents the card shows.
    readonly property int recentLimit: 6

    // The words and the button sit this far inside the card's edge.
    readonly property int rowInset: CelestinaTheme.spaceLg

    objectName: "emptyState"
    spacing: CelestinaTheme.spaceCardGap

    CelestinaSurface {
        width: empty.width
        height: content.implicitHeight + empty.rowInset * 2
        role: CelestinaSurface.Grouped
        Accessible.ignored: true

        Column {
            id: content
            x: empty.rowInset
            y: empty.rowInset
            width: parent.width - empty.rowInset * 2
            spacing: CelestinaTheme.spaceMd

            Text {
                objectName: "emptyTitle"
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: qsTr("Sin documento")
                color: CelestinaTheme.text
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightDemiBold
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                text: qsTr("Abre un PDF o suéltalo aquí para leerlo.")
                color: CelestinaTheme.textMuted
                font.pixelSize: CelestinaTheme.fontBody
            }

            CelestinaButton {
                objectName: "openButton"
                anchors.horizontalCenter: parent.horizontalCenter
                role: CelestinaButton.Primary
                text: qsTr("Abrir…")
                onClicked: empty.openRequested()
            }
        }
    }

    CelestinaSectionLabel {
        visible: recentRepeater.count > 0
        text: qsTr("Recientes")
    }

    CelestinaSurface {
        objectName: "recentCard"
        visible: recentRepeater.count > 0
        width: empty.width
        height: recentColumn.implicitHeight
        role: CelestinaSurface.Grouped
        Accessible.role: Accessible.List
        Accessible.name: qsTr("Documentos recientes")

        Column {
            id: recentColumn
            width: parent.width

            Repeater {
                id: recentRepeater
                model: Math.min(empty.recentKeys.length, empty.recentLimit)

                delegate: Item {
                    id: recentRow
                    required property int index

                    objectName: "recentRow" + index
                    width: recentColumn.width
                    height: CelestinaTheme.controlHeightLg
                    activeFocusOnTab: true
                    Accessible.role: Accessible.Button
                    Accessible.name: qsTr("Abrir %1").arg(empty.recentNames[recentRow.index])
                    Accessible.description: qsTr("Documento reciente; su menú permite quitarlo de recientes")
                    Accessible.onPressAction: empty.recentChosen(empty.recentKeys[recentRow.index])
                    Keys.onReturnPressed: empty.recentChosen(empty.recentKeys[recentRow.index])
                    Keys.onSpacePressed: empty.recentChosen(empty.recentKeys[recentRow.index])
                    Keys.onMenuPressed: empty.openRecentMenu(recentRow.index, recentRow)
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier)) {
                            empty.openRecentMenu(recentRow.index, recentRow)
                            event.accepted = true
                        }
                    }

                    Rectangle {
                        visible: recentRow.index > 0
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.leftMargin: empty.rowInset
                        anchors.rightMargin: empty.rowInset
                        height: CelestinaTheme.borderHairline
                        color: CelestinaTheme.divider
                    }

                    CelestinaRowHighlight {
                        anchors.fill: parent
                        hovered: rowMouse.containsMouse
                        pressed: rowMouse.pressed
                        focused: recentRow.activeFocus
                    }

                    CelestinaIcon {
                        id: rowIcon
                        x: empty.rowInset
                        anchors.verticalCenter: parent.verticalCenter
                        name: "application-pdf"
                        width: CelestinaTheme.iconSm
                        height: width
                    }

                    Text {
                        anchors.left: rowIcon.right
                        anchors.leftMargin: CelestinaTheme.spaceSm
                        anchors.right: parent.right
                        anchors.rightMargin: empty.rowInset
                        anchors.verticalCenter: parent.verticalCenter
                        elide: Text.ElideMiddle
                        text: empty.recentNames[recentRow.index]
                        color: CelestinaTheme.text
                        font.pixelSize: CelestinaTheme.fontBody
                    }

                    MouseArea {
                        id: rowMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        onClicked: mouse => {
                            if (mouse.button === Qt.RightButton)
                                empty.openRecentMenu(recentRow.index, recentRow)
                            else
                                empty.recentChosen(empty.recentKeys[recentRow.index])
                        }
                    }
                }
            }
        }
    }

    // The row menu of a recent document.
    GlassContextMenu {
        id: rowMenu
        objectName: "recentMenu"
        backdropSource: empty.backdropSource
        // The recent document the open menu is for.
        property string key: ""
        title: qsTr("Documento reciente")

        GlassMenuItem {
            objectName: "forgetRecentItem"
            text: qsTr("Quitar de recientes")
            icon.name: "list-x"
            onTriggered: empty.recentForgotten(rowMenu.key)
        }
    }
}
