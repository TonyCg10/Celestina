import QtQuick
import org.celestina.fluorita 1.0

// A still, drawn by the toolkit.
//
// There is no media backend behind this: `Image` is Qt reading the file, which
// is the whole point — looking at a photograph must not start a decoder. The
// budget was already checked in Rust before the source was ever set, so what
// arrives here is either safe to decode or empty.
Item {
    id: view

    required property string source
    // How close, and at what part. Published so the window can put a magnifier
    // beside its other actions without owning the arithmetic.
    readonly property ZoomController zoom: zoomer
    // True only once the toolkit has a decoded picture to draw. "The player has
    // a source" is not the same thing and is what a caller must not confuse it
    // with: between the two there is a decode, and anyone who hands over at the
    // first one shows an empty rectangle for its duration.
    readonly property bool presented: picture.status === Image.Ready
    // What the window may show at most. A scaled read of a large photograph is
    // cheap where the format allows it, and never larger than the surface.
    readonly property int decodeCap: Math.max(1, Math.ceil(
        Math.max(view.width, view.height) * Screen.devicePixelRatio * view.decodeZoom))

    // The zoom the *decode* is sized for, which is not the zoom being drawn.
    //
    // Enlarging a picture decoded at window size only magnifies the pixels the
    // reader already threw away, so looking closer has to ask for more of them.
    // It follows the level in steps and settles rather than tracking it: a
    // re-read on every notch of the wheel would decode the same photograph a
    // dozen times on the way to where the person was going.
    property real decodeZoom: 1.0
    readonly property real maximumDecodeZoom: 4.0

    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Imagen")

    // What the zoom is measured against, and what it is clipped by.
    Item {
        id: viewport

        anchors.fill: parent
        clip: true

        Image {
            id: picture

            width: viewport.width
            height: viewport.height
            x: view.zoom.panX
            y: view.zoom.panY
            scale: view.zoom.level
            transformOrigin: Item.Center
            source: view.source
            // Decoding on the GUI thread would freeze the window on a large file.
            asynchronous: true
            // Honour the camera's orientation; a portrait photograph that arrives
            // sideways is the classic sign this was forgotten.
            autoTransform: true
            fillMode: Image.PreserveAspectFit
            // Cap what is decoded rather than what is drawn: `sourceSize` is what
            // makes the reader do a scaled read instead of allocating the full
            // surface first.
            sourceSize.width: view.decodeCap
            sourceSize.height: view.decodeCap
            // A still has no motion of its own; nothing here animates, so there is
            // nothing for reduced motion to turn off.
            cache: false
            visible: picture.status === Image.Ready
    }

    // Ctrl and the wheel, at the pointer. Without the modifier the wheel keeps
    // whatever meaning the surface around this already gave it.
    WheelHandler {
        acceptedModifiers: Qt.ControlModifier
        onWheel: function(event) {
            view.zoom.byWheel(event.angleDelta.y, event.x, event.y)
            settleDecode.restart()
        }
    }

    // Dragging moves the picture, but only when there is somewhere to move it.
    DragHandler {
        enabled: view.zoom.zoomed
        cursorShape: Qt.ClosedHandCursor
        target: null
        property real lastX: 0
        property real lastY: 0
        onActiveChanged: {
            lastX = centroid.position.x
            lastY = centroid.position.y
        }
        onCentroidChanged: {
            if (!active) {
                return
            }
            view.zoom.nudge(centroid.position.x - lastX, centroid.position.y - lastY)
            lastX = centroid.position.x
            lastY = centroid.position.y
        }
    }

    TapHandler {
        onDoubleTapped: function(point) {
            if (view.zoom.zoomed) {
                view.zoom.reset()
            } else {
                view.zoom.to(2.0, point.position.x, point.position.y)
            }
            settleDecode.restart()
        }
    }
    }

    ZoomController {
        id: zoomer

        viewportWidth: view.width
        viewportHeight: view.height
        // The *painted* size, not the item's: a letterboxed photograph fills
        // less than its rectangle, and pan limits taken from the rectangle
        // would let a person drag the picture off into the blank beside it.
        contentWidth: picture.paintedWidth
        contentHeight: picture.paintedHeight
    }

    // A new picture is looked at from the start, not from wherever the last one
    // was being examined.
    onSourceChanged: {
        zoomer.reset()
        view.decodeZoom = 1.0
    }

    Timer {
        id: settleDecode

        interval: 250
        onTriggered: view.decodeZoom = Math.min(view.maximumDecodeZoom,
                                                Math.max(1.0, zoomer.level))
    }

    // Loading and failure are states, not blank space.
    CelestinaSectionLabel {
        anchors.centerIn: parent
        visible: picture.status === Image.Loading
        text: qsTr("Cargando…")
    }

    Text {
        anchors.centerIn: parent
        width: Math.min(parent.width - CelestinaTheme.spaceLg * 2, 420)
        visible: picture.status === Image.Error
        text: qsTr("El sistema no pudo decodificar esta imagen")
        color: CelestinaTheme.danger
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
}
