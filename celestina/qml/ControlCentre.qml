// One place to change what the panel already reports.
//
// Every control here reads its provider and writes through a verb that already
// existed: nothing in this file talks to a device, and nothing invents a second
// path to one. That is the whole point — a control that sent its own command
// would be a second source of truth about the same hardware.
//
// And no control paints what it asked for. A switch shows the state its
// provider last reported, and the request's own life — pending, confirmed,
// failed — is shown beside it. A switch that flipped on click would be lying
// every time the write failed, which is exactly the case a person needs to see.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window
import "ProviderReading.js" as ProviderReading

Window {
    id: centre

    required property var providerSource
    required property bool reducedMotion
    property alias anchoredFromPanel: placement.anchoredFromPanel
    property alias openerRect: placement.openerRect
    property alias attachmentAnchorRect: placement.attachmentAnchorRect
    property alias attachmentStartY: placement.attachmentStartY
    property alias compositorBlurAvailable: scene.compositorBlurAvailable
    property alias glassRects: scene.glassRects
    property alias glassRegions: scene.glassRegions
    readonly property int anchorGap: placement.anchorGap

    signal dismissed()

    BackdropInk {
        id: backdropInk
    }

    // The selected Velo composition is intentionally a roomy control surface,
    // not the old compact context menu stretched over more providers.
    readonly property int cardWidth: 530
    readonly property int cardHeight: 732
    readonly property real cardX: placement.x
    readonly property real cardY: placement.y

    // Every reading goes through the one access point, so a key inserted while
    // this window is open becomes visible instead of staying missing until the
    // window is built again. `weather` is the one that really arrives late: it
    // has nothing to publish until a location is set and a request succeeds.
    readonly property var audio: ProviderReading.read(centre.providerSource, "audio")
    readonly property var notifications: ProviderReading.read(centre.providerSource, "notifications")
    readonly property var nightLight: ProviderReading.read(centre.providerSource, "night-light")
    readonly property var caffeine: ProviderReading.read(centre.providerSource, "caffeine")
    readonly property var power: ProviderReading.read(centre.providerSource, "power")
    readonly property var network: ProviderReading.read(centre.providerSource, "network")
    readonly property var bluetooth: ProviderReading.read(centre.providerSource, "bluetooth")
    readonly property var settings: ProviderReading.read(centre.providerSource, "settings")
    readonly property var weather: ProviderReading.read(centre.providerSource, "weather")

    readonly property int levelStep: centre.settings && centre.settings.levelStep !== undefined
                                     ? centre.settings.levelStep : 5


    // How much larger this output needs the shell drawn; see shellscale.h. The
    // host supplies it and divides the geometry it hands this overlay by it, so
    // every number here is already in unscaled units.
    property real shellScale: 1.0
    readonly property real surfaceWidth: centre.width / centre.shellScale
    readonly property real surfaceHeight: centre.height / centre.shellScale

    width: Math.round(cardWidth * shellScale)
    height: Math.round(cardHeight * shellScale)
    color: CelestinaTheme.clear
    title: qsTr("Centro de control")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = centre.reducedMotion;
        firstControl.forceActiveFocus();
    }

    PanelPopupPlacement {
        id: placement

        surfaceWidth: centre.surfaceWidth
        surfaceHeight: centre.surfaceHeight
        contentWidth: centre.cardWidth
        contentHeight: centre.cardHeight
        edgeInset: 0
    }

    onVisibleChanged: {
        if (visible)
            Qt.callLater(scene.reveal);
    }

    // The request ledger lives on the bridge, so one owner reports every
    // surface's requests and none of them is lost when its window closes.
    readonly property var ledger: centre.providerSource
                                  ? centre.providerSource.requests : null

    // `immediate` is the contract every verb here has always had: the helper
    // answering `accepted` is the whole answer. Nothing sends these a later
    // `confirmed`, so waiting for one would leave a control saying it is
    // asking for ever — which is exactly what the connectivity contract would
    // have done to them.
    function send(provider, verb, options) {
        if (centre.ledger)
            centre.ledger.send(provider, verb, options === undefined ? {} : options, verb, "immediate");
    }

    // A verb is unique within its own provider, which is exactly how the
    // ledger keys it — so a lookup names both, like the send did.
    function outcomeOf(provider, verb) {
        if (!centre.ledger || centre.ledger.revision < 0)
            return null;

        const known = centre.ledger.stateOf(provider, verb);
        return known.state === undefined ? null : known;
    }

    function isPending(provider, verb) {
        return centre.ledger !== null && centre.ledger.revision >= 0
               && centre.ledger.isPending(provider, verb);
    }

    component ControlRow: Item {
        id: row

        required property string label
        // What the provider says, in the person's words. Empty when the
        // provider is not reporting at all.
        required property string reading
        required property BackdropInk ink
        property string iconName: ""
        property int iconTone: CelestinaIcon.Primary
        property string status: ""
        property color statusColor: row.ink.accent
        // A row names the provider as well as the verb, because the ledger
        // keys a request by both and a lookup that knew only half of it would
        // read another provider's request of the same name.
        property string provider: ""
        property string verb: ""
        property var outcome: row.verb.length > 0
                              ? centre.outcomeOf(row.provider, row.verb) : null
        default property alias control: holder.data

        width: parent ? parent.width : 0
        implicitHeight: Math.max(text.implicitHeight, holder.implicitHeight)
                        + CelestinaTheme.spaceMd

        CelestinaIcon {
            id: rowIcon

            anchors.left: parent.left
            anchors.leftMargin: CelestinaTheme.spaceLg
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.space2xl
            height: width
            visible: row.iconName.length > 0
            name: row.iconName
            fallbackName: row.iconName
            tone: row.iconTone
            tintOverride: row.iconTone === CelestinaIcon.Primary
                          ? row.ink.primary : row.ink.accent
        }

        Column {
            id: text

            anchors.left: rowIcon.visible ? rowIcon.right : parent.left
            anchors.leftMargin: rowIcon.visible ? CelestinaTheme.spaceMd
                                                : CelestinaTheme.spaceLg
            anchors.verticalCenter: parent.verticalCenter
            anchors.right: holder.left
            anchors.rightMargin: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceXs

            Text {
                width: parent.width
                text: row.label
                color: row.ink.primary
                elide: Text.ElideRight
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                font.weight: CelestinaTheme.weightDemiBold
            }

            Text {
                width: parent.width
                // The provider's own reading, and only then what happened to
                // the last request about it.
                text: {
                    // `accepted` is still waiting: the helper ran a tool and
                    // nothing has observed an effect yet.
                    if (row.verb.length > 0 && centre.isPending(row.provider, row.verb))
                        return qsTr("%1 · preguntando…").arg(row.reading);

                    const outcome = row.outcome;
                    if (outcome && outcome.state === "failed") {
                        // The helper's own reason is English by contract and is
                        // logged rather than shown. What reaches the surface is
                        // this shell's own sentence, chosen from a typed cause.
                        if (outcome.cause === "unsent")
                            return qsTr("%1 · falló: el shell no pudo enviarlo").arg(row.reading);

                        if (outcome.cause === "generation-lost")
                            return qsTr("%1 · falló: el asistente se reinició").arg(row.reading);

                        return qsTr("%1 · falló").arg(row.reading);
                    }
                    return row.reading;
                }
                color: row.outcome && row.outcome.state === "failed"
                       ? row.ink.danger : row.ink.muted
                elide: Text.ElideRight
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }

            Text {
                width: parent.width
                visible: row.status.length > 0
                text: row.status
                textFormat: Text.PlainText
                color: row.statusColor
                elide: Text.ElideRight
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.weight: CelestinaTheme.weightDemiBold
            }
        }

        Item {
            id: holder

            anchors.right: parent.right
            anchors.rightMargin: CelestinaTheme.spaceLg
            anchors.verticalCenter: parent.verticalCenter
            implicitWidth: childrenRect.width
            implicitHeight: childrenRect.height
        }
    }

    // A click anywhere outside the card closes this surface.
    //
    // The surface is the whole output, not the card: that is what makes an
    // outside click land here at all, and it is also what makes the panel
    // button that opened this close it in one click rather than two. While the
    // overlay is up the button is behind it, so the click never reaches the
    // panel, never re-enters `toggle()`, and focus returns exactly once.
    // Everything this overlay draws, in unscaled units, scaled once on its way
    // to the output. The two visual children below name this as their parent
    // instead of being nested inside it: their order, and so their stacking,
    // stays exactly as it reads.
    Item {
        id: shellScene
        objectName: "celestina-shell-scene"

        width: centre.surfaceWidth
        height: centre.surfaceHeight
        transformOrigin: Item.TopLeft
        scale: centre.shellScale
    }

    MouseArea {
        parent: shellScene
        // Below the card, said rather than implied. These two are reparented
        // instead of nested, and the order bindings are evaluated in is not
        // the order they are written in — which put this catch-all on top of
        // the card and made a click inside it dismiss the overlay.
        z: -1
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onPressed: centre.dismissed()
    }

    SoftOverlayCard {
        parent: shellScene
        id: scene

        ink: backdropInk
        x: centre.cardX
        y: centre.cardY
        width: centre.cardWidth
        height: centre.cardHeight
        reducedMotion: centre.reducedMotion
        accessibleName: qsTr("Centro de control")
        attachedToTop: centre.anchoredFromPanel
        openerRect: centre.openerRect
        attachmentAnchorRect: centre.attachmentAnchorRect
        attachmentStartY: centre.attachmentStartY
        surfacePosition: Qt.point(centre.cardX, centre.cardY)

        Column {
                id: contentColumn

                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceMd
                spacing: CelestinaTheme.spaceSm

                Keys.onEscapePressed: centre.dismissed()

                MenuHeader {
                    width: parent.width
                    ink: backdropInk
                    title: qsTr("Centro de control")
                    iconName: "settings"
                    trailingIconName: "go-up"
                }

                Item {
                    id: quickControls

                    objectName: "celestina-control-centre-quick-controls"
                    width: parent.width
                    implicitHeight: 238

                    MenuSection { ink: backdropInk }

                    Column {
                        id: quickControlsColumn

                        anchors.fill: parent
                        spacing: 0

                        ControlRow {
                            height: 78
                            ink: backdropInk
                            label: qsTr("Volumen")
                            iconName: "media-volume"
                            reading: centre.audio && centre.audio.volume !== undefined
                                     ? (centre.audio.muted ? qsTr("%1 %, silenciado").arg(centre.audio.volume)
                                                           : qsTr("%1 %").arg(centre.audio.volume))
                                     : qsTr("sin dispositivo legible")
                            provider: "audio"
                            verb: "mute-toggle"

                            Row {
                                spacing: CelestinaTheme.spaceXs

                                BackdropButton {
                                    id: firstControl

                                    ink: backdropInk
                                    text: qsTr("−")
                                    helpText: qsTr("Bajar %1 %").arg(centre.levelStep)
                                    onClicked: centre.send("audio", "volume-step", {"by": -centre.levelStep})
                                }

                                BackdropButton {
                                    ink: backdropInk
                                    text: qsTr("+")
                                    helpText: qsTr("Subir %1 %").arg(centre.levelStep)
                                    onClicked: centre.send("audio", "volume-step", {"by": centre.levelStep})
                                }

                                CelestinaSwitch {
                                    id: audioMuteSwitch

                                    checked: centre.audio !== undefined && centre.audio.muted === true
                                    Accessible.name: qsTr("Silenciar el altavoz")
                                    // The provider decides what `checked` becomes;
                                    // this only asks and restores the binding.
                                    onToggled: {
                                        audioMuteSwitch.checked = Qt.binding(
                                            () => centre.audio !== undefined
                                                  && centre.audio.muted === true);
                                        centre.send("audio", "mute-toggle");
                                    }
                                }
                            }
                        }

                        Rectangle {
                            x: CelestinaTheme.spaceXl
                            width: parent.width - CelestinaTheme.spaceXl * 2
                            height: CelestinaTheme.borderHairline
                            color: backdropInk.divider
                        }

                        Item {
                            width: parent.width
                            height: 78

                            Item {
                                anchors.left: parent.left
                                anchors.top: parent.top
                                anchors.bottom: parent.bottom
                                width: parent.width / 2

                                ControlRow {
                                    id: nightLightRow

                                    anchors.fill: parent
                                    ink: backdropInk
                                    label: qsTr("Luz nocturna")
                                    iconName: "sun"
                                    reading: centre.nightLight === undefined
                                             ? qsTr("sin proveedor")
                                             : (centre.nightLight.active ? qsTr("encendida") : qsTr("apagada"))
                                    provider: "night-light"
                                    verb: "night-light-toggle"

                                    CelestinaSwitch {
                                        id: nightLightSwitch

                                        checked: centre.nightLight !== undefined
                                                 && centre.nightLight.active === true
                                        Accessible.name: qsTr("Luz nocturna")
                                        onToggled: {
                                            nightLightSwitch.checked = Qt.binding(
                                                () => centre.nightLight !== undefined
                                                      && centre.nightLight.active === true);
                                            centre.send("night-light", "night-light-toggle");
                                        }
                                    }
                                }
                            }

                            Item {
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.bottom: parent.bottom
                                width: parent.width / 2

                                ControlRow {
                                    id: notificationsRow

                                    anchors.fill: parent
                                    ink: backdropInk
                                    label: qsTr("No molestar")
                                    iconName: "bell-off"
                                    reading: centre.notifications === undefined
                                             ? qsTr("servidor externo")
                                             : (centre.notifications.quiet ? qsTr("activado")
                                                                           : qsTr("desactivado"))
                                    provider: "notifications"
                                    verb: "quiet-toggle"

                                    CelestinaSwitch {
                                        id: notificationsSwitch

                                        enabled: centre.notifications !== undefined
                                        checked: centre.notifications !== undefined
                                                 && centre.notifications.quiet === true
                                        Accessible.name: qsTr("Silenciar notificaciones")
                                        onToggled: {
                                            notificationsSwitch.checked = Qt.binding(
                                                () => centre.notifications !== undefined
                                                      && centre.notifications.quiet === true);
                                            centre.send("notifications", "quiet-toggle");
                                        }
                                    }
                                }
                            }

                            Rectangle {
                                anchors.horizontalCenter: parent.horizontalCenter
                                anchors.top: parent.top
                                anchors.topMargin: CelestinaTheme.spaceLg
                                anchors.bottom: parent.bottom
                                anchors.bottomMargin: CelestinaTheme.spaceLg
                                width: CelestinaTheme.borderHairline
                                color: backdropInk.divider
                            }
                        }

                        Rectangle {
                            x: CelestinaTheme.spaceXl
                            width: parent.width - CelestinaTheme.spaceXl * 2
                            height: CelestinaTheme.borderHairline
                            color: backdropInk.divider
                        }

                        Item {
                            width: parent.width
                            height: 80

                            Item {
                                anchors.left: parent.left
                                anchors.top: parent.top
                                anchors.bottom: parent.bottom
                                width: parent.width / 2

                                ControlRow {
                                    id: caffeineRow

                                    anchors.fill: parent
                                    ink: backdropInk
                                    label: qsTr("Mantener despierto")
                                    iconName: "leaf"
                                    iconTone: centre.caffeine !== undefined
                                              && centre.caffeine.active === true
                                              ? CelestinaIcon.Accent
                                              : CelestinaIcon.Primary
                                    reading: centre.caffeine === undefined
                                             ? qsTr("sin proveedor")
                                             : (centre.caffeine.active ? qsTr("encendido") : qsTr("apagado"))
                                    provider: "caffeine"
                                    verb: "caffeine-toggle"

                                    CelestinaSwitch {
                                        id: caffeineSwitch

                                        checked: centre.caffeine !== undefined
                                                 && centre.caffeine.active === true
                                        Accessible.name: qsTr("Mantener la sesión despierta")
                                        onToggled: {
                                            caffeineSwitch.checked = Qt.binding(
                                                () => centre.caffeine !== undefined
                                                      && centre.caffeine.active === true);
                                            centre.send("caffeine", "caffeine-toggle");
                                        }
                                    }
                                }
                            }

                            Item {
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.bottom: parent.bottom
                                width: parent.width / 2

                                ControlRow {
                                    id: powerRow

                                    anchors.fill: parent
                                    ink: backdropInk
                                    label: qsTr("Energía")
                                    iconName: "gauge"
                                    reading: centre.power && centre.power.active !== undefined
                                             ? centre.power.active : qsTr("sin demonio")
                                    provider: "power"
                                    verb: "cycle"

                                    BackdropButton {
                                        ink: backdropInk
                                        text: qsTr("Siguiente")
                                        enabled: centre.power !== undefined
                                        helpText: qsTr("Cambiar al siguiente perfil que ofrece el demonio")
                                        onClicked: centre.send("power", "cycle")
                                    }
                                }
                            }

                            Rectangle {
                                anchors.horizontalCenter: parent.horizontalCenter
                                anchors.top: parent.top
                                anchors.topMargin: CelestinaTheme.spaceLg
                                anchors.bottom: parent.bottom
                                anchors.bottomMargin: CelestinaTheme.spaceLg
                                width: CelestinaTheme.borderHairline
                                color: backdropInk.divider
                            }
                        }
                    }
                }

                Item {
                    id: connectivityGroup

                    objectName: "celestina-control-centre-connectivity"
                    width: parent.width
                    implicitHeight: 126

                    MenuSection {
                        ink: backdropInk
                    }

                    Item {
                        anchors.fill: parent

                        Item {
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: parent.width / 2

                            // Read-only on purpose: this shell is not a network
                            // manager, and a switch here would promise one.
                            ControlRow {
                                id: networkRow

                                anchors.fill: parent
                                ink: backdropInk
                                label: qsTr("Wi-Fi")
                                iconName: "wifi"
                                iconTone: centre.network && centre.network.kind === "wifi"
                                          ? CelestinaIcon.Accent
                                          : CelestinaIcon.Primary
                                reading: centre.network && centre.network.connection !== undefined
                                         ? centre.network.connection : qsTr("sin conexión")
                                status: centre.network && centre.network.kind !== undefined
                                        ? qsTr("Conectado") : qsTr("Sin conexión")
                            }
                        }

                        Item {
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: parent.width / 2

                            ControlRow {
                                id: bluetoothRow

                                anchors.fill: parent
                                ink: backdropInk
                                label: qsTr("Bluetooth")
                                iconName: "bluetooth"
                                iconTone: CelestinaIcon.Device
                                reading: {
                                    if (!centre.bluetooth || centre.bluetooth.adapter === undefined)
                                        return qsTr("sin lectura");

                                    if (centre.bluetooth.adapter === "absent")
                                        return qsTr("sin adaptador");

                                    if (centre.bluetooth.adapter === "off")
                                        return qsTr("apagado");

                                    return centre.bluetooth.first !== undefined
                                           ? centre.bluetooth.first : qsTr("nada conectado");
                                }
                                status: {
                                    if (!centre.bluetooth || centre.bluetooth.adapter === undefined)
                                        return "";

                                    if (centre.bluetooth.adapter === "off")
                                        return qsTr("Apagado");

                                    if (centre.bluetooth.adapter === "absent")
                                        return qsTr("No disponible");

                                    return centre.bluetooth.first !== undefined
                                           ? qsTr("Conectado") : qsTr("Disponible");
                                }
                                statusColor: backdropInk.accent
                            }
                        }

                        Rectangle {
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.top: parent.top
                            anchors.topMargin: CelestinaTheme.spaceLg
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: CelestinaTheme.spaceLg
                            width: CelestinaTheme.borderHairline
                            color: backdropInk.divider
                        }
                    }
                }

                Item {
                    objectName: "celestina-control-centre-calendar"
                    width: parent.width
                    implicitHeight: 264

                    MenuSection {
                        ink: backdropInk
                    }

                    Column {
                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceMd
                        spacing: CelestinaTheme.spaceSm

                        Item {
                            id: weatherBlock

                            width: parent.width
                            height: CelestinaTheme.controlHeightXl

                            Column {
                                anchors.left: parent.left
                                anchors.right: temperature.left
                                anchors.rightMargin: CelestinaTheme.spaceMd
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: CelestinaTheme.spaceXs

                                Text {
                                    width: parent.width
                                    text: qsTr("Tiempo")
                                    color: backdropInk.primary
                                    font.family: CelestinaTheme.sansFamily
                                    font.pixelSize: CelestinaTheme.fontBody
                                    font.weight: CelestinaTheme.weightDemiBold
                                    elide: Text.ElideRight
                                }

                                Text {
                                    width: parent.width
                                    text: centre.weather && centre.weather.label !== undefined
                                          ? centre.weather.label
                                          : qsTr("sin lectura actual")
                                    color: backdropInk.muted
                                    font.family: CelestinaTheme.sansFamily
                                    font.pixelSize: CelestinaTheme.fontRowSecondary
                                    elide: Text.ElideRight
                                }
                            }

                            Text {
                                id: temperature

                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                text: centre.weather && centre.weather.celsius !== undefined
                                      ? qsTr("%1°").arg(centre.weather.celsius)
                                      : ""
                                color: backdropInk.primary
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontHeaderCollapsed
                                font.weight: CelestinaTheme.weightDemiBold
                            }
                        }

                        Rectangle {
                            width: parent.width
                            height: CelestinaTheme.borderHairline
                            color: backdropInk.divider
                        }

                        MonthCalendar {
                            id: calendar

                            width: parent.width
                            dayCellHeight: CelestinaTheme.spaceXl
                            ink: backdropInk
                        }
                    }
                }
        }
    }
}
