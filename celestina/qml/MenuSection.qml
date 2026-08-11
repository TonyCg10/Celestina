// PANEL-1. A denser division inside one compositor-backed menu card.
//
// This is deliberately not another compositor region. The complete menu owns
// one blur sample; sections only establish hierarchy above that shared glass,
// so a menu reads as one object instead of a stack of independent pills.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

GlassSurface {
    id: section

    required property BackdropInk ink
    property real radius: CelestinaTheme.radiusSm

    objectName: "celestina-menu-section"
    anchors.fill: parent
    z: -1
    backdropMode: GlassSurface.ExternalBackdrop
    // SoftMenuField always supplies either one compositor sample or its own
    // readable fallback below every section. This means the external backdrop
    // is materially ready even when KWindowEffects itself is unavailable.
    externalBackdropReady: true
    captureEnabled: false
    materialRole: GlassSurface.ContentSurface
    materialTint: ink.contentMaterialTint
    cornerRadius: radius
    elevation: 0
}
