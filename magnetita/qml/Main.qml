import QtQuick
import QtQuick.Window
import QtQuick.Controls
import org.celestina.magnetita 1.0

ApplicationWindow {
    id: window

    required property bool reducedMotion

    visible: true
    width: 520
    height: 740
    minimumWidth: 420
    minimumHeight: 480
    title: "Magnetita"
    color: CelestinaTheme.canvas

    Component.onCompleted: CelestinaTheme.reducedMotion = reducedMotion

    DevicesModel {
        id: devicesModel
        Component.onCompleted: reload()
    }

    readonly property int mediaIndex: {
        if (!devicesModel.devicesAvailable)
            return -1
        for (var i = 0; i < devicesModel.deviceMediaPlayers.length; i++) {
            if (devicesModel.deviceMediaPlayers[i].length > 0)
                return i
        }
        return -1
    }

    readonly property int primaryIndex:
            devicesModel.devicesAvailable && devicesModel.deviceNames.length > 0 ? 0 : -1
    readonly property int mediaControlIndex: mediaIndex >= 0 ? mediaIndex : primaryIndex

    property bool settingsOpen: false
    property bool messagesOpen: false

    MessagesModel {
        id: messagesModel
    }

    CommandsModel {
        id: commandsModel
    }

    // The phone's screen, in a window of its own that follows the daemon's
    // link mirror: it opens when the phone streams and closes with it.
    MirrorView {
        id: mirrorView
        Component.onCompleted: start()
    }

    MirrorWindow {
        view: mirrorView
    }

    Item {
        id: appSurface
        anchors.fill: parent

        CelestinaBackdrop {
            id: backdropLayer
            anchors.fill: parent
        }

        Column {
            anchors.fill: parent
            anchors.margins: 25
            spacing: 0

            AppHeader {
                id: appHeader
                width: parent.width
                settingsOpen: window.settingsOpen
                messagesOpen: window.messagesOpen
                deviceCount: devicesModel.devicesAvailable
                             ? devicesModel.deviceNames.length : 0
                devicesAvailable: devicesModel.devicesAvailable
                settingsAvailable: devicesModel.settingsAvailable
                onToggleRequested: {
                    window.settingsOpen = !window.settingsOpen
                    if (window.settingsOpen) {
                        devicesModel.reloadSettings()
                        commandsModel.refresh()
                    }
                }
                onMessagesRequested: window.messagesOpen = !window.messagesOpen
            }

            MessagesPage {
                visible: window.messagesOpen && !window.settingsOpen
                width: parent.width
                height: parent.height - y
                messages: messagesModel
                deviceId: window.primaryIndex >= 0 && window.primaryIndex < devicesModel.deviceIds.length
                          ? devicesModel.deviceIds[window.primaryIndex] : ""
            }

            DevicesPage {
                visible: !window.settingsOpen && !window.messagesOpen
                width: parent.width
                height: parent.height - y
                devices: devicesModel
                mediaIndex: window.mediaIndex
                primaryIndex: window.primaryIndex
                mediaControlIndex: window.mediaControlIndex
            }

            SettingsPage {
                visible: window.settingsOpen
                width: parent.width
                height: parent.height - y
                devices: devicesModel
                commands: commandsModel
            }
        }
    }

}
