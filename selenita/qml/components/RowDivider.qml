import QtQuick
import org.celestina.selenita 1.0

// The hairline above a row that follows another in the same card. It runs
// between the row's insets, so it lines up with the words, not the card.
Rectangle {
    id: divider
    // The card's `rowInset`.
    required property int inset

    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: divider.inset
    anchors.rightMargin: divider.inset
    height: CelestinaTheme.borderHairline
    color: CelestinaTheme.divider
}
