import QtQuick
import org.celestina.cuprita 1.0

// The one muted line a card shows while it has no rows.
Item {
    id: line

    required property string text
    // The card's `rowInset`.
    required property int inset

    width: parent ? parent.width : 0
    height: CelestinaTheme.rowHeight

    Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: line.inset
        anchors.rightMargin: line.inset
        anchors.verticalCenter: parent.verticalCenter
        text: line.text
        elide: Text.ElideRight
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
    }
}
