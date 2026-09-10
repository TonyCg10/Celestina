import QtQuick
import org.celestina.magnetita 1.0

// One message: mine on the right in the accent, theirs on the left on a
// surface, the sender's name above when it is a group.
Item {
    id: root

    required property string body
    required property bool fromMe
    required property string name

    implicitHeight: bubble.height + CelestinaTheme.spaceXs

    CelestinaSurface {
        id: bubble
        anchors.right: root.fromMe ? parent.right : undefined
        anchors.left: root.fromMe ? undefined : parent.left
        width: Math.min(parent.width * 0.78, label.implicitWidth + CelestinaTheme.spaceMd * 2)
        height: label.implicitHeight + CelestinaTheme.spaceSm * 2
        role: root.fromMe ? CelestinaSurface.Selected : CelestinaSurface.Grouped
        radiusOverride: CelestinaTheme.radiusMd

        Text {
            id: label
            // Peer-supplied text: never interpreted as markup.
            textFormat: Text.PlainText
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceSm
            anchors.leftMargin: CelestinaTheme.spaceMd
            anchors.rightMargin: CelestinaTheme.spaceMd
            text: root.body
            color: bubble.ink
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            wrapMode: Text.Wrap
        }
    }
}
