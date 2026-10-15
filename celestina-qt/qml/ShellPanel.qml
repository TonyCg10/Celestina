// SIMPLE-2. THE surface painter — see DRAWING.md for the contract.
//
// One panel is one piece of Samsung glass: a soft two-layer shadow, a
// single colour-summary region, a single elevated tint, a hairline. The
// bar's capsules are its consumers; contextual surfaces paint the same
// anatomy through `MenuSection`, which is the glass card itself.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: panel

    // The dense channel reads this pair: the collector picks every visible
    // item named this way and blurs the backdrop into colour washes under
    // exactly this rounded rectangle.
    objectName: "celestina-menu-section"
    readonly property real cornerRadius: panel.radius

    property real radius: CelestinaTheme.radiusLg
    // A floating panel casts; a bar capsule does not (the author removed
    // every strip-level shade — the capsules sit flush with the edge).
    property bool castsShadow: true

    CelestinaShadow {
        anchors.fill: parent
        visible: panel.castsShadow
        radius: panel.radius
    }

    // The weak-channel marker. Only the bar's window arms weak blur; on
    // every other carrier this exists to fire the region beat that keeps
    // the dense channel fed. Inset 2: the compositor region's rounded
    // corners are integer scanline steps nothing can antialias, and flush
    // with the panel they saw-toothed around the tint's smooth corner.
    Item {
        objectName: "celestina-compositor-glass-region"

        anchors.fill: parent
        anchors.margins: 2
        readonly property real radius: panel.radius
        readonly property var polygon: []
    }

    Rectangle {
        objectName: "celestina-panel-tint"

        anchors.fill: parent
        radius: panel.radius
        antialiasing: true
        // The One UI balance: roughly half tint, half colour summary.
        color: CelestinaTheme.panelTint
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.divider
    }
}
