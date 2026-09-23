pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// The storage section's starting points: one card, one row per mounted
// filesystem with how full it is. The page weaves the rows; this list shows
// them, is one Tab stop, and says which row the person wants to enter.
CelestinaSurface {
    id: card

    // Each row: { kind, name, path, share (0..1), usage, readable }.
    required property var locationRows

    signal entered(int index)

    readonly property var emptyRow: ({ kind: "", name: "", path: "", share: 0, usage: "",
                                       readable: false })

    // A rebuilt model puts the view back at the top; the offset read before it
    // is put back, clamped, so a refresh never takes the person's place.
    function keepViewport(apply) {
        const offset = list.contentY
        const index = list.currentIndex
        apply()
        list.forceLayout()
        const reach = Math.max(0, list.contentHeight - list.height)
        list.contentY = list.originY + Math.min(Math.max(0, offset - list.originY), reach)
        list.currentIndex = Math.min(index, card.locationRows.length - 1)
    }

    function glyph(kind) {
        switch (kind) {
        case "system": return "monitor"
        case "home": return "go-home"
        }
        return "hard-drive"
    }

    function shownName(row) {
        switch (row.kind) {
        case "system": return qsTr("Sistema")
        case "home": return qsTr("Inicio")
        }
        return row.name
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
            model: card.locationRows.length
            activeFocusOnTab: true
            keyNavigationEnabled: true
            highlightFollowsCurrentItem: false
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Ubicaciones")

            onCurrentIndexChanged: {
                if (list.currentIndex >= 0)
                    list.positionViewAtIndex(list.currentIndex, ListView.Contain)
            }

            Keys.onReturnPressed: function(event) {
                event.accepted = list.currentIndex >= 0
                if (event.accepted)
                    card.entered(list.currentIndex)
            }
            Keys.onEnterPressed: function(event) {
                event.accepted = list.currentIndex >= 0
                if (event.accepted)
                    card.entered(list.currentIndex)
            }

            delegate: AbstractButton {
                id: row

                required property int index

                readonly property var rowData: row.index >= 0 && row.index < card.locationRows.length
                                               ? card.locationRows[row.index] : card.emptyRow

                width: list.width
                implicitHeight: CelestinaTheme.rowHeight
                hoverEnabled: true
                // The list is the one Tab stop; the arrows move between rows.
                focusPolicy: Qt.NoFocus
                opacity: row.rowData.readable ? 1 : CelestinaTheme.unavailableContentOpacity

                Accessible.role: Accessible.ListItem
                Accessible.name: card.shownName(row.rowData) + ", " + row.rowData.path + ", "
                                 + row.rowData.usage
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
                            width: CelestinaTheme.iconMd
                            height: width
                            name: card.glyph(row.rowData.kind)
                            tone: CelestinaIcon.Secondary
                        }

                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - CelestinaTheme.iconMd - usage.width
                                   - parent.spacing * 2
                            spacing: CelestinaTheme.spaceXs

                            Text {
                                width: parent.width
                                text: card.shownName(row.rowData)
                                textFormat: Text.PlainText
                                color: CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontRowTitle
                                elide: Text.ElideRight
                            }

                            Text {
                                width: parent.width
                                text: row.rowData.path
                                textFormat: Text.PlainText
                                color: CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontRowSecondary
                                elide: Text.ElideMiddle
                            }

                            // How full the filesystem is.
                            Rectangle {
                                width: parent.width
                                height: CelestinaTheme.spaceXs
                                radius: CelestinaTheme.radiusPill
                                color: CelestinaTheme.badgeFill

                                Rectangle {
                                    width: parent.width * Math.max(0, Math.min(1, row.rowData.share))
                                    height: parent.height
                                    radius: parent.radius
                                    color: CelestinaTheme.glyphAccentAmber
                                }
                            }
                        }

                        Text {
                            id: usage

                            anchors.verticalCenter: parent.verticalCenter
                            text: row.rowData.usage
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontRowSecondary
                            font.features: CelestinaTheme.fontFeaturesTabular
                        }
                    }
                }
            }
        }

        CelestinaScrollBar {
            surface: list
            anchors.right: list.right
            anchors.top: list.top
            anchors.bottom: list.bottom
        }
    }
}
