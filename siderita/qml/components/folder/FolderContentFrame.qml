import QtQuick
import org.celestina.siderita 1.0

// Canonical geometry and surface for the folder viewport. Every interactive
// layer reads these metrics instead of independently reconstructing the box.
Item {
    id: root

    required property real frameY
    // The region this sits in already keeps the window margin, so the box has
    // no inset of its own: adding one put it 8 px further in than the sidebar
    // and 12 px short of the bottom, which no other box agreed with.
    readonly property real frameX: 0
    readonly property real frameWidth: Math.max(0, width - 2 * frameX)
    readonly property real frameBottom: height
    readonly property real frameHeight: Math.max(0, frameBottom - frameY)
    property alias surface: surfaceItem
    // Rounded like its siblings: at the window margin the window's own corner
    // no longer reaches it, so the role's radius is the honest answer — and the
    // one the sidebar beside it uses.
    readonly property real surfaceRadius:
            CelestinaTheme.nestedRadius(CelestinaTheme.windowMargin, CelestinaTheme.radiusLg)

    CelestinaSurface {
        id: surfaceItem
        x: root.frameX
        y: root.frameY
        width: root.frameWidth
        height: root.frameHeight
        role: CelestinaSurface.Grouped
        radiusOverride: root.surfaceRadius
    }
}
