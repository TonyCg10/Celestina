// PANEL-1. One finite region whose material comes from the compositor.
//
// The continuous panel veil and larger menu fields share the same
// blur/fallback contract but not the same geometry. Dense panel capsules are
// paint-only materials over the panel's sample. This component owns the
// compositor region lifecycle; each finite surface supplies its geometry.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Shapes

Rectangle {
    id: region

    required property bool blurAvailable
    property color fallbackColor: CelestinaTheme.glassTintStrong
    // Optional vector/polygon pair from EdgeAttachedGeometry. The path paints
    // the no-compositor fallback; the polygon is the same boundary sampled for
    // KWindowEffects. Empty values retain the rounded Rectangle contract.
    property string silhouettePath: ""
    property var polygon: []
    readonly property bool usesSilhouette: silhouettePath.length > 0

    signal blurRegionChanged()

    // Both panel and menu collectors publish these exact rectangles to Niri.
    objectName: "celestina-compositor-glass-region"
    z: -1
    color: region.usesSilhouette || region.blurAvailable
           ? CelestinaTheme.clear
           : region.fallbackColor
    border.width: 0
    border.color: CelestinaTheme.clear
    visible: region.parent !== null
             && region.parent.visible
             && region.width > 0
             && region.height > 0

    Component.onCompleted: region.blurRegionChanged()
    onXChanged: region.blurRegionChanged()
    onYChanged: region.blurRegionChanged()
    onWidthChanged: region.blurRegionChanged()
    onHeightChanged: region.blurRegionChanged()
    onRadiusChanged: region.blurRegionChanged()
    onPolygonChanged: region.blurRegionChanged()
    onSilhouettePathChanged: region.blurRegionChanged()
    onVisibleChanged: region.blurRegionChanged()

    Shape {
        objectName: "celestina-compositor-glass-fallback-shape"
        anchors.fill: parent
        visible: region.usesSilhouette && !region.blurAvailable
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeWidth: 0
            fillColor: region.fallbackColor
            PathSvg { path: region.silhouettePath }
        }
    }

    Connections {
        target: region.parent

        function onXChanged() { region.blurRegionChanged(); }
        function onYChanged() { region.blurRegionChanged(); }
        function onWidthChanged() { region.blurRegionChanged(); }
        function onHeightChanged() { region.blurRegionChanged(); }
        function onVisibleChanged() { region.blurRegionChanged(); }
    }
}
