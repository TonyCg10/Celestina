pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// The analysed folder as rectangles whose areas are the children's sizes,
// laid out in Rust. Each tile is a button: a click makes it current, a double
// click enters a folder. The map is one Tab stop; the arrows walk the tiles
// in reading order, Space selects, Enter enters, Backspace goes up. Tiles the
// active filter does not keep fade; the merged remainder says "otros".
CelestinaSurface {
    id: map

    // Each tile: { id, x, y, w, h (0..1), name, tone, matches }; id -1 is the
    // remainder.
    required property var tiles
    required property var selectedIds
    required property var toneColors
    required property int currentId
    required property string currentName

    signal entered(int id)
    signal toggled(int id)
    signal chosen(int id)
    signal upRequested()

    // Tile indexes in reading order: top to bottom, then left to right.
    readonly property var order: {
        const indexes = []
        for (let index = 0; index < map.tiles.length; ++index)
            indexes.push(index)
        indexes.sort(function(a, b) {
            const dy = map.tiles[a].y - map.tiles[b].y
            return Math.abs(dy) > 1e-9 ? dy : map.tiles[a].x - map.tiles[b].x
        })
        return indexes
    }

    function step(delta) {
        if (map.order.length === 0)
            return
        let at = -1
        for (let position = 0; position < map.order.length; ++position) {
            if (map.tiles[map.order[position]].id === map.currentId)
                at = position
        }
        const next = at < 0 ? 0 : Math.max(0, Math.min(map.order.length - 1, at + delta))
        map.chosen(map.tiles[map.order[next]].id)
    }

    role: CelestinaSurface.Panel
    padding: CelestinaTheme.spaceSm

    contentItem: Item {
        id: field

        activeFocusOnTab: true
        Accessible.role: Accessible.Grouping
        Accessible.name: qsTr("Mapa de %1").arg(map.currentName)

        Keys.onPressed: function(event) {
            switch (event.key) {
            case Qt.Key_Left:
            case Qt.Key_Up:
                map.step(-1)
                break
            case Qt.Key_Right:
            case Qt.Key_Down:
                map.step(1)
                break
            case Qt.Key_Space:
                if (map.currentId >= 0)
                    map.toggled(map.currentId)
                break
            case Qt.Key_Return:
            case Qt.Key_Enter:
                if (map.currentId >= 0)
                    map.entered(map.currentId)
                break
            case Qt.Key_Backspace:
                map.upRequested()
                break
            default:
                return
            }
            event.accepted = true
        }

        Repeater {
            model: map.tiles.length

            delegate: AbstractButton {
                id: tile

                required property int index

                readonly property var tileData: map.tiles[tile.index]
                readonly property bool remainder: tile.tileData.id < 0
                readonly property bool marked: map.selectedIds.indexOf(tile.tileData.id) >= 0
                readonly property bool current: !tile.remainder && tile.tileData.id === map.currentId

                x: tile.tileData.x * field.width + CelestinaTheme.spaceXs / 2
                y: tile.tileData.y * field.height + CelestinaTheme.spaceXs / 2
                width: Math.max(0, tile.tileData.w * field.width - CelestinaTheme.spaceXs)
                height: Math.max(0, tile.tileData.h * field.height - CelestinaTheme.spaceXs)
                hoverEnabled: true
                focusPolicy: Qt.NoFocus
                enabled: !tile.remainder
                opacity: tile.tileData.matches ? 1 : CelestinaTheme.unavailableContentOpacity

                Accessible.role: Accessible.Button
                Accessible.name: tile.remainder ? qsTr("otros") : tile.tileData.name
                Accessible.selected: tile.marked

                onClicked: map.chosen(tile.tileData.id)
                onDoubleClicked: map.entered(tile.tileData.id)

                background: Item {
                    Rectangle {
                        anchors.fill: parent
                        radius: CelestinaTheme.radiusSm
                        color: tile.remainder ? CelestinaTheme.card : map.toneColors[tile.tileData.tone] || CelestinaTheme.textFaint
                        opacity: tile.remainder ? 1 : CelestinaTheme.accentSoftOpacity
                    }

                    Rectangle {
                        anchors.fill: parent
                        radius: CelestinaTheme.radiusSm
                        color: tile.marked || tile.current ? CelestinaTheme.surfaceSelected
                                                           : CelestinaTheme.contentHover
                        visible: tile.marked || tile.current || tile.hovered
                    }
                }

                contentItem: Text {
                    leftPadding: CelestinaTheme.spaceXs
                    rightPadding: CelestinaTheme.spaceXs
                    visible: tile.width > CelestinaTheme.fontCaption * 3
                             && tile.height > CelestinaTheme.fontCaption * 1.5
                    text: tile.remainder ? qsTr("otros") : tile.tileData.name
                    textFormat: Text.PlainText
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignTop
                    topPadding: CelestinaTheme.spaceXs
                }

                CelestinaFocusRing {
                    target: tile
                    cornerRadius: CelestinaTheme.radiusSm
                    shown: field.activeFocus && tile.current
                }
            }
        }
    }
}
