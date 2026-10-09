import QtQuick
import org.celestina.selenita 1.0

// A card's single muted line while it has nothing else to show.
Item {
    id: emptyLine

    required property string text
    // The card's `rowInset`.
    required property int inset

    width: parent ? parent.width : 0
    height: CelestinaTheme.rowHeight
    Accessible.role: Accessible.StaticText
    Accessible.name: text

    Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: emptyLine.inset
        anchors.rightMargin: emptyLine.inset
        anchors.verticalCenter: parent.verticalCenter
        text: emptyLine.text
        elide: Text.ElideRight
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
    }
}
