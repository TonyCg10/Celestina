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

    // The phone, when Magnetita has one connected. Hidden otherwise — no daemon,
    // no device, and the panel is just the clock.
    Row {
        id: phoneIndicator

        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        spacing: 6
        visible: Phone.phoneConnected

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "📱"
            font.pixelSize: 13
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Phone.phoneName
            color: "#e0def4"
            font.pixelSize: 13
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: Phone.phoneBattery >= 0
            text: (Phone.phoneCharging ? "⚡ " : "") + Phone.phoneBattery + " %"
            color: Phone.phoneBattery <= 15 ? "#eb6f92"
                 : Phone.phoneBattery <= 30 ? "#f6c177"
                 : "#908caa"
            font.pixelSize: 13
        }
    }

    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        height: 1
        color: "#403d52"
    }
}
