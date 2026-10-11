pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Shapes
import org.celestina.calcita 1.0

// One freehand drawing while editing, drawn by the window over the page
// (the copy QtPdf renders has it hidden): each stroke in its colour and
// width, round at its ends and joins as the saved appearance draws it.
// Selected, it shows its frame; dragged, it moves.
Item {
    id: drawing

    // The annotation as the editor publishes it.
    required property var item
    required property real renderScale
    required property bool selected
    // «Seleccionar» is armed: the drawing moves and is selected.
    required property bool interactive

    signal selectRequested()
    // The drawing should fill this rectangle, in points.
    signal moveRequested(real x, real y, real width, real height)

    property real shiftX: 0
    property real shiftY: 0

    // Each stroke as a run of points.
    readonly property var strokes: drawing.item.shape.map(
                                       flat => Array.from({ "length": flat.length / 2 },
                                                          (unused, at) => Qt.point(flat[at * 2], flat[at * 2 + 1])))

    objectName: "ink_" + drawing.item.id
    x: (drawing.item.x + drawing.shiftX) * drawing.renderScale
    y: (drawing.item.y + drawing.shiftY) * drawing.renderScale
    width: drawing.item.w * drawing.renderScale
    height: drawing.item.h * drawing.renderScale
    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Dibujo")
    Accessible.selectable: true
    Accessible.selected: drawing.selected
    Accessible.onPressAction: drawing.selectRequested()

    Shape {
        // The strokes are in the page's points: drawn from the page's
        // origin.
        x: -drawing.item.x * drawing.renderScale
        y: -drawing.item.y * drawing.renderScale

        ShapePath {
            strokeColor: Qt.color(drawing.item.color)
            strokeWidth: drawing.item.size * drawing.renderScale
            fillColor: CelestinaTheme.clear
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            scale: Qt.size(drawing.renderScale, drawing.renderScale)
            PathMultiline {
                paths: drawing.strokes
            }
        }
    }

    Rectangle {
        objectName: "inkFrame"
        anchors.fill: parent
        visible: drawing.selected
        color: CelestinaTheme.clear
        radius: CelestinaTheme.radiusNone
        border.width: CelestinaTheme.borderFocus
        border.color: CelestinaTheme.focusRing
    }

    DragHandler {
        id: mover
        enabled: drawing.interactive
        target: null
        onCentroidChanged: {
            if (!mover.active)
                return
            drawing.shiftX = (mover.centroid.scenePosition.x - mover.centroid.scenePressPosition.x) / drawing.renderScale
            drawing.shiftY = (mover.centroid.scenePosition.y - mover.centroid.scenePressPosition.y) / drawing.renderScale
        }
        onActiveChanged: {
            if (mover.active) {
                drawing.selectRequested()
                return
            }
            const x = drawing.item.x + drawing.shiftX
            const y = drawing.item.y + drawing.shiftY
            drawing.shiftX = 0
            drawing.shiftY = 0
            drawing.moveRequested(x, y, drawing.item.w, drawing.item.h)
        }
    }

    TapHandler {
        enabled: drawing.interactive
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onTapped: drawing.selectRequested()
    }
}
