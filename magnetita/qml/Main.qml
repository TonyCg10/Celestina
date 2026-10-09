import QtQuick
import QtQuick.Window
import QtQuick.Controls
import org.celestina.magnetita 1.0

ApplicationWindow {
    id: window

    // Launched by the daemon for the mirror alone: no main window, and the
    // process ends when the mirror does.
    required property bool mirrorOnly
    // `magnetita --send` with several devices connected: the window is the
    // device chooser alone, and closes once the person picks or cancels.
    required property var sendDeviceNames
    required property int sendCount
    readonly property bool sendMode: sendCount > 0

    visible: !mirrorOnly
    width: 520
    height: 740
    minimumWidth: 420
    minimumHeight: 480
    title: "Magnetita"
    // Transparent: the compositor blurs what lies behind the window and
    // CelestinaBackdrop paints the Haze canvas over it (DESIGN §5.2 L0).
    // The mirror is its own window and keeps the opaque canvas: it is sized
    // to the phone's picture, and nothing of the desktop belongs around it.
    color: CelestinaTheme.clear

    // `--send`: the window stays while the files go, so a failure is seen.
    onClosing: function(close) {
        close.accepted = !sendChooser.busy
    }

    // The suite's appearance file (reduced motion, text scale), followed on
    // the adapter's worker and bound into the theme once for this window.
    MagnetitaAppearance {
        id: appearanceAdapter
    }

    CelestinaAppearance {
        reducedMotion: appearanceAdapter.appearanceReducedMotion
        textScale: appearanceAdapter.appearanceTextScale
    }

    DevicesModel {
        id: devicesModel
        Component.onCompleted: reload()
        // `--send`'s chooser: done closes the window; a failure stays shown.
        onSendFinished: function(error) {
            sendChooser.busy = false
            sendChooser.failure = error
            if (error === "")
                Qt.quit()
        }
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
        // A mirror-only process ends when the mirror is over and the view
        // has let its engine go; exiting earlier aborts under libmpv.
        onDoneChanged: {
            if (window.mirrorOnly && done && mirrorView.everStreamed)
                Qt.quit()
        }
        onStreamingChanged: {
            if (streaming)
                mirrorView.everStreamed = true
        }
        property bool everStreamed: false
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

        // A drop on the device page, outside any row, goes to the page's
        // device; the rows, stacked above, take a drop meant for them.
        DropArea {
            id: pageDrop
            anchors.fill: parent
            enabled: devicesPage.visible && window.primaryIndex >= 0
            keys: ["text/uri-list"]
            onEntered: function(drag) {
                if (!drag.hasUrls)
                    drag.accepted = false
            }
            onDropped: function(drop) {
                devicesPage.sendDrop(window.primaryIndex, drop)
            }

            // Siderita's drop highlight: an accent rim, nothing filled.
            Rectangle {
                anchors.fill: parent
                visible: pageDrop.containsDrag
                color: CelestinaTheme.clear
                border.width: CelestinaTheme.borderFocus
                border.color: CelestinaTheme.accent
                radius: CelestinaTheme.radiusWindow
            }
        }

        Column {
            anchors.fill: parent
            anchors.margins: 25
            spacing: 0

            SendChooser {
                id: sendChooser
                visible: window.sendMode
                width: parent.width
                deviceNames: window.sendDeviceNames
                fileCount: window.sendCount
                onChosen: function(index) {
                    sendChooser.busy = true
                    devicesModel.chooseSendDevice(index)
                }
                onCancelled: Qt.quit()
            }

            AppHeader {
                id: appHeader
                visible: !window.sendMode
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
                visible: window.messagesOpen && !window.settingsOpen && !window.sendMode
                width: parent.width
                height: parent.height - y
                messages: messagesModel
                deviceId: window.primaryIndex >= 0 && window.primaryIndex < devicesModel.deviceIds.length
                          ? devicesModel.deviceIds[window.primaryIndex] : ""
            }

            DevicesPage {
                id: devicesPage
                visible: !window.settingsOpen && !window.messagesOpen && !window.sendMode
                width: parent.width
                height: parent.height - y
                devices: devicesModel
                backdrop: appSurface
                mediaIndex: window.mediaIndex
                primaryIndex: window.primaryIndex
                mediaControlIndex: window.mediaControlIndex
            }

            SettingsPage {
                visible: window.settingsOpen && !window.sendMode
                width: parent.width
                height: parent.height - y
                devices: devicesModel
                commands: commandsModel
            }
        }
    }

}
