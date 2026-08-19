import QtQuick
import org.celestina.siderita 1.0

// Canonical geometry and surface for the folder viewport. Every interactive
// layer reads these metrics instead of independently reconstructing the box.
Item {
    id: root

    required property real frameY
    readonly property real frameX: CelestinaTheme.spaceSm
    readonly property real frameWidth: Math.max(0, width - 2 * frameX)
    readonly property real frameBottom: height - CelestinaTheme.spaceMd
    readonly property real frameHeight: Math.max(0, frameBottom - frameY)
    property alias surface: surfaceItem
    readonly property real surfaceRadius: CelestinaTheme.concentricRadius(frameX)

    CelestinaSurface {
        id: surfaceItem
        x: root.frameX
        y: root.frameY
        width: root.frameWidth
        height: root.frameHeight
        role: CelestinaSurface.Grouped
        // Concentric with the window: the box sits `frameX` inside it, so its
        // corner is the window's minus that gap and the two curves stay
        // parallel.
        radiusOverride: root.surfaceRadius
    }
}
