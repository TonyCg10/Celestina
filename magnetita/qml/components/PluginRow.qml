import QtQuick
import org.celestina.magnetita 1.0

Item {
    id: root

    required property string label
    required property bool enabledFlag
    signal toggleRequested

    height: 46

    Text {
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.right: toggle.left
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        text: root.label
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
        elide: Text.ElideRight
    }

    CelestinaSwitch {
        id: toggle
        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        checked: root.enabledFlag
        Accessible.name: root.label

        // A click is only a request. Re-bind immediately so the switch keeps
        // showing the daemon's confirmed state instead of an optimistic one.
        onClicked: {
            checked = Qt.binding(function() { return root.enabledFlag })
            root.toggleRequested()
        }
    }
}
