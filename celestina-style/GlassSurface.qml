import QtQuick
import QtQuick.Effects
import QtQuick.Shapes

// ─── GlassSurface ─────────────────────────────────────────────────────────────
// Frosted-glass surface that blurs the real content behind it. The consumer
// injects `backdropSource` (the item to sample); capture is bounded to this
// surface's rectangle. `liveCapture` decides whether the blur tracks what is
// behind it frame by frame or is snapshotted once when shown; a one-shot
// capture is cheaper but freezes, so it reads as a blurred screenshot the
// moment anything behind it moves. Nothing renders while hidden either way.
// Falls back to a translucent tint if it cannot capture.
//
// Recipe (One UI 8.5, DESIGN §6.5): bounded capture → pyramid blur → *slight
// desaturation* + tint/dim → a thin dark outline for definition → the lit
// top-edge glow. `elevation > 0` adds the L2 drop shadow (a floating layer
// stops pasting and starts floating). The shadow lives outside the clipped
// body, so the root itself does not clip.
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
    // Elevation level (DESIGN §6.4): 0 = flush (grouped card, separation by
    // grouping), 2 = floating (menu, tooltip, pill, toast) → L2 drop shadow.
    // Modals (L3) use a scrim behind, never a shadow, so they stay at 0.
    property int elevation: 0
    default property alias contentData: foreground.data

    readonly property bool active: captureEnabled
                                   && backdropSource !== null
                                   && width > 0
                                   && height > 0

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

    // A moving surface (a popup being positioned) re-arms the sample from its
    // consumer, on the event that moved it — NOT every frame. An earlier
    // self-tracking FrameAnimation re-sampled on the GUI thread each frame while
    // live, which starved input and pinned the CPU even at idle; the wiring below
    // (GlassContextMenu / GlassCard) is cheaper and does the same job.

    // L2 drop shadow, behind the body and outside its clip. RectangularShadow is
    // an analytic SDF (Qt 6.9+) — far cheaper than a MultiEffect shadow and it
    // extends past the rect, which is why the root must not clip.
    RectangularShadow {
        anchors.fill: body
        visible: root.elevation > 0
        radius: root.cornerRadius
        blur: CelestinaTheme.shadowBlur
        spread: CelestinaTheme.shadowSpread
        offset.y: CelestinaTheme.shadowOffsetY
        color: CelestinaTheme.shadow
    }

    // The glass itself, clipped to the rounded rectangle. Everything visible
    // lives here so the shadow above can spill while the content cannot.
    Item {
        id: body
        anchors.fill: parent
        clip: true

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
                // Slight desaturation of the backdrop — the 8.5 recipe, not the
                // earlier saturation boost (a negative value here desaturates).
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

        // Fine noise dither over the blur — breaks the banding the downsample
        // pyramid leaves. Tiled at a low opacity; the body clip keeps it inside.
        Image {
            anchors.fill: parent
            visible: root.active
            source: "qrc:/qt/qml/CelestinaStyle/icons/glass-noise.png"
            fillMode: Image.Tile
            opacity: CelestinaTheme.glassNoiseOpacity
            smooth: false
        }

        // A thin dark outline (dark outside) — gives the pane an edge against a
        // light backdrop where the lit glow alone would wash out. One UI's glass
        // wears both: a dark hairline and the lit top edge.
        Rectangle {
            anchors.fill: parent
            radius: root.cornerRadius
            color: "transparent"
            border.width: 1
            border.color: CelestinaTheme.glassOutline
        }

        // The lit glass edge: a rounded-rect gradient stroke, brightest along the
        // top and fading to nothing at the bottom, so the pane catches light like
        // real glass instead of wearing a flat box border. A GPU Shape
        // (CurveRenderer) fills a ~1.3px ring — two rounded PathRectangles under
        // an odd-even fill — with a vertical gradient. Replaces the CPU Canvas,
        // which re-rastered the whole edge on every resize (DESIGN §6.5).
        Shape {
            anchors.fill: parent
            visible: root.active
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                fillRule: ShapePath.OddEvenFill
                strokeWidth: 0
                fillColor: "transparent"
                fillGradient: LinearGradient {
                    x1: 0
                    y1: 0
                    x2: 0
                    y2: root.height
                    GradientStop { position: 0.0; color: Qt.rgba(CelestinaTheme.glassBorder.r, CelestinaTheme.glassBorder.g, CelestinaTheme.glassBorder.b, 0.5) }
                    GradientStop { position: 0.35; color: Qt.rgba(CelestinaTheme.glassBorder.r, CelestinaTheme.glassBorder.g, CelestinaTheme.glassBorder.b, 0.16) }
                    GradientStop { position: 0.7; color: Qt.rgba(CelestinaTheme.glassBorder.r, CelestinaTheme.glassBorder.g, CelestinaTheme.glassBorder.b, 0.05) }
                    GradientStop { position: 1.0; color: Qt.rgba(CelestinaTheme.glassBorder.r, CelestinaTheme.glassBorder.g, CelestinaTheme.glassBorder.b, 0.0) }
                }
                PathRectangle {
                    x: 0
                    y: 0
                    width: root.width
                    height: root.height
                    radius: root.cornerRadius
                }
                PathRectangle {
                    x: 1.3
                    y: 1.3
                    width: root.width - 2.6
                    height: root.height - 2.6
                    radius: Math.max(0, root.cornerRadius - 1.3)
                }
            }
        }

        Item {
            id: foreground
            anchors.fill: parent
        }
    }
}
