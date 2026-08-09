// One side of the panel: an ordered row of widgets that grows from its own
// edge and is clipped by the space the centred clock leaves it.
//
// The panel's regions are a list, not a set of anchors. A later phase adds a
// widget by adding a child in the order it should appear — caffeine (R3), the
// unread badge (R4), weather (R5) — and nothing about the layout has to be
// renegotiated. An empty flank paints nothing at all: a reserved place is
// structural, never a visible placeholder.
import CelestinaStyle
import QtQuick

Item {
    id: root

    // Which edge the row grows from. The right flank packs against the right
    // so that, when space runs out, it is the innermost widget that is clipped
    // rather than the one at the screen edge.
    property bool trailing: false
    // Width this flank's first widget must leave for the ones after it. A
    // widget that grows with its content — the workspace strip carries the
    // focused window's title — would otherwise claim the whole row and the
    // clip below would quietly remove everything behind it from the bar.
    property real reservedWidth: 0
    readonly property real spacing: row.spacing

    // What one widget costs this row: its own width, plus the gap before it
    // when it has any width at all. A widget with nothing to show costs
    // nothing, gap included, exactly as the Row lays it out.
    function roomFor(widget) {
        return widget.implicitWidth > 0 ? widget.implicitWidth + row.spacing : 0;
    }
    default property alias widgets: row.data
    readonly property real contentWidth: row.implicitWidth

    implicitHeight: row.implicitHeight
    clip: true

    Row {
        id: row

        // PANEL-1 — the flank clips, and the outermost pill overhangs its
        // reading by a few pixels, so without this inset the first and last
        // piece of glass were sliced down their outer edge.
        anchors.left: root.trailing ? undefined : parent.left
        anchors.leftMargin: root.trailing ? 0 : CelestinaTheme.spaceSm
        anchors.right: root.trailing ? parent.right : undefined
        anchors.rightMargin: root.trailing ? CelestinaTheme.spaceSm : 0
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.space2xl
    }

}
