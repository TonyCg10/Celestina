// SIMPLE-2 (final form). The section IS the glass — see DRAWING.md.
//
// The author's contract: menus have NO containing backdrop. The visible
// material lives entirely in these content cards — each one a piece of
// Samsung's colour-summary glass under the elevated tint — and the menu's
// only background is the block's soft shadow, drawn by the field behind
// the group. A containing panel was tried and rejected on sight.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: section

    required property BackdropInk ink
    property real radius: CelestinaTheme.radiusSm

    // The dense collector reads this pair for the strong-blur shape.
    readonly property real cornerRadius: section.radius

    objectName: "celestina-menu-section"
    anchors.fill: parent
    z: -1

    // The region marker, inset two units: the compositor region's rounded
    // corners are integer scanline steps nothing can antialias, and flush
    // with the card they saw-toothed around the tint's smooth corner.
    Item {
        objectName: "celestina-compositor-glass-region"

        anchors.fill: parent
        anchors.margins: 2
        readonly property real radius: section.radius
        readonly property var polygon: []
    }

    Rectangle {
        objectName: "celestina-panel-tint"

        anchors.fill: parent
        radius: section.radius
        antialiasing: true
        // The One UI balance: roughly half tint, half colour summary.
        color: CelestinaTheme.panelTint
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.divider
    }
}
