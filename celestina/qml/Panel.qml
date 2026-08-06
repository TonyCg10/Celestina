import CelestinaStyle
import QtQuick
import QtQuick.Window
import "ProviderReading.js" as ProviderReading

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
    // One wheel notch, in whole percent. The panel names the step it takes
    // instead of leaving each provider to invent one, so volume and brightness
    // move by the same amount under the same gesture.
    readonly property int levelStep: 5
    // Forwarded to the host, which owns every surface this window does not.
    signal contextMenuRequested(int globalX, int globalY, var workspaces)
    signal trayMenuRequested(string service, string path, int globalX, int globalY)
    signal notificationCentreRequested()

    // A provider key may be inserted by a later frame of the same helper
    // generation, so every lookup goes through the one access point that makes
    // a binding depend on the snapshot revision rather than on a key that did
    // not exist yet. See `ProviderReading`.
    function provider(name) {
        return ProviderReading.read(panel.providerSource, name);
    }

    width: Screen.width
    height: 40
    visible: false
    // Wallpaper is untrusted visual input. The host reports whether it could
    // arm compositor blur; an opaque fallback keeps every ink pair legible.
    color: compositorBlurAvailable
           ? CelestinaTheme.compositorGlassTint
           : CelestinaTheme.compositorGlassFallback
    title: qsTr("Panel de Celestina")
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
        // What the strip must leave for the widgets that follow it. Measured
        // by their own width rather than by `visible`, which also depends on
        // the window being shown: what matters here is whether a widget has
        // anything to occupy. An absent one is zero wide and reserves nothing,
        // its gap included.
        reservedWidth: leftFlank.roomFor(sysMon) + leftFlank.roomFor(media)

        WorkspaceStrip {
            // Its natural width until the flank runs out — *minus* what the
            // widgets beside it need. Taking the whole flank is what hid the
            // media widget on a live session: the strip grows with the focused
            // window's title, the flank clips what does not fit, and everything
            // after the strip silently left the bar. The title elides instead.
            width: Math.min(
                implicitWidth,
                Math.max(0, leftFlank.width - leftFlank.reservedWidth)
            )
            niriAvailable: panel.niriProvider.available
            outputName: panel.outputName
            workspaces: panel.niriProvider.workspaces
            // The strip reports the gesture; the provider owns the protocol and
            // answers through `workspaces`, never through this call's return.
            onFocusRequested: (output, index) => panel.niriProvider.requestWorkspaceFocus(output, index)
            onMenuRequested: (globalX, globalY, workspaces) => panel.contextMenuRequested(globalX, globalY, workspaces)
        }

        SysMon {
            id: sysMon

            reading: panel.provider("sysmon")
            onMonitorRequested: panel.providerSource.sendCommand("sysmon", "open-monitor")
        }

        MediaMini {
            id: media
            objectName: "celestina-panel-media"

            anchors.verticalCenter: parent.verticalCenter
            reading: panel.provider("media")
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
            network: panel.provider("network")
            bluetooth: panel.provider("bluetooth")
            power: panel.provider("power")
            onProfileCycleRequested: panel.providerSource.sendCommand("power", "cycle")
        }

        AudioLevel {
            anchors.verticalCenter: parent.verticalCenter
            reading: panel.provider("audio")
            onMuteToggled: panel.providerSource.sendCommand("audio", "mute-toggle")
            onMicMuteToggled: panel.providerSource.sendCommand("audio", "mic-mute-toggle")
            onMixerRequested: panel.providerSource.sendCommand("audio", "open-mixer")
            onStepRequested: (direction) => panel.providerSource.sendCommand(
                "audio", "volume-step", {"by": direction > 0 ? panel.levelStep : -panel.levelStep})
        }

        BrightnessLevel {
            anchors.verticalCenter: parent.verticalCenter
            reading: panel.provider("brightness")
            outputName: panel.outputName
            // The step names its own monitor: one helper serves every panel,
            // and each panel speaks only for the output it is mapped on.
            onStepRequested: (direction) => panel.providerSource.sendCommand(
                "brightness", "brightness-step", {
                    "by": direction > 0 ? panel.levelStep : -panel.levelStep,
                    "output": panel.outputName
                })
        }

        NotificationIndicator {
            anchors.verticalCenter: parent.verticalCenter
            reading: panel.provider("notifications")
            // The panel asks; the host owns the surface that answers, exactly
            // as it does for the menus.
            onHistoryRequested: panel.notificationCentreRequested()
            onQuietToggled: panel.providerSource.sendCommand(
                "notifications", "quiet-toggle")
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
