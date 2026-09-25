pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls

// A folder drawn as rectangles whose areas are its children's sizes. The host
// lays the tiles out (0..1 of the field) and owns every meaning beyond
// navigation: which ids are current, marked or dimmed, and what entering or
// going up does. Each tile is a button: a click chooses it, a double click
// enters it. The map is one Tab stop; the arrows walk the tiles in reading
// order, Enter enters, Backspace asks to go up. Space and Delete are left
// unaccepted so they reach the host. A negative id is the merged remainder,
// drawn disabled and named "otros".
CelestinaSurface {
    id: map

    // Each tile: { id: int, x, y, w, h: real (0..1), name: string,
    // kind: "dir"|"file"|"other", tone: string }; id < 0 is the remainder.
    required property var tiles
    // The colour each tone paints, supplied by the consumer.
    required property var toneColors
    required property int currentId
    required property string currentName
    // Ids drawn selected, and ids drawn at the unavailable opacity; -1 in
    // `dimmedIds` dims the remainder.
    property var markedIds: []
    property var dimmedIds: []

    signal entered(int id)
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
        Accessible.name: qsTr("Mapa de ocupación de %1").arg(map.currentName)

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
                readonly property bool marked: map.markedIds.indexOf(tile.tileData.id) >= 0
                readonly property bool current: !tile.remainder && tile.tileData.id === map.currentId

                x: tile.tileData.x * field.width + CelestinaTheme.spaceXs / 2
                y: tile.tileData.y * field.height + CelestinaTheme.spaceXs / 2
                width: Math.max(0, tile.tileData.w * field.width - CelestinaTheme.spaceXs)
                height: Math.max(0, tile.tileData.h * field.height - CelestinaTheme.spaceXs)
                hoverEnabled: true
                focusPolicy: Qt.NoFocus
                enabled: !tile.remainder
                opacity: map.dimmedIds.indexOf(tile.tileData.id) >= 0 ? CelestinaTheme.unavailableContentOpacity : 1

                Accessible.role: Accessible.Button
                Accessible.name: tile.remainder ? qsTr("otros")
                                 : tile.tileData.kind === "dir" ? qsTr("%1, carpeta").arg(tile.tileData.name)
                                 : qsTr("%1, archivo").arg(tile.tileData.name)
                Accessible.description: tile.tileData.name
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
