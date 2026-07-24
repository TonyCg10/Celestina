import QtQuick
import QtQuick.Effects

// ─── GlassSurface ─────────────────────────────────────────────────────────────
// Frosted-glass surface that blurs the real content behind it. The consumer
// injects `backdropSource` (the item to sample); capture is bounded to this
// surface's rectangle. By default it is snapshotted once when shown, with no
// continuous rendering when hidden — right for a transient menu. Set
// `liveCapture` for a surface that content can move behind (a modal you can
// scroll under), so the blur tracks it live while shown. Falls back to a
// translucent tint if it cannot capture.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    required property Item backdropSource
    property bool captureEnabled: true
    // One-shot snapshot on show (false) vs continuous re-capture while shown
    // (true). Live tracks content moving behind the surface at a GPU cost, so
    // reserve it for modals; menus stay one-shot.
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
        if (!active)
            capture.sourceRect = Qt.rect(0, 0, 0, 0)
    }

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
        border.width: 1
        border.color: CelestinaTheme.glassBorder
    }

    // The One UI top-edge specular — a bright hairline just inside the outline
    // that sells the "piece of glass". Inset so it clears the rounded corners.
    Rectangle {
        x: root.cornerRadius
        y: 1
        width: parent.width - root.cornerRadius * 2
        height: 1
        color: CelestinaTheme.glassHighlight
        visible: root.active
    }

    Item {
        id: foreground
        anchors.fill: parent
    }
}
