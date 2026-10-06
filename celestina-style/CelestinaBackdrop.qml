import QtQuick

// Canonical L0 window backdrop: the window is transparent and the compositor
// blurs what lies behind it, so this paints the Haze material over that blur
// — the canvas tint at 0.70 and a light grain — the same recipe GlassSurface
// paints over its in-scene capture. Apps may place restrained decorative
// children inside it, but the material stays suite-owned.
Rectangle {
    color: CelestinaTheme.glassTint

    Image {
        anchors.fill: parent
        source: Qt.resolvedUrl(".").toString().startsWith("file:")
                ? Qt.resolvedUrl("icons/haze-noise.png")
                : "qrc:/qt/qml/CelestinaStyle/icons/haze-noise.png"
        fillMode: Image.Tile
        opacity: CelestinaTheme.glassCanvasNoiseOpacity
        smooth: false
    }
}
