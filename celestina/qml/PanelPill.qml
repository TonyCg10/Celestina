// PANEL-1. The piece of glass one reading sits on.
//
// The bar itself is a shadow with no edge, which is what lets it disappear into
// the wallpaper — and what took the contrast floor away with it: on a bright
// picture the panel's own text fell to 4.2:1 and its quiet text to 1.1:1.
//
// A pill gives the compositor blur somewhere finite to stop. It deliberately
// draws neither a successful-path fill nor a stroke: either one would sit above
// the compositor result and turn the glass back into dark paint.
//
// It is a *background*, not a wrapper: it paints behind the widget it is placed
// in and takes no part in the layout, so no flank changes width and nothing on
// the bar moves. It grows only into the gap the row already had. Deliberately
// tight — a bar of fat capsules is a bar that has grown a second chrome.
//
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Rectangle {
    id: pill

    required property bool blurAvailable

    signal blurRegionChanged()

    // Found by the panel, which publishes the real rectangles the blur
    // controller gives to the compositor.
    objectName: "celestina-panel-pill"

    // Behind the reading it belongs to. A child is drawn above its parent's own
    // content unless it says otherwise, and this one is a floor.
    z: -1
    anchors.centerIn: parent
    // Tight on purpose, and tighter than it looks: the parents clip, so an
    // overhang wider than the room the row leaves is simply cut off — which is
    // what sliced the phone's glass in half at the first attempt.
    width: parent.width + CelestinaTheme.spaceSm * 2
    height: CelestinaTheme.controlHeightXs
    radius: CelestinaTheme.radiusPill
    // Niri already draws the blurred background under this region. A clear fill
    // is what lets that result remain visible. Only the no-blur path paints a
    // readable floor, and it stays borderless so fallback does not invent a
    // different component anatomy.
    color: pill.blurAvailable
           ? CelestinaTheme.clear
           : CelestinaTheme.compositorGlassFallback
    border.width: 0
    border.color: CelestinaTheme.clear
    // A widget with no reading has no width, and its glass must vanish with it
    // rather than leaving a bubble floating where a reading used to be.
    visible: parent.width > 0 && parent.visible

    Component.onCompleted: pill.blurRegionChanged()
    onXChanged: pill.blurRegionChanged()
    onYChanged: pill.blurRegionChanged()
    onWidthChanged: pill.blurRegionChanged()
    onHeightChanged: pill.blurRegionChanged()
    onVisibleChanged: pill.blurRegionChanged()

    // A Row moves the reading that owns this pill, not the pill inside it.
    // Follow that direct parent so the compositor region moves with the
    // reading when another provider appears, disappears or changes width.
    Connections {
        target: pill.parent

        function onXChanged() { pill.blurRegionChanged(); }
        function onYChanged() { pill.blurRegionChanged(); }
        function onWidthChanged() { pill.blurRegionChanged(); }
        function onHeightChanged() { pill.blurRegionChanged(); }
        function onVisibleChanged() { pill.blurRegionChanged(); }
    }
}
