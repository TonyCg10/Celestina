import QtQuick
import QtQuick.Effects

// ─── GlassSurface ─────────────────────────────────────────────────────────────
// Frosted-glass surface that blurs the real content behind it. The consumer
// injects `backdropSource` (the item to sample); capture is bounded to this
// surface's rectangle. `liveCapture` decides whether the blur tracks what is
// behind it frame by frame or is snapshotted once when shown; a one-shot
// capture is cheaper but freezes, so it reads as a blurred screenshot the
// moment anything behind it moves. Nothing renders while hidden either way.
// Falls back to a translucent tint if it cannot capture.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    required property Item backdropSource
    property bool captureEnabled: true
    // One-shot snapshot on show (false) vs continuous re-capture while shown
    // (true). Live costs a small per-frame render, but it is what makes the
    // surface read as glass rather than as a frozen picture of the moment it
    // opened, so anything the user can scroll, hover or drag under wants it.
    property bool liveCapture: false
    property real cornerRadius: CelestinaTheme.radiusMd
    property int sampleMargin: CelestinaTheme.glassSampleMargin
    property real sampleScale: CelestinaTheme.glassSampleScale
    default property alias contentData: foreground.data

    readonly property bool active: captureEnabled
                                   && backdropSource !== null
                                   && width > 0
                                   && height > 0

    clip: true

    function refreshBackdrop() {
        if (!active)
            return

        const point = sampleLayer.mapToItem(backdropSource, 0, 0)
        capture.sourceRect = Qt.rect(point.x, point.y,
                                     sampleLayer.width, sampleLayer.height)
        capture.scheduleUpdate()
    }

    onActiveChanged: {
        if (active)
            refreshBackdrop()
        else
            capture.sourceRect = Qt.rect(0, 0, 0, 0)
    }

    // The sampled region has to follow the surface. Its size can still change
    // after it is shown — a menu grows as its items decide to be visible — and
    // a sourceRect left at the old size shows a stretched, wrong-looking region
    // instead of what is actually behind. A consumer that *moves* the surface
    // (a popup being positioned) re-arms this by calling refreshBackdrop().
    onWidthChanged: refreshBackdrop()
    onHeightChanged: refreshBackdrop()

    Rectangle {
        anchors.fill: parent
        radius: root.cornerRadius
        color: CelestinaTheme.surfaceStrong
    }

    Item {
        id: sampleLayer
        x: -root.sampleMargin
        y: -root.sampleMargin
        width: root.width + root.sampleMargin * 2
        height: root.height + root.sampleMargin * 2
        visible: root.active

        ShaderEffectSource {
            id: capture
            anchors.fill: parent
            sourceItem: root.active ? root.backdropSource : null
            sourceRect: Qt.rect(0, 0, 0, 0)
            textureSize: Qt.size(
                Math.max(1, Math.ceil(width * root.sampleScale)),
                Math.max(1, Math.ceil(height * root.sampleScale)))
            live: root.liveCapture
            recursive: false
            hideSource: false
            smooth: true
            visible: false
        }

        Item {
            id: roundedMask
            anchors.fill: parent
            visible: false
            layer.enabled: true

            Rectangle {
                x: root.sampleMargin
                y: root.sampleMargin
                width: root.width
                height: root.height
                radius: root.cornerRadius
                color: "white"
            }
        }

        MultiEffect {
            anchors.fill: parent
            source: capture
            visible: root.active
            blurEnabled: true
            blur: CelestinaTheme.glassBlur
            blurMax: CelestinaTheme.glassBlurMax
            saturation: CelestinaTheme.glassSaturation
            autoPaddingEnabled: false
            maskEnabled: true
            maskSource: roundedMask
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: root.cornerRadius
        color: root.active ? CelestinaTheme.glassTint : CelestinaTheme.surfaceStrong
    }

    // The "lit glass edge": a rounded-rect stroke with a vertical gradient —
    // brightest along the top, fading down the sides, gone at the bottom — so
    // the surface catches light like a real pane instead of wearing a flat box
    // border. A Canvas, because a Rectangle border cannot be a gradient.
    Canvas {
        id: edge
        anchors.fill: parent
        visible: root.active
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        Component.onCompleted: requestPaint()
        Connections {
            target: root
            function onCornerRadiusChanged() { edge.requestPaint() }
        }
        onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            var lw = 1.3
            var w = width - lw, h = height - lw
            if (w <= 0 || h <= 0)
                return
            var b = CelestinaTheme.glassBorder
            var g = ctx.createLinearGradient(0, 0, 0, height)
            g.addColorStop(0.0, Qt.rgba(b.r, b.g, b.b, 0.44))   // lit top
            g.addColorStop(0.35, Qt.rgba(b.r, b.g, b.b, 0.14))
            g.addColorStop(0.7, Qt.rgba(b.r, b.g, b.b, 0.04))
            g.addColorStop(1.0, Qt.rgba(b.r, b.g, b.b, 0.0))    // dark bottom
            ctx.strokeStyle = g
            ctx.lineWidth = lw
            var r = Math.max(0, root.cornerRadius - lw / 2)
            var x = lw / 2, y = lw / 2
            ctx.beginPath()
            ctx.moveTo(x + r, y)
            ctx.arcTo(x + w, y, x + w, y + h, r)
            ctx.arcTo(x + w, y + h, x, y + h, r)
            ctx.arcTo(x, y + h, x, y, r)
            ctx.arcTo(x, y, x + w, y, r)
            ctx.closePath()
            ctx.stroke()
        }
    }

    Item {
        id: foreground
        anchors.fill: parent
    }
}
