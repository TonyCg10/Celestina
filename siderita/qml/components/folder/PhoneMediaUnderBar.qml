import QtQuick
import org.celestina.siderita 1.0

// Where the phone's media action hangs once the heading has retired: centred on
// the search glyph, one gap below the bar.
//
// Anchored to the bar rather than positioned from a computed point. The first
// attempt used `mapToItem`, which is evaluated once and never again, so the
// button was drawn in the corner and stayed there while the bar moved.
//
// The button floats directly over the listing, and a disabled item is dropped
// from pointer delivery together with everything inside it: a disconnected
// phone's dimmed button was a hole through which the press selected, opened
// or dragged the row behind. The shield therefore lives on this wrapper, which
// is never disabled — the same floor the Trash and Recientes headers stand on.
Item {
    id: root

    // Typed loosely on purpose would cost the checker its knowledge of
    // `searchCentreFromRight`; this is the bar that publishes it.
    required property TopBar bar
    required property var heading

    signal clicked

    z: root.bar.z
    width: 32
    height: 32
    anchors.right: root.bar.right
    anchors.rightMargin: root.bar.searchCentreFromRight - width / 2
    anchors.top: root.bar.bottom
    anchors.topMargin: CelestinaTheme.spaceSm
    visible: root.heading.retiredProgress > 0.5 && root.heading.phoneLocation

    CelestinaInputShield { }

    PhoneMediaButton {
        anchors.fill: parent
        connected: root.heading.phoneConnected
        onClicked: root.clicked()
    }
}
