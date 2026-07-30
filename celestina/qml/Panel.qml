import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: panel

    required property string outputName
    required property bool reducedMotion
    // C++ adapters are not QML-creatable types. These two narrow providers
    // expose only the documented Niri and DevicesClient read interfaces.
    required property var niriProvider
    required property var phoneProvider
    property bool compositorBlurAvailable: false

    width: Screen.width
    height: 40
    visible: false
    // Wallpaper is untrusted visual input. The host reports whether it could
    // arm compositor blur; an opaque fallback keeps every ink pair legible.
    color: compositorBlurAvailable
           ? CelestinaTheme.compositorGlassTint
           : CelestinaTheme.compositorGlassFallback
    title: qsTr("Celestina Panel")
    flags: Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus

    Component.onCompleted: CelestinaTheme.reducedMotion = reducedMotion

    WorkspaceStrip {
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: clock.left
        anchors.rightMargin: CelestinaTheme.space2xl
        anchors.verticalCenter: parent.verticalCenter
        height: implicitHeight
        clip: true
        niriAvailable: panel.niriProvider.available
        outputName: panel.outputName
        workspaces: panel.niriProvider.workspaces
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
        connected: panel.phoneProvider.phoneConnected
        phoneName: panel.phoneProvider.phoneName
        battery: panel.phoneProvider.phoneBattery
        charging: panel.phoneProvider.phoneCharging
    }

    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        height: 1
        color: CelestinaTheme.divider
    }

}
