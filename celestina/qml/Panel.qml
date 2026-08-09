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
    signal workspaceMapRequested(int globalX, int globalY, var workspaces)
    signal trayMenuRequested(string service, string path, int globalX, int globalY)
    signal notificationCentreRequested()
    signal controlCentreRequested()
    signal clipboardRequested()
    signal sessionMenuRequested()
    signal indicatorMenuRequested(string kind, int globalX, int globalY)

    // A provider key may be inserted by a later frame of the same helper
    // generation, so every lookup goes through the one access point that makes
    // a binding depend on the snapshot revision rather than on a key that did
    // not exist yet. See `ProviderReading`.
    function provider(name) {
        return ProviderReading.read(panel.providerSource, name);
    }

    width: Screen.width
    // PANEL-1 — the surface is taller than the bar; `bar` is the bar.
    readonly property int barHeight: 40
    height: 112
    visible: false
    // PANEL-1 replaces the flat tint with a
    // scrim that fades into the wallpaper, which is the macOS-style bar the
    // author asked to see. It deliberately breaks the contrast contract: the
    // flat tint was 90-96% opaque precisely so the guard could prove 4.5:1 over
    // black *and* white, and nothing that fades to transparent can make that
    // claim without measuring the wallpaper first. Perceptual validation must
    // therefore include bright and dark wallpaper regions.
    color: CelestinaTheme.clear
    title: qsTr("Panel de Celestina")
    flags: Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = panel.reducedMotion;
        glassSettle.restart();
    }

    // PANEL-1 — the rectangles the compositor should blur, in window
    // coordinates, collected from the pills themselves.
    //
    // Published from here rather than found from C++: an item declared in a QML
    // `Window` sits in an object tree whose shape is Qt's business, and walking
    // it guessed wrong twice — first finding nothing under the content item, and
    // an empty region is not "blur nothing", it is "blur everything". The pills
    // know where they are; this asks them.
    property var glassRects: []

    function collectGlass() {
        const found = [];
        const walk = function(item) {
            for (let index = 0; index < item.children.length; ++index) {
                const child = item.children[index];
                if (child.objectName === "celestina-panel-pill" && child.visible
                    && child.width > 0 && child.height > 0) {
                    const at = child.mapToItem(null, 0, 0);
                    found.push(Qt.rect(at.x, at.y, child.width, child.height));
                }
                walk(child);
            }
        };
        walk(panel.contentItem);
        panel.glassRects = found;
    }

    function scheduleGlassCollection() {
        glassSettle.restart();
    }

    // Layout settles over several passes, and a rectangle read mid-pass is a
    // rectangle in the wrong place. This runs after it stops moving.
    Timer {
        id: glassSettle

        interval: 120
        repeat: false
        onTriggered: panel.collectGlass()
    }

    onWidthChanged: panel.scheduleGlassCollection()

    // Where the bar's own content lives: the top band, the part that reserves
    // screen. Everything below it is scrim and nothing else.
    Item {
        id: bar

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: panel.barHeight
    }

    // The bar as a shadow over the content rather than a surface on top of it.
    //
    // The falloff runs well past the bar because a scrim that has to finish
    // inside the bar's own height cannot be soft: it ends where the surface
    // ends, and a ramp cut off mid-slope reads as an edge however gentle its
    // gradient was. Many stops rather than three, easing out rather than
    // straight, so there is no point along it where the rate of change jumps.
    Rectangle {
        anchors.fill: parent

        gradient: Gradient {
            // The shadow remains a first-class part of the bar: dense behind
            // the readings, then progressively absent without a terminal edge.
            // Capsule fill and stroke stay clear, so this depth does not replace
            // the compositor's independently shaped blur regions.
            GradientStop { position: 0.00; color: CelestinaTheme.withAlpha(CelestinaTheme.canvas, 0.82) }
            GradientStop { position: 0.14; color: CelestinaTheme.withAlpha(CelestinaTheme.canvas, 0.72) }
            GradientStop { position: 0.30; color: CelestinaTheme.withAlpha(CelestinaTheme.canvas, 0.52) }
            GradientStop { position: 0.48; color: CelestinaTheme.withAlpha(CelestinaTheme.canvas, 0.30) }
            GradientStop { position: 0.68; color: CelestinaTheme.withAlpha(CelestinaTheme.canvas, 0.13) }
            GradientStop { position: 0.85; color: CelestinaTheme.withAlpha(CelestinaTheme.canvas, 0.04) }
            GradientStop { position: 1.00; color: CelestinaTheme.withAlpha(CelestinaTheme.canvas, 0.00) }
        }

    }

    // Three regions, and only the middle one is anchored to the screen: the
    // clock stays geometrically centred whatever the flanks carry, which is
    // the one placement the lived bar never gives up.
    PanelFlank {
        id: leftFlank

        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceMd + CelestinaTheme.spaceSm
        anchors.right: clock.left
        anchors.rightMargin: CelestinaTheme.space2xl
        anchors.verticalCenter: bar.verticalCenter
        // What the strip must leave for the widgets that follow it. Measured
        // by their own width rather than by `visible`, which also depends on
        // the window being shown: what matters here is whether a widget has
        // anything to occupy. An absent one is zero wide and reserves nothing,
        // its gap included.
        reservedWidth: leftFlank.roomFor(sysMon) + leftFlank.roomFor(media)

        WorkspaceStrip {

            // PANEL-1 — one height for every reading, so the row aligns them.

            height: CelestinaTheme.controlHeightXs
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
            onMapRequested: (globalX, globalY, workspaces) => panel.workspaceMapRequested(globalX, globalY, workspaces)

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

        SysMon {

            // PANEL-1 — one height for every reading, so the row aligns them.

            height: CelestinaTheme.controlHeightXs
            id: sysMon

            reading: panel.provider("sysmon")
            onMonitorRequested: panel.providerSource.sendCommand("sysmon", "open-monitor")

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

        MediaMini {

            // PANEL-1 — one height for every reading, so the row aligns them.

            height: CelestinaTheme.controlHeightXs
            id: media
            objectName: "celestina-panel-media"

            anchors.verticalCenter: parent.verticalCenter
            reading: panel.provider("media")
            onToggleRequested: panel.providerSource.sendCommand("media", "PlayPause")

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

    }

    Clock {
        id: clock

        anchors.centerIn: bar

        PanelPill {
            blurAvailable: panel.compositorBlurAvailable
            onBlurRegionChanged: panel.scheduleGlassCollection()
        }

    }

    PanelFlank {
        id: rightFlank

        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceMd + CelestinaTheme.spaceSm
        anchors.left: clock.right
        anchors.leftMargin: CelestinaTheme.space2xl
        anchors.verticalCenter: bar.verticalCenter
        trailing: true

        // PANEL-1 — this reading lays out its own children, so a pill

        // placed inside it became a cell of that row instead of a floor

        // under it. The wrapper gives the glass somewhere to sit that the

        // layout does not own, and reports the same size to the flank.

        Item {

            height: CelestinaTheme.controlHeightXs
            implicitWidth: traydrawerBody.implicitWidth

            implicitHeight: traydrawerBody.implicitHeight

            // Do not bind a parent's visibility to the effective visibility of
            // its child. The tray starts empty; that hid the parent, which in
            // turn kept the child effectively hidden after its late D-Bus
            // items arrived — a visibility cycle with four valid items and no
            // pixels. The model is the independent source of truth.
            visible: panel.traySource.items.length > 0

            TrayDrawer {

                id: traydrawerBody

                anchors.verticalCenter: parent.verticalCenter
                items: panel.traySource.items
                onActivated: (service, path, globalX, globalY) => panel.traySource.activate(service, path, globalX, globalY)
                onSecondaryActivated: (service, path, globalX, globalY) => panel.traySource.secondaryActivate(service, path, globalX, globalY)
                onMenuRequested: (service, path, globalX, globalY) => panel.trayMenuRequested(service, path, globalX, globalY)

            }

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

        // PANEL-1 — this reading lays out its own children, so a pill

        // placed inside it became a cell of that row instead of a floor

        // under it. The wrapper gives the glass somewhere to sit that the

        // layout does not own, and reports the same size to the flank.

        Item {

            height: CelestinaTheme.controlHeightXs
            implicitWidth: sessionstatusBody.implicitWidth

            implicitHeight: sessionstatusBody.implicitHeight

            visible: sessionstatusBody.visible

            SessionStatus {

                id: sessionstatusBody

                anchors.verticalCenter: parent.verticalCenter
                network: panel.provider("network")
                bluetooth: panel.provider("bluetooth")
                power: panel.provider("power")
                onProfileCycleRequested: panel.providerSource.sendCommand("power", "cycle")
                onIndicatorMenuRequested: (kind, globalX, globalY) => panel.indicatorMenuRequested(kind, globalX, globalY)

            }

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

        AudioLevel {

            // PANEL-1 — one height for every reading, so the row aligns them.

            height: CelestinaTheme.controlHeightXs
            anchors.verticalCenter: parent.verticalCenter
            reading: panel.provider("audio")
            onMuteToggled: panel.providerSource.sendCommand("audio", "mute-toggle")
            onMicMuteToggled: panel.providerSource.sendCommand("audio", "mic-mute-toggle")
            onMixerRequested: panel.providerSource.sendCommand("audio", "open-mixer")
            onStepRequested: (direction) => panel.providerSource.sendCommand(
                "audio", "volume-step", {"by": direction > 0 ? panel.levelStep : -panel.levelStep})

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

        BrightnessLevel {

            // PANEL-1 — one height for every reading, so the row aligns them.

            height: CelestinaTheme.controlHeightXs
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

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

        NotificationIndicator {

            // PANEL-1 — one height for every reading, so the row aligns them.

            height: CelestinaTheme.controlHeightXs
            anchors.verticalCenter: parent.verticalCenter
            reading: panel.provider("notifications")
            // The panel asks; the host owns the surface that answers, exactly
            // as it does for the menus.
            onHistoryRequested: panel.notificationCentreRequested()
            onQuietToggled: panel.providerSource.sendCommand(
                "notifications", "quiet-toggle")

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

        PanelActionButton {
            objectName: "celestina-control-centre-button"
            blurAvailable: panel.compositorBlurAvailable
            iconName: "settings"
            helpText: qsTr("Abrir el centro de control")
            onClicked: panel.controlCentreRequested()
            onBlurRegionChanged: panel.scheduleGlassCollection()
        }

        PanelActionButton {
            objectName: "celestina-clipboard-button"
            blurAvailable: panel.compositorBlurAvailable
            iconName: "clipboard-paste"
            helpText: qsTr("Abrir el historial del portapapeles")
            onClicked: panel.clipboardRequested()
            onBlurRegionChanged: panel.scheduleGlassCollection()
        }

        PanelActionButton {
            objectName: "celestina-session-menu-button"
            blurAvailable: panel.compositorBlurAvailable
            iconName: "power"
            helpText: qsTr("Abrir el menú de sesión")
            onClicked: panel.sessionMenuRequested()
            onBlurRegionChanged: panel.scheduleGlassCollection()
        }

        CaptureButton {

            // PANEL-1 — one height for every reading, so the row aligns them.

            height: CelestinaTheme.controlHeightXs
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

            PanelPill {
                blurAvailable: panel.compositorBlurAvailable
                onBlurRegionChanged: panel.scheduleGlassCollection()
            }

        }

        PhoneStatus {

            // PANEL-1 — one height for every reading, so the row aligns them.

            height: CelestinaTheme.controlHeightXs
            width: Math.min(implicitWidth, rightFlank.width)
            blurAvailable: panel.compositorBlurAvailable
            connected: panel.phoneProvider.phoneConnected
            battery: panel.phoneProvider.phoneBattery
            charging: panel.phoneProvider.phoneCharging
            onBlurRegionChanged: panel.scheduleGlassCollection()
        }
    }

}
