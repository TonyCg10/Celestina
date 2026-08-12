// PANEL-1. The piece of glass one reading sits on.
//
// The complete bar now owns one compositor backdrop from edge to edge. A pill
// therefore paints only the canonical dense content material over that shared
// sample; it does not publish a second blur region. The material is matte and
// shadowless and the capsule remains inset from every screen edge.
//
// It is a *background*, not a wrapper: it paints behind the widget it is placed
// in and takes no part in the layout, so no flank changes width and nothing on
// the bar moves. It grows only into the gap the row already had. Deliberately
// tight — a bar of fat capsules is a bar that has grown a second chrome.
//
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: pill

    required property BackdropInk ink
    required property bool blurAvailable
    // Panel readings use a small flank overhang; menu fields own their complete
    // rounded region and do not use this panel-specific extension.
    property int horizontalOverhang: CelestinaTheme.spaceSm
    readonly property real restingWidth:
            (parent ? parent.width : 0) + pill.horizontalOverhang * 2
    readonly property real restingHeight: CelestinaTheme.controlHeightXs
    readonly property real restingX: -pill.horizontalOverhang
    readonly property real restingY:
            parent ? (parent.height - pill.restingHeight) / 2 : 0
    // Behind the reading it belongs to. A child is drawn above its parent's own
    // content unless it says otherwise, and this one is a floor.
    anchors.horizontalCenter: parent ? parent.horizontalCenter : undefined
    z: -1
    // Tight on purpose, and tighter than it looks: the parents clip, so an
    // overhang wider than the room the row leaves is simply cut off — which is
    // what sliced the phone's glass in half at the first attempt.
    y: pill.restingY
    width: pill.restingWidth
    height: pill.restingHeight

    GlassSurface {
        anchors.fill: parent
        visible: pill.visible
        objectName: "celestina-panel-pill-material"
        backdropMode: GlassSurface.ExternalBackdrop
        externalBackdropReady: pill.blurAvailable
        captureEnabled: false
        materialRole: GlassSurface.ContentSurface
        materialTint: pill.ink.contentMaterialTint
        cornerRadius: CelestinaTheme.radiusPill
        elevation: 0
    }
}
