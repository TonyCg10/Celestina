import QtQuick
import org.celestina.grafita 1.0

// The document's name, its unsaved marker, its encoding, and whatever the file
// on disk disagrees about. A conflict is the user's to act on, so it stays in
// view rather than passing as a toast.
Item {
    id: root

    required property var session

    implicitHeight: heading.height + CelestinaTheme.space2xl
                    + (conflict.visible ? conflict.height + CelestinaTheme.spaceXs : 0)

    Text {
        id: heading
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.top: parent.top
        anchors.topMargin: CelestinaTheme.spaceMd
        width: parent.width - encoding.width - CelestinaTheme.space3xl
        elide: Text.ElideMiddle
        visible: root.session.active
        text: root.session.dirty ? root.session.name + " •" : root.session.name
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
        font.weight: CelestinaTheme.weightDemiBold

        // The bullet says "unsaved" to the eye only; assistive technology is
        // told in words rather than read a punctuation mark.
        Accessible.role: Accessible.StaticText
        Accessible.name: root.session.dirty
                         ? root.session.name + ", sin guardar" : root.session.name
    }

    Text {
        id: encoding
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceLg
        anchors.verticalCenter: heading.verticalCenter
        visible: root.session.active
        text: root.session.encodingLabel
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption

        Accessible.role: Accessible.StaticText
        Accessible.name: "Codificación " + root.session.encodingLabel
    }

    Text {
        id: conflict
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceLg
        anchors.top: heading.bottom
        anchors.topMargin: CelestinaTheme.spaceXs
        wrapMode: Text.WordWrap
        visible: root.session.active && root.session.conflictText.length > 0
        text: root.session.conflictText
        color: CelestinaTheme.warning
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption

        Accessible.role: Accessible.AlertMessage
        Accessible.name: "Aviso: " + text
    }
}
