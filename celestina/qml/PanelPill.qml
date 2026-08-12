// PANEL-1. The piece of glass one reading sits on.
//
// The complete bar now owns one compositor backdrop from edge to edge. A pill
// therefore paints only the canonical dense content material over that shared
// sample; it does not publish a second blur region. The material is matte and
// shadowless and the capsule remains inset from every screen edge.
//
// It is a *background*, not a wrapper: it paints behind the widget it is placed
// in and takes no part in the layout, so no flank changes width and nothing on
// the bar moves. It grows only into the gap the row already had, and upward
// into the one above it, which is what welds it to the screen's top edge.
// Deliberately tight — a bar of fat capsules is a bar that has grown a second
// chrome.
//
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "EdgeAttachedGeometry.js" as EdgeAttachedGeometry

Item {
    id: pill

    required property BackdropInk ink
    required property bool blurAvailable
    // Panel readings use a small flank overhang; menu fields own their complete
    // rounded region and do not use this panel-specific extension.
    property int horizontalOverhang: CelestinaTheme.spaceSm
    // A reading hangs from the screen's top edge instead of floating on a
    // plate inside the bar: the capsule grows up to that edge, squares off
    // where it meets it and keeps every other corner round.
    //
    // The bar states its own height for that, rather than the pill reading it
    // off whatever window it happens to be in. Deriving it from the window was
    // tried and is wrong: a pill constructed outside the panel — every
    // offscreen case does exactly that — inherited the test window's height
    // and hung its capsule tens of pixels above its reading. Zero, the
    // default, keeps the floating capsule this component has always drawn, so
    // only a real bar welds anything.
    property real barHeight: 0
    readonly property real weldExtension: pill.barHeight > 0 && pill.parent
            ? Math.max(0, (pill.barHeight - pill.parent.height) / 2)
            : 0
    // Only the centred reading is held by a visibly elastic skin. A flanked
    // capsule keeps straight sides: its neighbours sit at a fixed distance,
    // and widening it at the edge made adjacent readings overlap. Widening the
    // gaps instead would have moved every reading on the bar to decorate one.
    property bool elasticWeld: false
    // How much wider the capsule is where the edge grips it. Tied to the
    // distance it climbs, so a bar of another height keeps the same stretch.
    // Zero is the straight side, which the same curve degenerates to exactly.
    readonly property real weldFlare: pill.elasticWeld
                                      ? pill.weldExtension * 2.6 : 0
    readonly property var weldedSilhouette: pill.weldExtension > 0
            ? EdgeAttachedGeometry.topWeldedCapsule(
                  pill.width, pill.height, CelestinaTheme.radiusPill,
                  pill.weldFlare)
            : ({"path": "", "edgePath": ""})
    // The flare is painted beyond the capsule's own body, so the item carries
    // it on both sides and the body stays centred on the reading behind it.
    readonly property real restingWidth:
            (parent ? parent.width : 0) + pill.horizontalOverhang * 2
            + pill.weldFlare * 2
    readonly property real restingHeight: CelestinaTheme.controlHeightXs
                                          + pill.weldExtension
    readonly property real restingX: -pill.horizontalOverhang
    readonly property real restingY:
            parent
            ? (parent.height - CelestinaTheme.controlHeightXs) / 2
              - pill.weldExtension
            : 0
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
        silhouettePath: pill.weldedSilhouette.path
        silhouetteEdgePath: pill.weldedSilhouette.edgePath
        elevation: 0
    }
}
