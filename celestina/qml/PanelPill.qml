// PANEL-1. The piece of glass one reading sits on.
//
// The bar itself has no full-width paint, which lets it disappear into the
// wallpaper — and takes away any shared contrast floor. On a bright picture
// the panel's own text once fell to 4.2:1 and its quiet text to 1.1:1.
//
// A pill gives the compositor blur somewhere finite to stop, then places the
// canonical dense content material over that result. The material is matte and
// shadowless: it establishes the requested information-bearing surface without
// recreating a full-width panel plate or attempting an in-scene capture.
//
// It is a *background*, not a wrapper: it paints behind the widget it is placed
// in and takes no part in the layout, so no flank changes width and nothing on
// the bar moves. It grows only into the gap the row already had. Deliberately
// tight — a bar of fat capsules is a bar that has grown a second chrome.
//
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

CompositorGlassRegion {
    id: pill

    required property BackdropInk ink
    // Panel readings use a small flank overhang; menu fields own their complete
    // rounded region and do not use this panel-specific extension.
    property int horizontalOverhang: CelestinaTheme.spaceSm

    // Behind the reading it belongs to. A child is drawn above its parent's own
    // content unless it says otherwise, and this one is a floor.
    anchors.centerIn: parent
    // Tight on purpose, and tighter than it looks: the parents clip, so an
    // overhang wider than the room the row leaves is simply cut off — which is
    // what sliced the phone's glass in half at the first attempt.
    width: parent.width + pill.horizontalOverhang * 2
    height: CelestinaTheme.controlHeightXs
    radius: CelestinaTheme.radiusPill
    // GlassSurface owns the fallback so a missing compositor sample receives
    // one readable floor instead of this region and the material painting two.
    fallbackColor: CelestinaTheme.clear

    GlassSurface {
        anchors.fill: parent
        visible: pill.visible
        objectName: "celestina-panel-pill-material"
        backdropMode: GlassSurface.ExternalBackdrop
        externalBackdropReady: pill.blurAvailable
        captureEnabled: false
        materialRole: GlassSurface.ContentSurface
        materialTint: pill.ink.contentMaterialTint
        cornerRadius: pill.radius
        elevation: 0
    }
}
