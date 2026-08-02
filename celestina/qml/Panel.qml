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
    // The aggregate provider helper's client: one bridge, many providers.
    required property var providerSource
    // The system tray host: other applications' own controls.
    required property var traySource
    property bool compositorBlurAvailable: false
    // Forwarded to the host, which owns every surface this window does not.
    signal contextMenuRequested(int globalX, int globalY, var workspaces)
    signal trayMenuRequested(string service, string path, int globalX, int globalY)

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

    // Three regions, and only the middle one is anchored to the screen: the
    // clock stays geometrically centred whatever the flanks carry, which is
    // the one placement the lived bar never gives up.
    PanelFlank {
        id: leftFlank

        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: clock.left
        anchors.rightMargin: CelestinaTheme.space2xl
        anchors.verticalCenter: parent.verticalCenter

        WorkspaceStrip {
            // Its natural width until the flank runs out, so the window title
            // elides inside the panel instead of being cut mid-glyph.
            width: Math.min(implicitWidth, leftFlank.width)
            niriAvailable: panel.niriProvider.available
            outputName: panel.outputName
            workspaces: panel.niriProvider.workspaces
            // The strip reports the gesture; the provider owns the protocol and
            // answers through `workspaces`, never through this call's return.
            onFocusRequested: (output, index) => panel.niriProvider.requestWorkspaceFocus(output, index)
            onMenuRequested: (globalX, globalY, workspaces) => panel.contextMenuRequested(globalX, globalY, workspaces)
        }

        SysMon {
            reading: panel.providerSource.providers.sysmon
            onMonitorRequested: panel.providerSource.sendCommand("sysmon", "open-monitor")
        }

        MediaMini {
            anchors.verticalCenter: parent.verticalCenter
            reading: panel.providerSource.providers.media
            onToggleRequested: panel.providerSource.sendCommand("media", "PlayPause")
        }

    }

    Clock {
        id: clock

        anchors.centerIn: parent
    }

    PanelFlank {
        id: rightFlank

        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.left: clock.right
        anchors.leftMargin: CelestinaTheme.space2xl
        anchors.verticalCenter: parent.verticalCenter
        trailing: true

        TrayDrawer {
            anchors.verticalCenter: parent.verticalCenter
            items: panel.traySource.items
            onActivated: (service, path, globalX, globalY) => panel.traySource.activate(service, path, globalX, globalY)
            onSecondaryActivated: (service, path, globalX, globalY) => panel.traySource.secondaryActivate(service, path, globalX, globalY)
            onMenuRequested: (service, path, globalX, globalY) => panel.trayMenuRequested(service, path, globalX, globalY)
        }

        SessionStatus {
            anchors.verticalCenter: parent.verticalCenter
            network: panel.providerSource.providers.network
            bluetooth: panel.providerSource.providers.bluetooth
            power: panel.providerSource.providers.power
            onProfileCycleRequested: panel.providerSource.sendCommand("power", "cycle")
        }

        AudioLevel {
            anchors.verticalCenter: parent.verticalCenter
            reading: panel.providerSource.providers.audio
            onMuteToggled: panel.providerSource.sendCommand("audio", "toggle-mute")
            onMixerRequested: panel.providerSource.sendCommand("audio", "open-mixer")
            onStepRequested: (direction) => panel.providerSource.sendCommand(
                "audio", direction > 0 ? "louder" : "quieter")
        }

        BrightnessLevel {
            anchors.verticalCenter: parent.verticalCenter
            reading: panel.providerSource.providers.brightness
            outputName: panel.outputName
            // The step names its own monitor: one helper serves every panel,
            // and each panel speaks only for the output it is mapped on.
            onStepRequested: (direction) => panel.providerSource.sendCommand(
                "brightness", direction > 0 ? "brighter" : "dimmer",
                {"output": panel.outputName})
        }

        CaptureButton {
            id: captureButton

            anchors.verticalCenter: parent.verticalCenter
            onCaptureRequested: panel.niriProvider.requestScreenshot()

            // The provider reports only what it could not do; a capture the
            // compositor took over is not something this panel can observe.
            Connections {
                function onScreenshotFailed(reason) {
                    captureButton.reportFailure();
                }

                target: panel.niriProvider
            }

        }

        PhoneStatus {
            width: Math.min(implicitWidth, rightFlank.width)
            clip: true
            connected: panel.phoneProvider.phoneConnected
            phoneName: panel.phoneProvider.phoneName
            battery: panel.phoneProvider.phoneBattery
            charging: panel.phoneProvider.phoneCharging
        }

    }

    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        height: 1
        color: CelestinaTheme.divider
    }

}
