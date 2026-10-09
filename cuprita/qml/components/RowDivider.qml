import QtQuick
import org.celestina.cuprita 1.0

// The hairline between two rows of one card. It spans the rows' words, not
// the card: it starts and ends where the text column and the trailing
// controls do, `spaceLg` from the card's edge.
Rectangle {
    id: divider
    // The card's `rowInset`.
    required property int inset

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.leftMargin: divider.inset
    anchors.rightMargin: divider.inset
    height: CelestinaTheme.borderHairline
    color: CelestinaTheme.divider
}
