pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Shapes
import org.celestina.calcita 1.0

// One highlight while editing, drawn by the window over the page (the copy
// QtPdf renders has it hidden): each quadrilateral in its colour, light
// enough that the text under it reads, as the saved multiply appearance
// does. Selected, it shows its frame; dragged, it moves.
Item {
    id: mark

    // The annotation as the editor publishes it.
    required property var item
    required property real renderScale
    required property bool selected
    // «Seleccionar» is armed: the highlight moves and is selected.
    required property bool interactive

    signal selectRequested()
    // The highlight should fill this rectangle, in points.
    signal moveRequested(real x, real y, real width, real height)

    property real shiftX: 0
    property real shiftY: 0

    // Each quadrilateral as a closed outline of points.
    readonly property var outlines: mark.item.shape.map(flat => [0, 1, 2, 3, 0].map(
                                                            at => Qt.point(flat[at * 2], flat[at * 2 + 1])))

    objectName: "highlight_" + mark.item.id
    x: (mark.item.x + mark.shiftX) * mark.renderScale
    y: (mark.item.y + mark.shiftY) * mark.renderScale
    width: mark.item.w * mark.renderScale
    height: mark.item.h * mark.renderScale
    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Resaltado")
    Accessible.selectable: true
    Accessible.selected: mark.selected
    Accessible.onPressAction: mark.selectRequested()

    Shape {
        // The outlines are in the page's points: drawn from the page's
        // origin.
        x: -mark.item.x * mark.renderScale
        y: -mark.item.y * mark.renderScale

        ShapePath {
            strokeWidth: -1
            fillColor: CelestinaTheme.withAlpha(Qt.color(mark.item.color), CelestinaTheme.usageTileOpacity)
            scale: Qt.size(mark.renderScale, mark.renderScale)
            PathMultiline {
                paths: mark.outlines
            }
        }
    }

    Rectangle {
        objectName: "highlightFrame"
        anchors.fill: parent
        visible: mark.selected
        color: CelestinaTheme.clear
        radius: CelestinaTheme.radiusNone
        border.width: CelestinaTheme.borderFocus
        border.color: CelestinaTheme.focusRing
    }

    DragHandler {
        id: mover
        enabled: mark.interactive
        target: null
        onCentroidChanged: {
            if (!mover.active)
                return
            mark.shiftX = (mover.centroid.scenePosition.x - mover.centroid.scenePressPosition.x) / mark.renderScale
            mark.shiftY = (mover.centroid.scenePosition.y - mover.centroid.scenePressPosition.y) / mark.renderScale
        }
        onActiveChanged: {
            if (mover.active) {
                mark.selectRequested()
                return
            }
            const x = mark.item.x + mark.shiftX
            const y = mark.item.y + mark.shiftY
            mark.shiftX = 0
            mark.shiftY = 0
            mark.moveRequested(x, y, mark.item.w, mark.item.h)
        }
    }

    TapHandler {
        enabled: mark.interactive
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onTapped: mark.selectRequested()
    }
}
