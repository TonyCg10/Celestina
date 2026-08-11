// PANEL-1. One finite region whose material comes from the compositor.
//
// Pills and larger menu cards share the same blur/fallback contract but not
// the same geometry. This component owns that material and region lifecycle;
// each consumer supplies the size and radius that describe its content.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Rectangle {
    id: region

    required property bool blurAvailable
    property color fallbackColor: CelestinaTheme.glassTintStrong

    signal blurRegionChanged()

    // Both panel and menu collectors publish these exact rectangles to Niri.
    objectName: "celestina-compositor-glass-region"
    z: -1
    color: region.blurAvailable
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
    onVisibleChanged: region.blurRegionChanged()

    Connections {
        target: region.parent

        function onXChanged() { region.blurRegionChanged(); }
        function onYChanged() { region.blurRegionChanged(); }
        function onWidthChanged() { region.blurRegionChanged(); }
        function onHeightChanged() { region.blurRegionChanged(); }
        function onVisibleChanged() { region.blurRegionChanged(); }
    }
}
