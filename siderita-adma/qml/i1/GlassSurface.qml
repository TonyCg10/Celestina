import QtQuick
import QtQuick.Effects
import org.celestina.siderita 1.0

Item {
    id: root

    required property Item backdropSource
    property bool captureEnabled: true
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
            live: false
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
        border.color: CelestinaTheme.borderStrong
    }

    Item {
        id: foreground
        anchors.fill: parent
    }
}
