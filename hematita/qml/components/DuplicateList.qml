pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// Duplicate sets, one header per set and its copies below it. A set of equal
// size is a candidate until its content is checked; then it says so and
// offers to select every copy but one. One Tab stop: the arrows move, Space
// selects a copy, Enter on a header checks the content or, once verified,
// selects all copies but one. A set with a copy that could not be read says
// so and is never offered for "all but one"; the sets beyond the listed cap
// are counted at the foot. Delete asks to trash the selection. Copies are
// named by node id.
CelestinaSurface {
    id: card

    // Each row: { header, group, count, size, verified, unreadable } for a set, or
    // { header: false, id, name, path } for a copy.
    required property var duplicateRows
    required property var selectedIds
    // Whether the content check is running.
    required property bool checking
    // Candidate sets beyond the listed ones.
    required property int hiddenGroups

    signal confirmRequested()
    signal allButOneRequested(int group)
    signal toggled(int id)
    signal trashRequested()

    readonly property var emptyRow: ({ header: false, group: -1, count: 0, size: "",
                                       verified: false, unreadable: false, id: -1, name: "",
                                       path: "" })

    function keepViewport(apply) {
        const offset = list.contentY
        const index = list.currentIndex
        apply()
        list.forceLayout()
        const reach = Math.max(0, list.contentHeight - list.height)
        list.contentY = list.originY + Math.min(Math.max(0, offset - list.originY), reach)
        list.currentIndex = Math.min(index, card.duplicateRows.length - 1)
    }

    function takeFocus() {
        list.forceActiveFocus()
    }

    function activate(index) {
        if (index < 0 || index >= card.duplicateRows.length)
            return
        const row = card.duplicateRows[index]
        if (!row.header)
            card.toggled(row.id)
        else if (row.verified && !row.unreadable)
            card.allButOneRequested(row.group)
        else if (!card.checking)
            card.confirmRequested()
    }

    role: CelestinaSurface.Panel
    padding: 0

    contentItem: Item {
        ListView {
            id: list

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: hiddenNote.visible ? hiddenNote.top : parent.bottom
            anchors.margins: CelestinaTheme.spaceSm
            clip: true
            model: card.duplicateRows.length
            activeFocusOnTab: true
            keyNavigationEnabled: true
            highlightFollowsCurrentItem: false
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Duplicados")

            onCurrentIndexChanged: {
                if (list.currentIndex >= 0)
                    list.positionViewAtIndex(list.currentIndex, ListView.Contain)
            }

            Keys.onReturnPressed: function(event) {
                event.accepted = list.currentIndex >= 0
                card.activate(list.currentIndex)
            }
            Keys.onEnterPressed: function(event) {
                event.accepted = list.currentIndex >= 0
                card.activate(list.currentIndex)
            }
            Keys.onSpacePressed: function(event) {
                event.accepted = list.currentIndex >= 0
                card.activate(list.currentIndex)
            }
            Keys.onDeletePressed: function(event) {
                event.accepted = true
                card.trashRequested()
            }

            delegate: AbstractButton {
                id: row

                required property int index

                readonly property var rowData: row.index >= 0 && row.index < card.duplicateRows.length
                                               ? card.duplicateRows[row.index] : card.emptyRow
                readonly property bool marked: !row.rowData.header
                                               && card.selectedIds.indexOf(row.rowData.id) >= 0

                width: list.width
                implicitHeight: CelestinaTheme.rowHeight
                hoverEnabled: true
                focusPolicy: Qt.NoFocus

                Accessible.role: Accessible.ListItem
                Accessible.name: row.rowData.header
                                 ? qsTr("%1 copias · %2").arg(row.rowData.count).arg(row.rowData.size)
                                   + (row.rowData.verified ? ", " + qsTr("verificado") : "")
                                   + (row.rowData.unreadable ? ", " + qsTr("no se pudo leer") : "")
                                 : row.rowData.name + ", " + row.rowData.path
                Accessible.selected: row.marked

                onClicked: list.currentIndex = row.index
                onDoubleClicked: if (!row.rowData.header) card.toggled(row.rowData.id)

                background: CelestinaRowHighlight {
                    family: CelestinaRowHighlight.Content
                    hovered: row.hovered
                    pressed: row.down
                    selected: row.marked || row.index === list.currentIndex
                    focused: list.activeFocus && row.index === list.currentIndex
                }

                contentItem: Item {
                    Row {
                        anchors.fill: parent
                        anchors.leftMargin: row.rowData.header ? CelestinaTheme.spaceMd
                                                               : CelestinaTheme.spaceMd * 3
                        anchors.rightMargin: CelestinaTheme.spaceMd
                        spacing: CelestinaTheme.spaceMd

                        CelestinaIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.iconSm
                            height: width
                            name: row.rowData.header ? "copy" : (row.marked ? "check" : "file")
                            tone: CelestinaIcon.Secondary
                        }

                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - CelestinaTheme.iconSm - tail.width
                                   - parent.spacing * 2

                            Text {
                                width: parent.width
                                text: row.rowData.header
                                      ? qsTr("%1 copias · %2").arg(row.rowData.count)
                                                              .arg(row.rowData.size)
                                      : row.rowData.name
                                textFormat: Text.PlainText
                                color: CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontRowTitle
                                elide: Text.ElideMiddle
                            }

                            Text {
                                width: parent.width
                                visible: !row.rowData.header
                                text: row.rowData.path
                                textFormat: Text.PlainText
                                color: CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontCaption
                                elide: Text.ElideLeft
                            }
                        }

                        Row {
                            id: tail

                            anchors.verticalCenter: parent.verticalCenter
                            spacing: CelestinaTheme.spaceXs
                            visible: row.rowData.header

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: row.rowData.unreadable
                                text: qsTr("no se pudo leer")
                                color: CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontCaption
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: row.rowData.verified
                                text: qsTr("verificado")
                                color: CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontCaption
                            }

                            CelestinaIconButton {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: !row.rowData.verified
                                enabled: !card.checking
                                iconName: "check"
                                helpText: qsTr("Comprobar contenido")
                                role: CelestinaButton.Ghost
                                focusPolicy: Qt.NoFocus
                                onClicked: card.confirmRequested()
                            }

                            CelestinaIconButton {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: row.rowData.verified && !row.rowData.unreadable
                                iconName: "files"
                                helpText: qsTr("Seleccionar todas menos una")
                                role: CelestinaButton.Ghost
                                focusPolicy: Qt.NoFocus
                                onClicked: card.allButOneRequested(row.rowData.group)
                            }
                        }
                    }
                }
            }
        }

        Text {
            id: hiddenNote

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: CelestinaTheme.spaceSm
            visible: card.hiddenGroups > 0
            text: qsTr("%1 grupos más no se muestran").arg(card.hiddenGroups)
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }

        Text {
            anchors.centerIn: parent
            visible: card.duplicateRows.length === 0
            text: qsTr("Sin duplicados")
            color: CelestinaTheme.textMuted
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
