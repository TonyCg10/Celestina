import QtQuick
import QtQuick.Window

Window {
    id: panel

    width: Screen.width
    height: 40
    visible: false
    color: "#191724"
    title: qsTr("Celestina Panel")
    flags: Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus

    Clock {
        anchors.centerIn: parent
    }

    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        height: 1
        color: "#403d52"
    }
}
