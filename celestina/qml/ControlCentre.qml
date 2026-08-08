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

    signal dismissed()

    readonly property int cardWidth: 420
    readonly property int cardHeight: 700

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


    width: cardWidth
    height: cardHeight
    color: CelestinaTheme.clear
    title: qsTr("Centro de control")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = centre.reducedMotion;
        firstControl.forceActiveFocus();
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
                        + CelestinaTheme.spaceSm

        Column {
            id: text

            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - holder.width - CelestinaTheme.spaceMd
            spacing: 1

            Text {
                width: parent.width
                text: row.label
                color: CelestinaTheme.text
                elide: Text.ElideRight
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
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
                       ? CelestinaTheme.danger : CelestinaTheme.textMuted
                elide: Text.ElideRight
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }
        }

        Item {
            id: holder

            anchors.right: parent.right
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
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onPressed: centre.dismissed()
    }

    Item {
        id: scene

        width: centre.cardWidth
        height: centre.cardHeight
        anchors.centerIn: parent

        // Anything the card itself does not handle stops here rather than
        // falling through to the dismissal area behind it. It is the first
        // child, so every control declared after it is still reached first.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        }

        GlassCard {
            anchors.fill: parent
            backdropSource: scene
            Accessible.role: Accessible.Dialog
            Accessible.name: qsTr("Centro de control")

            Column {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceLg
                spacing: CelestinaTheme.spaceXs

                Keys.onEscapePressed: centre.dismissed()

                Text {
                    width: parent.width
                    text: qsTr("Centro de control")
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                    font.weight: CelestinaTheme.weightDemiBold
                    bottomPadding: CelestinaTheme.spaceSm
                }

                ControlRow {
                    label: qsTr("Volumen")
                    reading: centre.audio && centre.audio.volume !== undefined
                             ? (centre.audio.muted ? qsTr("%1 %, silenciado").arg(centre.audio.volume)
                                                   : qsTr("%1 %").arg(centre.audio.volume))
                             : qsTr("sin dispositivo legible")
                    provider: "audio"
                    verb: "mute-toggle"

                    Row {
                        spacing: CelestinaTheme.spaceXs

                        CelestinaButton {
                            id: firstControl

                            text: qsTr("−")
                            helpText: qsTr("Bajar %1 %").arg(centre.levelStep)
                            onClicked: centre.send("audio", "volume-step", {"by": -centre.levelStep})
                        }

                        CelestinaButton {
                            text: qsTr("+")
                            helpText: qsTr("Subir %1 %").arg(centre.levelStep)
                            onClicked: centre.send("audio", "volume-step", {"by": centre.levelStep})
                        }

                        CelestinaSwitch {
                            checked: centre.audio !== undefined && centre.audio.muted === true
                            Accessible.name: qsTr("Silenciar el altavoz")
                            // The provider decides what `checked` becomes; this
                            // only asks, and puts the switch back where the
                            // reading says it is until an answer arrives.
                            onToggled: {
                                checked = Qt.binding(() => centre.audio !== undefined
                                                     && centre.audio.muted === true);
                                centre.send("audio", "mute-toggle");
                            }
                        }
                    }
                }

                ControlRow {
                    label: qsTr("Luz nocturna")
                    reading: centre.nightLight === undefined
                             ? qsTr("sin proveedor")
                             : (centre.nightLight.active ? qsTr("encendida") : qsTr("apagada"))
                    provider: "night-light"
                    verb: "night-light-toggle"

                    CelestinaSwitch {
                        checked: centre.nightLight !== undefined
                                 && centre.nightLight.active === true
                        Accessible.name: qsTr("Luz nocturna")
                        onToggled: {
                            checked = Qt.binding(() => centre.nightLight !== undefined
                                                 && centre.nightLight.active === true);
                            centre.send("night-light", "night-light-toggle");
                        }
                    }
                }

                ControlRow {
                    label: qsTr("Mantener despierto")
                    reading: centre.caffeine === undefined
                             ? qsTr("sin proveedor")
                             : (centre.caffeine.active ? qsTr("encendida") : qsTr("apagada"))
                    provider: "caffeine"
                    verb: "caffeine-toggle"

                    CelestinaSwitch {
                        checked: centre.caffeine !== undefined
                                 && centre.caffeine.active === true
                        Accessible.name: qsTr("Mantener la sesión despierta")
                        onToggled: {
                            checked = Qt.binding(() => centre.caffeine !== undefined
                                                 && centre.caffeine.active === true);
                            centre.send("caffeine", "caffeine-toggle");
                        }
                    }
                }

                ControlRow {
                    label: qsTr("Silenciar notificaciones")
                    reading: centre.notifications === undefined
                             ? qsTr("otro programa sirve las notificaciones")
                             : (centre.notifications.quiet ? qsTr("silenciadas")
                                                           : qsTr("permitidas"))
                    provider: "notifications"
                    verb: "quiet-toggle"

                    CelestinaSwitch {
                        enabled: centre.notifications !== undefined
                        checked: centre.notifications !== undefined
                                 && centre.notifications.quiet === true
                        Accessible.name: qsTr("Silenciar notificaciones")
                        onToggled: {
                            checked = Qt.binding(() => centre.notifications !== undefined
                                                 && centre.notifications.quiet === true);
                            centre.send("notifications", "quiet-toggle");
                        }
                    }
                }

                ControlRow {
                    label: qsTr("Perfil de energía")
                    reading: centre.power && centre.power.active !== undefined
                             ? centre.power.active : qsTr("sin demonio")
                    provider: "power"
                    verb: "cycle"

                    CelestinaButton {
                        text: qsTr("Siguiente")
                        enabled: centre.power !== undefined
                        helpText: qsTr("Cambiar al siguiente perfil que ofrece el demonio")
                        onClicked: centre.send("power", "cycle")
                    }
                }

                // Read-only on purpose: this shell is not a network or
                // Bluetooth manager, and a switch here would promise one.
                ControlRow {
                    label: qsTr("Red")
                    reading: centre.network && centre.network.connection !== undefined
                             ? centre.network.connection : qsTr("nada transporta esta sesión")
                }

                // The adapter's own state comes first: "nothing connected" was
                // the same sentence for a radio that is off, one that is not
                // there, and one that is on and simply idle.
                ControlRow {
                    label: qsTr("Bluetooth")
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
                }

                // Absent rather than stale: the provider withdraws a reading
                // the moment it stops being current, so this row simply has
                // nothing to say instead of showing an old temperature.
                ControlRow {
                    label: centre.weather && centre.weather.label !== undefined
                           ? qsTr("Tiempo — %1").arg(centre.weather.label)
                           : qsTr("Tiempo")
                    reading: centre.weather && centre.weather.celsius !== undefined
                             ? qsTr("%1 °C").arg(centre.weather.celsius)
                             : qsTr("sin lectura actual")
                }

                MonthCalendar {
                    width: parent.width
                }
            }
        }
    }
}
