import QtQuick
import org.celestina.cuprita 1.0

// One labelled switch: the row carries the words, the switch the state. The
// switch shows what the controller says; a click asks for the change and the
// binding comes back, so the switch only stays moved once the backend agrees.
Item {
    id: root

    required property string label
    property bool checked: false

    signal toggled(bool on)

    implicitHeight: CelestinaTheme.rowHeight

    Text {
        anchors.left: parent.left
        anchors.right: toggle.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        text: root.label
        elide: Text.ElideRight
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
    }

    CelestinaSwitch {
        id: toggle
        objectName: "settingSwitch"
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        checked: root.checked
        Accessible.role: Accessible.CheckBox
        Accessible.name: root.label
        onToggled: {
            root.toggled(toggle.checked)
            toggle.checked = Qt.binding(function() { return root.checked })
        }
    }
}
