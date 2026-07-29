import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: panel

    required property string outputName

    width: Screen.width
    height: 40
    visible: false
    // The compositor owns this backdrop blur (enableBlurBehind, main.cpp), so
    // use its lighter, dedicated tint and keep the blurred wallpaper readable.
    color: CelestinaTheme.compositorGlassTint
    title: qsTr("Celestina Panel")
    flags: Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus

    WorkspaceStrip {
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: clock.left
        anchors.rightMargin: CelestinaTheme.space2xl
        anchors.verticalCenter: parent.verticalCenter
        height: implicitHeight
        clip: true
        niriAvailable: Niri.available
        outputName: panel.outputName
        workspaces: Niri.workspaces
    }

    Clock {
        id: clock

        anchors.centerIn: parent
    }

    PhoneStatus {
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(implicitWidth, Math.max(0, panel.width / 2 - clock.width / 2 - CelestinaTheme.space2xl))
        clip: true
        connected: Phone.phoneConnected
        phoneName: Phone.phoneName
        battery: Phone.phoneBattery
        charging: Phone.phoneCharging
    }

    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        height: 1
        color: CelestinaTheme.divider
    }

}
