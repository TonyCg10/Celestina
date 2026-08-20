import QtQuick

// Where a picture is looked at from: how close, and at what part of it.
//
// One object because two surfaces need exactly the same arithmetic — the
// viewer and the editor — and zoom that behaves differently in the two would
// be the same defect as a mark that lands where it was not drawn. It holds no
// picture and draws nothing: a host gives it the viewport and the fitted
// content size, and reads back a level and an offset.
//
// **Zoom happens around a point, not around the middle.** Scaling about the
// centre moves whatever you were looking at away from the cursor, so the
// gesture fights the person on every step. Keeping the image point under the
// cursor fixed is the whole difference between a zoom that can be aimed and
// one that has to be chased with a pan afterwards.
QtObject {
    id: zoom

    // How much closer than fitted. 1 is the whole picture in view.
    property real level: 1.0
    // Where the content sits relative to centred, in viewport pixels.
    property real panX: 0
    property real panY: 0

    // The viewport, and the content's drawn size at level 1.
    property real viewportWidth: 0
    property real viewportHeight: 0
    property real contentWidth: 0
    property real contentHeight: 0

    readonly property real minimum: 1.0
    readonly property real maximum: 8.0
    // The step one notch of the wheel takes. Multiplicative, because zoom is
    // perceived in ratios: a fixed increment crawls when close and jumps when
    // far away.
    readonly property real notch: 1.15

    readonly property bool zoomed: zoom.level > zoom.minimum + 0.0001

    // The drawn size at the current level.
    readonly property real drawnWidth: zoom.contentWidth * zoom.level
    readonly property real drawnHeight: zoom.contentHeight * zoom.level

    // Back to the whole picture.
    function reset() {
        zoom.level = zoom.minimum
        zoom.panX = 0
        zoom.panY = 0
    }

    // Zooms to `target`, keeping the content point under (`x`, `y`) — viewport
    // coordinates — where it is.
    function to(target, x, y) {
        const clamped = Math.max(zoom.minimum, Math.min(zoom.maximum, target))
        if (Math.abs(clamped - zoom.level) < 0.0001) {
            return
        }
        const ratio = clamped / zoom.level
        const offsetX = x - zoom.viewportWidth / 2
        const offsetY = y - zoom.viewportHeight / 2
        zoom.panX = offsetX * (1 - ratio) + zoom.panX * ratio
        zoom.panY = offsetY * (1 - ratio) + zoom.panY * ratio
        zoom.level = clamped
        zoom.settle()
    }

    // One wheel notch at a point. `delta` is Qt's angle delta.
    function byWheel(delta, x, y) {
        zoom.to(zoom.level * Math.pow(zoom.notch, delta / 120), x, y)
    }

    // Fit and close-up, on one control: the second press of a magnifier should
    // undo the first rather than go on climbing forever.
    function toggle() {
        if (zoom.zoomed) {
            zoom.reset()
        } else {
            zoom.to(2.0, zoom.viewportWidth / 2, zoom.viewportHeight / 2)
        }
    }

    function nudge(dx, dy) {
        zoom.panX += dx
        zoom.panY += dy
        zoom.settle()
    }

    // Keeps the picture in touch with its viewport. An axis with nothing to
    // spare is centred rather than draggable, which is what stops a picture
    // from being flicked off the screen and left there.
    function settle() {
        const spareX = Math.max(0, (zoom.drawnWidth - zoom.viewportWidth) / 2)
        const spareY = Math.max(0, (zoom.drawnHeight - zoom.viewportHeight) / 2)
        zoom.panX = Math.max(-spareX, Math.min(spareX, zoom.panX))
        zoom.panY = Math.max(-spareY, Math.min(spareY, zoom.panY))
    }

    // A viewport that changed size can leave the offsets past their limits.
    property Connections sizeGuard: Connections {
        target: zoom
        function onViewportWidthChanged() { zoom.settle() }
        function onViewportHeightChanged() { zoom.settle() }
        function onContentWidthChanged() { zoom.settle() }
        function onContentHeightChanged() { zoom.settle() }
    }
}
