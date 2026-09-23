pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// The analysed folder's children, biggest first: name, a bar of the share of
// the folder in the entry's kind colour, size and percentage, and a mark when
// the entry is selected. One Tab stop; the arrows move, Space selects, Enter
// enters a folder, Backspace goes up. Rows are named by node id.
CelestinaSurface {
    id: card

    // Each row: { id, name, kind, tone, share (0..1), size, percent }.
    required property var usageRows
    required property var selectedIds
    // The colour each tone paints (dir|file|duplicate|empty|unreadable|other).
    required property var toneColors
    // Whether a filter is hiding rows, which changes what "empty" means.
    required property bool filtered

    signal entered(int id)
    signal toggled(int id)
    signal chosen(int id)
    signal upRequested()

    readonly property var emptyRow: ({ id: -1, name: "", kind: "", tone: "", share: 0,
                                       size: "", percent: "" })

    function reset(apply) {
        apply()
        list.currentIndex = card.usageRows.length > 0 ? 0 : -1
        list.positionViewAtBeginning()
    }

    function keepViewport(apply) {
        const offset = list.contentY
        const index = list.currentIndex
        apply()
        list.forceLayout()
        const reach = Math.max(0, list.contentHeight - list.height)
        list.contentY = list.originY + Math.min(Math.max(0, offset - list.originY), reach)
        list.currentIndex = Math.min(index, card.usageRows.length - 1)
    }

    // Puts the cursor on the row of node `id`, if it is listed.
    function follow(id) {
        for (let index = 0; index < card.usageRows.length; ++index) {
            if (card.usageRows[index].id === id) {
                list.currentIndex = index
                return
            }
        }
    }

    function takeFocus() {
        list.forceActiveFocus()
    }

    function currentId() {
        return list.currentIndex >= 0 && list.currentIndex < card.usageRows.length
               ? card.usageRows[list.currentIndex].id : -1
    }

    role: CelestinaSurface.Panel
    padding: 0

    contentItem: Item {
        ListView {
            id: list

            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceSm
            clip: true
            model: card.usageRows.length
            activeFocusOnTab: true
            keyNavigationEnabled: true
            highlightFollowsCurrentItem: false
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Uso de la carpeta")

            onCurrentIndexChanged: {
                if (list.currentIndex >= 0)
                    list.positionViewAtIndex(list.currentIndex, ListView.Contain)
                card.chosen(card.currentId())
            }

            Keys.onReturnPressed: function(event) {
                event.accepted = list.currentIndex >= 0
                if (event.accepted)
                    card.entered(card.currentId())
            }
            Keys.onEnterPressed: function(event) {
                event.accepted = list.currentIndex >= 0
                if (event.accepted)
                    card.entered(card.currentId())
            }
            Keys.onSpacePressed: function(event) {
                event.accepted = list.currentIndex >= 0
                if (event.accepted)
                    card.toggled(card.currentId())
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Backspace) {
                    event.accepted = true
                    card.upRequested()
                }
            }

            delegate: AbstractButton {
                id: row

                required property int index

                readonly property var rowData: row.index >= 0 && row.index < card.usageRows.length
                                               ? card.usageRows[row.index] : card.emptyRow
                readonly property bool marked: card.selectedIds.indexOf(row.rowData.id) >= 0

                width: list.width
                implicitHeight: CelestinaTheme.rowHeight
                hoverEnabled: true
                focusPolicy: Qt.NoFocus

                Accessible.role: Accessible.ListItem
                Accessible.name: row.rowData.name + ", " + row.rowData.size + ", "
                                 + row.rowData.percent
                Accessible.selected: row.marked

                onClicked: list.currentIndex = row.index
                onDoubleClicked: card.entered(row.rowData.id)

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
                        anchors.leftMargin: CelestinaTheme.spaceMd
                        anchors.rightMargin: CelestinaTheme.spaceMd
                        spacing: CelestinaTheme.spaceMd

                        CelestinaIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.iconSm
                            height: width
                            name: row.marked ? "check" : (row.rowData.kind === "dir" ? "folder" : "file")
                            tone: CelestinaIcon.Secondary
                        }

                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - CelestinaTheme.iconSm - numbers.width
                                   - parent.spacing * 2
                            spacing: CelestinaTheme.spaceXs

                            Text {
                                width: parent.width
                                text: row.rowData.name
                                textFormat: Text.PlainText
                                color: CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontRowTitle
                                elide: Text.ElideMiddle
                            }

                            Rectangle {
                                width: parent.width
                                height: CelestinaTheme.spaceXs
                                radius: CelestinaTheme.radiusPill
                                color: CelestinaTheme.badgeFill

                                Rectangle {
                                    width: parent.width * Math.max(0, Math.min(1, row.rowData.share))
                                    height: parent.height
                                    radius: parent.radius
                                    color: card.toneColors[row.rowData.tone] || CelestinaTheme.textFaint
                                }
                            }
                        }

                        Column {
                            id: numbers

                            anchors.verticalCenter: parent.verticalCenter

                            Text {
                                anchors.right: parent.right
                                text: row.rowData.size
                                color: CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontRowSecondary
                                font.features: CelestinaTheme.fontFeaturesTabular
                            }

                            Text {
                                anchors.right: parent.right
                                text: row.rowData.percent
                                color: CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontCaption
                                font.features: CelestinaTheme.fontFeaturesTabular
                            }
                        }
                    }
                }
            }
        }

        Text {
            anchors.centerIn: parent
            visible: card.usageRows.length === 0
            text: card.filtered ? qsTr("Nada coincide con el filtro") : qsTr("Carpeta vacía")
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
