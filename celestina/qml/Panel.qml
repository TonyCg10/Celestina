import CelestinaStyle
import QtQuick
import QtQuick.Dialogs
import QtQuick.Window
import "EdgeAttachedGeometry.js" as EdgeAttachedGeometry
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
    signal workspaceMapRequested(rect openerRect, rect attachmentAnchorRect, var workspaces)
    signal trayDrawerRequested(rect openerRect, rect attachmentAnchorRect)
    signal trayMenuRequested(string service, string path, int globalX, int globalY)
    signal launcherRequested(rect openerRect, rect attachmentAnchorRect)
    signal notificationCentreRequested(rect openerRect,
                                       rect attachmentAnchorRect)
    signal controlCentreRequested(rect openerRect, rect attachmentAnchorRect)
    signal clipboardRequested(rect openerRect, rect attachmentAnchorRect)
    signal sessionMenuRequested(rect openerRect, rect attachmentAnchorRect)
    signal indicatorMenuRequested(string kind, rect openerRect,
                                  rect attachmentAnchorRect)
    // The standard chooser owns folder selection. The host receives the
    // complete URL and is the only layer that turns it into a local filesystem
    // path; the gallery provider owns scanning and image validation after that.
    signal wallpaperFolderSelected(url source)

    BackdropInk {
        id: backdropInk
    }

    // A provider key may be inserted by a later frame of the same helper
    // generation, so every lookup goes through the one access point that makes
    // a binding depend on the snapshot revision rather than on a key that did
    // not exist yet. See `ProviderReading`.
    function provider(name) {
        return ProviderReading.read(panel.providerSource, name);
    }

    readonly property var settingsReading: panel.provider("settings")
    readonly property var trayPreferences: panel.settingsReading !== undefined
                                           && panel.settingsReading.trayItems !== undefined
                                           ? panel.settingsReading.trayItems : []
    readonly property var wallpaperGalleryReading:
        panel.provider("wallpaper-gallery")

    function openWallpaperFolderPicker() {
        if (panel.wallpaperGalleryReading !== undefined
            && panel.wallpaperGalleryReading.folderUrl !== undefined
            && panel.wallpaperGalleryReading.folderUrl.length > 0) {
            wallpaperPicker.currentFolder =
                panel.wallpaperGalleryReading.folderUrl;
        }
        wallpaperPicker.open();
    }

    // How much larger this output needs the shell drawn so that what it draws
    // measures the same as on every other monitor. The host derives it from the
    // output's real density; 1.0 is the density the tokens were drawn against,
    // and every token below stays in those unscaled units.
    property real shellScale: 1.0

    width: Screen.width
    // PANEL-1 — the surface is exactly the visible bar. There is no
    // transparent shadow canvas below it.
    readonly property int barHeight: 40
    // The reserved strip is the bar at this output's size, so a taller bar on a
    // denser monitor keeps windows clear of it.
    height: Math.round(panel.barHeight * panel.shellScale)
    visible: false
    // The bar has one nearly transparent, shadowless contextual backdrop from
    // edge to edge. Dense content capsules remain inset above it.
    color: CelestinaTheme.clear
    title: qsTr("Panel de Celestina")
    flags: Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = panel.reducedMotion;
        glassSettle.restart();
    }

    // PANEL-1 — the finite region the compositor should blur, in window
    // coordinates, collected from the continuous bar backdrop.
    //
    // Published from here rather than found from C++: an item declared in a QML
    // `Window` sits in an object tree whose shape is Qt's business, and walking
    // it guessed wrong twice — first finding nothing under the content item, and
    // an empty region is not "blur nothing", it is "blur everything". The QML
    // backdrop knows its exact geometry; this asks it.
    property var glassRects: []
    property var glassRegions: []

    function collectGlass() {
        const foundRects = [];
        const foundRegions = [];
        const walk = function(item) {
            for (let index = 0; index < item.children.length; ++index) {
                const child = item.children[index];
                if (child.objectName === "celestina-compositor-glass-region"
                    && child.visible
                    && child.width > 0 && child.height > 0) {
                    const rect = EdgeAttachedGeometry.mapRect(child);
                    foundRects.push(rect);
                    foundRegions.push({
                        "rect": rect,
                        "radius": child.radius,
                        "polygon": EdgeAttachedGeometry.mapPolygon(
                            child, child.polygon)
                    });
                }
                walk(child);
            }
        };
        walk(panel.contentItem);
        panel.glassRects = foundRects;
        panel.glassRegions = foundRegions;
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

    // Where the bar's content lives; this is also the complete window and the
    // exact strip reserved from application windows.
    // Everything the bar draws, laid out in the shell's own logical pixels and
    // then scaled to what this output needs. Keeping the layout unscaled is the
    // point: every measurement inside — the 40-pixel bar, the capsules at y=5,
    // the seam a contextual surface attaches to — stays the number the design
    // states, and only the last step to real pixels differs per monitor.
    Item {
        id: scene

        width: panel.width / panel.shellScale
        height: panel.barHeight
        transformOrigin: Item.TopLeft
        scale: panel.shellScale

        Item {
            id: bar

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: panel.barHeight
            readonly property var materialSilhouette:
                    EdgeAttachedGeometry.openBottomRectangle(width, height)

            CompositorGlassRegion {
                id: barBackdropRegion

                anchors.fill: parent
                blurAvailable: panel.compositorBlurAvailable
                fallbackColor: CelestinaTheme.glassTint
                radius: CelestinaTheme.radiusNone
                onBlurRegionChanged: panel.scheduleGlassCollection()

                GlassSurface {
                    anchors.fill: parent
                    objectName: "celestina-panel-backdrop-material"
                    backdropMode: GlassSurface.ExternalBackdrop
                    // Exactly as in a contextual menu, either the compositor or
                    // CompositorGlassRegion's declared fallback supplies the
                    // external sample beneath this very light material.
                    externalBackdropReady: true
                    captureEnabled: false
                    materialRole: GlassSurface.ContextualVeil
                    materialTint: backdropInk.materialTint
                    cornerRadius: CelestinaTheme.radiusNone
                    silhouettePath: bar.materialSilhouette.path
                    silhouetteEdgePath: bar.materialSilhouette.edgePath
                    elevation: 0
                }
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
            height: panel.barHeight
            // What the strip must leave for the widgets that follow it. Measured
            // by their own width rather than by `visible`, which also depends on
            // the window being shown: what matters here is whether a widget has
            // anything to occupy. An absent one is zero wide and reserves nothing,
            // its gap included.
            reservedWidth: leftFlank.roomFor(toolCluster) + leftFlank.roomFor(media)

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
                ink: backdropInk
                // The strip reports the gesture; the provider owns the protocol and
                // answers through `workspaces`, never through this call's return.
                onFocusRequested: (output, index) => panel.niriProvider.requestWorkspaceFocus(output, index)
                onMapRequested: (openerRect, anchorRect, workspaces) => panel.workspaceMapRequested(openerRect, anchorRect, workspaces)

                PanelPill {
                    barHeight: panel.barHeight
                    blurAvailable: panel.compositorBlurAvailable
                    ink: backdropInk
                }

            }

            PanelCluster {
                barHeight: panel.barHeight
                id: toolCluster

                spacing: CelestinaTheme.spaceXs
                blurAvailable: panel.compositorBlurAvailable
                ink: backdropInk

                CaptureButton {
                    id: captureButton

                    height: CelestinaTheme.controlHeightXs
                    blurAvailable: panel.compositorBlurAvailable
                    ownsGlass: false
                    ink: backdropInk
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.indicatorMenuRequested("capture", openerRect,
                                                     attachmentAnchorRect)

                    // The provider reports only what it could not do; a capture
                    // the compositor took over is not observable by this panel.
                    Connections {
                        function onScreenshotFailed(reason) {
                            captureButton.reportFailure();
                        }

                        target: panel.niriProvider
                    }
                }

                PanelActionButton {
                    barHeight: panel.barHeight
                    id: wallpaperButton

                    objectName: "celestina-wallpaper-button"
                    blurAvailable: panel.compositorBlurAvailable
                    ownsGlass: false
                    iconName: "image"
                    ink: backdropInk
                    helpText: qsTr("Cambiar el fondo de pantalla")
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.indicatorMenuRequested("wallpaper", openerRect,
                                                     attachmentAnchorRect)
                }
            }

            MediaMini {

                // PANEL-1 — one height for every reading, so the row aligns them.

                height: CelestinaTheme.controlHeightXs
                id: media
                objectName: "celestina-panel-media"

                anchors.verticalCenter: parent.verticalCenter
                reading: panel.provider("media")
                ink: backdropInk
                onToggleRequested: panel.providerSource.sendCommand("media", "PlayPause")

                PanelPill {
                    barHeight: panel.barHeight
                    blurAvailable: panel.compositorBlurAvailable
                    ink: backdropInk
                }

            }

        }

        Clock {
            id: clock

            ink: backdropInk
            onMenuRequested: (openerRect, attachmentAnchorRect) =>
                panel.indicatorMenuRequested("calendar", openerRect,
                                             attachmentAnchorRect)

            anchors.centerIn: bar

            PanelPill {
                barHeight: panel.barHeight
                blurAvailable: panel.compositorBlurAvailable
                // The clock is the one reading with open space either side of it,
                // so it is the one that can be held by a visibly elastic skin
                // without running into a neighbour.
                elasticWeld: true
                ink: backdropInk
            }

        }

        PanelFlank {
            id: rightFlank

            anchors.right: parent.right
            anchors.rightMargin: CelestinaTheme.spaceMd + CelestinaTheme.spaceSm
            anchors.left: clock.right
            anchors.leftMargin: CelestinaTheme.space2xl
            anchors.verticalCenter: bar.verticalCenter
            height: panel.barHeight
            trailing: true

            // PANEL-1 — this reading lays out its own children, so a pill

            // placed inside it became a cell of that row instead of a floor

            // under it. The wrapper gives the glass somewhere to sit that the

            // layout does not own, and reports the same size to the flank.

            Item {
                id: trayWrapper

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
                    preferences: panel.trayPreferences
                    ink: backdropInk
                    onActivated: (service, path, globalX, globalY) => panel.traySource.activate(service, path, globalX, globalY)
                    onSecondaryActivated: (service, path, globalX, globalY) => panel.traySource.secondaryActivate(service, path, globalX, globalY)
                    onMenuRequested: (service, path, globalX, globalY) => panel.trayMenuRequested(service, path, globalX, globalY)
                    onDrawerRequested: (openerRect, attachmentAnchorRect) =>
                        panel.trayDrawerRequested(openerRect, attachmentAnchorRect)

                }

                PanelPill {
                    barHeight: panel.barHeight
                    id: trayPill

                    blurAvailable: panel.compositorBlurAvailable
                    ink: backdropInk
                }

            }

            PanelCluster {
                barHeight: panel.barHeight
                id: connectivityCluster

                blurAvailable: panel.compositorBlurAvailable
                ink: backdropInk
                hasContent: sessionstatusBody.hasVisibleIndicator

                SessionStatus {
                    id: sessionstatusBody

                    network: panel.provider("network")
                    bluetooth: panel.provider("bluetooth")
                    ink: backdropInk
                    onIndicatorMenuRequested: (kind, openerRect,
                                               attachmentAnchorRect) =>
                        panel.indicatorMenuRequested(kind, openerRect,
                                                     attachmentAnchorRect)
                }
            }

            PanelCluster {
                barHeight: panel.barHeight
                id: levelCluster

                blurAvailable: panel.compositorBlurAvailable
                ink: backdropInk

                AudioLevel {
                    height: CelestinaTheme.controlHeightXs
                    reading: panel.provider("audio")
                    ink: backdropInk
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.indicatorMenuRequested("audio", openerRect,
                                                     attachmentAnchorRect)
                    onStepRequested: (direction) => panel.providerSource.sendCommand(
                        "audio", "volume-step", {"by": direction > 0 ? panel.levelStep : -panel.levelStep})
                }

                BrightnessLevel {
                    height: CelestinaTheme.controlHeightXs
                    reading: panel.provider("brightness")
                    outputName: panel.outputName
                    ink: backdropInk
                    blurAvailable: panel.compositorBlurAvailable
                    // Inside a semantic cluster, so the cluster owns the glass.
                    ownsGlass: false
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.indicatorMenuRequested("brightness", openerRect,
                                                     attachmentAnchorRect)
                    // The step names its own monitor: one helper serves every
                    // panel, and each panel speaks only for its mapped output.
                    onStepRequested: (direction) => panel.providerSource.sendCommand(
                        "brightness", "brightness-step", {
                            "by": direction > 0 ? panel.levelStep : -panel.levelStep,
                            "output": panel.outputName
                        })
                }
            }

            PanelCluster {
                barHeight: panel.barHeight
                id: utilityCluster

                // Compact buttons already reserve their complete pointer target;
                // four pixels between targets leave a clear icon rhythm without
                // recreating the former 24-pixel gaps inside one semantic group.
                spacing: CelestinaTheme.spaceXs
                blurAvailable: panel.compositorBlurAvailable
                ink: backdropInk

                NotificationIndicator {
                    height: CelestinaTheme.controlHeightXs
                    reading: panel.provider("notifications")
                    ink: backdropInk
                    // The panel asks; the host owns the surface that answers,
                    // exactly as it does for the menus.
                    onHistoryRequested: (openerRect, attachmentAnchorRect) =>
                        panel.notificationCentreRequested(openerRect,
                                                           attachmentAnchorRect)
                    onQuietToggled: panel.providerSource.sendCommand(
                        "notifications", "quiet-toggle")
                }

                PanelActionButton {
                    barHeight: panel.barHeight
                    objectName: "celestina-launcher-button"
                    blurAvailable: panel.compositorBlurAvailable
                    ownsGlass: false
                    iconName: "app-window"
                    ink: backdropInk
                    helpText: qsTr("Abrir el buscador de aplicaciones")
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.launcherRequested(openerRect, attachmentAnchorRect)
                }

                PanelActionButton {
                    barHeight: panel.barHeight
                    id: controlCentreButton

                    objectName: "celestina-control-centre-button"
                    blurAvailable: panel.compositorBlurAvailable
                    ownsGlass: false
                    iconName: "settings"
                    ink: backdropInk
                    helpText: qsTr("Abrir el centro de control")
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.controlCentreRequested(openerRect,
                                                     attachmentAnchorRect)
                }

                PanelActionButton {
                    barHeight: panel.barHeight
                    objectName: "celestina-clipboard-button"
                    blurAvailable: panel.compositorBlurAvailable
                    ownsGlass: false
                    iconName: "clipboard-paste"
                    ink: backdropInk
                    helpText: qsTr("Abrir el historial del portapapeles")
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.clipboardRequested(openerRect, attachmentAnchorRect)
                }

                PanelActionButton {
                    barHeight: panel.barHeight
                    objectName: "celestina-session-menu-button"
                    blurAvailable: panel.compositorBlurAvailable
                    ownsGlass: false
                    iconName: "power"
                    ink: backdropInk
                    helpText: qsTr("Abrir el menú de sesión")
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.sessionMenuRequested(openerRect,
                                                   attachmentAnchorRect)
                }

                SysMon {
                    reading: panel.provider("sysmon")
                    ink: backdropInk
                    blurAvailable: panel.compositorBlurAvailable
                    ownsGlass: false
                    onMenuRequested: (openerRect, attachmentAnchorRect) =>
                        panel.indicatorMenuRequested("performance", openerRect,
                                                     attachmentAnchorRect)
                }
            }

            PhoneStatus {
                barHeight: panel.barHeight
                onMenuRequested: (openerRect, attachmentAnchorRect) =>
                    panel.indicatorMenuRequested("phone", openerRect,
                                                 attachmentAnchorRect)

                // PANEL-1 — one height for every reading, so the row aligns them.

                height: CelestinaTheme.controlHeightXs
                width: Math.min(implicitWidth, rightFlank.width)
                blurAvailable: panel.compositorBlurAvailable
                connected: panel.phoneProvider.phoneConnected
                battery: panel.phoneProvider.phoneBattery
                charging: panel.phoneProvider.phoneCharging
                ink: backdropInk
            }
        }
    }


    FolderDialog {
        id: wallpaperPicker

        title: qsTr("Elegir carpeta de fondos")
        onAccepted: {
            panel.wallpaperFolderSelected(wallpaperPicker.selectedFolder);
            // The chooser temporarily retires the contextual surface. Reopen
            // the same anchored gallery immediately so its loading state and
            // then its thumbnails are visible without a second panel click.
            Qt.callLater(wallpaperButton.requestMenu);
        }
    }

}
