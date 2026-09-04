import QtQuick

// ─── CelestinaCapsule ─────────────────────────────────────────────────────────
// A few icon actions in one control-shaped surface: the sort field and its
// direction, the three view modes, undo and redo. The capsule paints the
// resting fill once, at pill radius, and the glyphs inside it are Ghost icon
// buttons, so each keeps the suite's hover circle and press recoil while the
// group reads as one control rather than as loose glyphs on the bar.
//
// Consumers put `CelestinaIconButton { role: CelestinaButton.Ghost }` children
// straight inside; a checked one wears Selected through the button's own
// `checkable`. The capsule owns nothing but the surface and the spacing.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: capsule

    default property alias actions: row.data
    property int spacing: 0
    // The rim between the glyph circles and the capsule's edge.
    property int inset: CelestinaTheme.spaceXs / 2
    property color fill: CelestinaTheme.controlFill

    implicitWidth: row.implicitWidth + inset * 2
    implicitHeight: row.implicitHeight + inset * 2

    Rectangle {
        anchors.fill: parent
        radius: CelestinaTheme.radiusPill
        color: capsule.fill
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: capsule.spacing
    }
}
