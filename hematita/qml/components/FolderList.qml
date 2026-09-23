pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// One folder's listing while browsing: folders first, then files, with a
// file's apparent size. The page weaves the rows; this list shows them, is
// one Tab stop, and says which row the person wants to enter or that they
// want to go up. A name is the filesystem's own, shown raw.
CelestinaSurface {
    id: card

    // Each row: { kind (dir|file|other), name, size }.
    required property var folderRows
    // Whether the folder could not be listed, which the list says in place.
    required property bool failed
    // Whether the listing is still being read; an empty list then says
    // nothing rather than "empty".
    required property bool loading

    signal entered(int index)
    signal upRequested()

    readonly property var emptyRow: ({ kind: "", name: "", size: "" })

    // A new folder starts at its top with the cursor on its first row.
    function reset(apply) {
        apply()
        list.currentIndex = card.folderRows.length > 0 ? 0 : -1
        list.positionViewAtBeginning()
    }

    // A rebuilt model puts the view back at the top; the offset read before it
    // is put back, clamped, so a refresh never takes the person's place.
    function keepViewport(apply) {
        const offset = list.contentY
        const index = list.currentIndex
        apply()
        list.forceLayout()
        const reach = Math.max(0, list.contentHeight - list.height)
        list.contentY = list.originY + Math.min(Math.max(0, offset - list.originY), reach)
        list.currentIndex = Math.min(index, card.folderRows.length - 1)
    }

    function kindWord(kind) {
        switch (kind) {
        case "dir": return qsTr("carpeta")
        case "file": return qsTr("archivo")
        }
        return qsTr("otro")
    }

    function takeFocus() {
        list.forceActiveFocus()
    }

    role: CelestinaSurface.Panel
    padding: 0

    contentItem: Item {
        ListView {
            id: list

            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceSm
            clip: true
            model: card.folderRows.length
            activeFocusOnTab: true
            keyNavigationEnabled: true
            highlightFollowsCurrentItem: false
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Contenido de la carpeta")

            onCurrentIndexChanged: {
                if (list.currentIndex >= 0)
                    list.positionViewAtIndex(list.currentIndex, ListView.Contain)
            }

            Keys.onReturnPressed: function(event) {
                event.accepted = list.currentIndex >= 0
                if (event.accepted)
                    card.entered(list.currentIndex)
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Backspace) {
                    event.accepted = true
                    card.upRequested()
                }
            }
            Keys.onEnterPressed: function(event) {
                event.accepted = list.currentIndex >= 0
                if (event.accepted)
                    card.entered(list.currentIndex)
            }

            delegate: AbstractButton {
                id: row

                required property int index

                readonly property var rowData: row.index >= 0 && row.index < card.folderRows.length
                                               ? card.folderRows[row.index] : card.emptyRow

                width: list.width
                implicitHeight: CelestinaTheme.rowHeight
                hoverEnabled: true
                // The list is the one Tab stop; the arrows move between rows.
                focusPolicy: Qt.NoFocus

                Accessible.role: Accessible.ListItem
                Accessible.name: row.rowData.name + ", " + card.kindWord(row.rowData.kind)
                                 + (row.rowData.kind === "file" ? ", " + row.rowData.size : "")
                Accessible.selected: row.index === list.currentIndex

                onClicked: list.currentIndex = row.index
                onDoubleClicked: card.entered(row.index)

                background: CelestinaRowHighlight {
                    family: CelestinaRowHighlight.Content
                    hovered: row.hovered
                    pressed: row.down
                    selected: row.index === list.currentIndex
                    focused: list.activeFocus && row.index === list.currentIndex
                }

                contentItem: Item {
                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: CelestinaTheme.spaceMd
                        anchors.rightMargin: CelestinaTheme.spaceMd
                        spacing: CelestinaTheme.spaceMd

                        CelestinaIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.iconSm
                            height: width
                            name: row.rowData.kind === "dir" ? "folder" : "file"
                            tone: CelestinaIcon.Secondary
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - CelestinaTheme.iconSm - sizeLabel.width
                                   - parent.spacing * 2
                            text: row.rowData.name
                            textFormat: Text.PlainText
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontRowTitle
                            elide: Text.ElideMiddle
                        }

                        Text {
                            id: sizeLabel

                            anchors.verticalCenter: parent.verticalCenter
                            text: row.rowData.kind === "file" ? row.rowData.size : "—"
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontRowSecondary
                            font.features: CelestinaTheme.fontFeaturesTabular
                        }
                    }
                }
            }
        }

        Text {
            anchors.centerIn: parent
            visible: card.failed || (!card.loading && card.folderRows.length === 0)
            text: card.failed ? qsTr("No se pudo leer esta carpeta") : qsTr("Carpeta vacía")
            color: card.failed ? CelestinaTheme.danger : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }

        CelestinaScrollBar {
            surface: list
            anchors.right: list.right
            anchors.top: list.top
            anchors.bottom: list.bottom
        }
    }
}
