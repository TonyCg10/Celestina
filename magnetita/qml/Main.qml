import QtQuick
import QtQuick.Window
import QtQuick.Controls
import org.celestina.magnetita 1.0

ApplicationWindow {
    id: window

    visible: true
    width: 460
    height: 680
    minimumWidth: 380
    minimumHeight: 480
    title: "Magnetita"
    color: CelestinaTheme.canvas

    DevicesModel {
        id: devices
        Component.onCompleted: reload()
    }

    readonly property int mediaIndex: {
        for (var i = 0; i < devices.deviceMedia.length; i++) {
            if (devices.deviceMedia[i].length > 0)
                return i
        }
        return -1
    }

    readonly property int primaryIndex: devices.deviceNames.length > 0 ? 0 : -1
    readonly property int mediaControlIndex: mediaIndex >= 0 ? mediaIndex : primaryIndex

    property bool settingsOpen: false

    Item {
        id: appSurface
        anchors.fill: parent

        CelestinaBackdrop {
            id: backdropLayer
            anchors.fill: parent

            Rectangle {
                x: -70
                y: 8
                width: 260
                height: 82
                radius: CelestinaTheme.radiusPill
                color: CelestinaTheme.dangerFill
                opacity: CelestinaTheme.decorationOpacitySoft
            }

            Rectangle {
                x: parent.width - width + 70
                y: -18
                width: 290
                height: 120
                radius: CelestinaTheme.radiusPill
                color: CelestinaTheme.surfaceSelected
                opacity: CelestinaTheme.decorationOpacityStrong
            }
        }

        Column {
            anchors.fill: parent
            anchors.margins: 22
            spacing: 6

            AppHeader {
                width: parent.width
                settingsOpen: window.settingsOpen
                backdropSource: backdropLayer
                onToggleRequested: {
                    window.settingsOpen = !window.settingsOpen
                    if (window.settingsOpen)
                        devices.reloadSettings()
                }
            }

            DevicesPage {
                visible: !window.settingsOpen
                width: parent.width
                height: parent.height - y
                devices: devices
                mediaIndex: window.mediaIndex
                primaryIndex: window.primaryIndex
                mediaControlIndex: window.mediaControlIndex
            }

            SettingsPage {
                visible: window.settingsOpen
                width: parent.width
                devices: devices
            }
        }
    }

}
