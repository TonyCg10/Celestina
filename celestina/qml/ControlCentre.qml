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

Window {
    id: centre

    required property var providerSource
    required property bool reducedMotion

    signal dismissed()

    readonly property int cardWidth: 420
    readonly property int cardHeight: 700

    readonly property var providers: providerSource && providerSource.providers
                                     ? providerSource.providers : ({})
    readonly property var audio: centre.providers.audio
    readonly property var notifications: centre.providers.notifications
    readonly property var nightLight: centre.providers["night-light"]
    readonly property var caffeine: centre.providers.caffeine
    readonly property var power: centre.providers.power
    readonly property var network: centre.providers.network
    readonly property var bluetooth: centre.providers.bluetooth
    readonly property var settings: centre.providers.settings
    readonly property var weather: centre.providers.weather

    readonly property int levelStep: centre.settings && centre.settings.levelStep !== undefined
                                     ? centre.settings.levelStep : 5

    // The last outcome for each verb, so a control can say what happened to the
    // request it made rather than pretending it worked. Keyed by verb because
    // that is what a control is about; a newer request for the same verb
    // replaces the older one's report.
    property var outcomes: ({})
    property var awaiting: ({})

    width: cardWidth
    height: cardHeight
    color: CelestinaTheme.clear
    title: qsTr("Control centre")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = centre.reducedMotion;
        firstControl.forceActiveFocus();
    }

    function send(provider, verb, options) {
        if (!centre.providerSource)
            return;
        const id = centre.providerSource.sendCommand(provider, verb, options === undefined ? {} : options);
        if (id === 0) {
            // A request that could not even be sent is a failure now, not a
            // pending one that will never resolve.
            const failed = centre.outcomes;
            failed[verb] = {"state": "failed", "reason": qsTr("the shell could not send this")};
            centre.outcomes = failed;
            return;
        }
        const pending = centre.outcomes;
        pending[verb] = {"state": "pending", "reason": ""};
        centre.outcomes = pending;
        const waiting = centre.awaiting;
        waiting[id] = verb;
        centre.awaiting = waiting;
    }

    function outcomeOf(verb) {
        return centre.outcomes[verb] === undefined ? null : centre.outcomes[verb];
    }

    Connections {
        function onCommandResult(requestId, state, reason) {
            const verb = centre.awaiting[requestId];
            if (verb === undefined)
                return;
            const next = centre.outcomes;
            next[verb] = {"state": state, "reason": reason};
            centre.outcomes = next;
            if (state !== "pending") {
                const waiting = centre.awaiting;
                delete waiting[requestId];
                centre.awaiting = waiting;
            }
        }

        target: centre.providerSource
    }

    component ControlRow: Item {
        id: row

        required property string label
        // What the provider says, in the person's words. Empty when the
        // provider is not reporting at all.
        required property string reading
        property string verb: ""
        property var outcome: row.verb.length > 0 ? centre.outcomeOf(row.verb) : null
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
                    const outcome = row.outcome;
                    if (outcome && outcome.state === "pending")
                        return qsTr("%1 · asking…").arg(row.reading);
                    if (outcome && outcome.state === "failed") {
                        return outcome.reason.length > 0
                               ? qsTr("%1 · failed: %2").arg(row.reading).arg(outcome.reason)
                               : qsTr("%1 · failed").arg(row.reading);
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

    Item {
        id: scene

        anchors.fill: parent

        GlassCard {
            anchors.fill: parent
            backdropSource: scene
            Accessible.role: Accessible.Dialog
            Accessible.name: qsTr("Control centre")

            Column {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceLg
                spacing: CelestinaTheme.spaceXs

                Keys.onEscapePressed: centre.dismissed()

                Text {
                    width: parent.width
                    text: qsTr("Control centre")
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                    font.weight: CelestinaTheme.weightDemiBold
                    bottomPadding: CelestinaTheme.spaceSm
                }

                ControlRow {
                    label: qsTr("Volume")
                    reading: centre.audio && centre.audio.volume !== undefined
                             ? (centre.audio.muted ? qsTr("%1 %, muted").arg(centre.audio.volume)
                                                   : qsTr("%1 %").arg(centre.audio.volume))
                             : qsTr("no readable device")
                    verb: "mute-toggle"

                    Row {
                        spacing: CelestinaTheme.spaceXs

                        CelestinaButton {
                            id: firstControl

                            text: qsTr("−")
                            helpText: qsTr("Quieter by %1 %").arg(centre.levelStep)
                            onClicked: centre.send("audio", "volume-step", {"by": -centre.levelStep})
                        }

                        CelestinaButton {
                            text: qsTr("+")
                            helpText: qsTr("Louder by %1 %").arg(centre.levelStep)
                            onClicked: centre.send("audio", "volume-step", {"by": centre.levelStep})
                        }

                        CelestinaSwitch {
                            checked: centre.audio !== undefined && centre.audio.muted === true
                            Accessible.name: qsTr("Silence the speaker")
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
                    label: qsTr("Night light")
                    reading: centre.nightLight === undefined
                             ? qsTr("no provider")
                             : (centre.nightLight.active ? qsTr("on") : qsTr("off"))
                    verb: "night-light-toggle"

                    CelestinaSwitch {
                        checked: centre.nightLight !== undefined
                                 && centre.nightLight.active === true
                        Accessible.name: qsTr("Night light")
                        onToggled: {
                            checked = Qt.binding(() => centre.nightLight !== undefined
                                                 && centre.nightLight.active === true);
                            centre.send("night-light", "night-light-toggle");
                        }
                    }
                }

                ControlRow {
                    label: qsTr("Stay awake")
                    reading: centre.caffeine === undefined
                             ? qsTr("no provider")
                             : (centre.caffeine.active ? qsTr("on") : qsTr("off"))
                    verb: "caffeine-toggle"

                    CelestinaSwitch {
                        checked: centre.caffeine !== undefined
                                 && centre.caffeine.active === true
                        Accessible.name: qsTr("Keep the session awake")
                        onToggled: {
                            checked = Qt.binding(() => centre.caffeine !== undefined
                                                 && centre.caffeine.active === true);
                            centre.send("caffeine", "caffeine-toggle");
                        }
                    }
                }

                ControlRow {
                    label: qsTr("Silence notifications")
                    reading: centre.notifications === undefined
                             ? qsTr("another program serves notifications")
                             : (centre.notifications.quiet ? qsTr("silenced")
                                                           : qsTr("allowed"))
                    verb: "quiet-toggle"

                    CelestinaSwitch {
                        enabled: centre.notifications !== undefined
                        checked: centre.notifications !== undefined
                                 && centre.notifications.quiet === true
                        Accessible.name: qsTr("Silence notifications")
                        onToggled: {
                            checked = Qt.binding(() => centre.notifications !== undefined
                                                 && centre.notifications.quiet === true);
                            centre.send("notifications", "quiet-toggle");
                        }
                    }
                }

                ControlRow {
                    label: qsTr("Power profile")
                    reading: centre.power && centre.power.active !== undefined
                             ? centre.power.active : qsTr("no daemon")
                    verb: "cycle"

                    CelestinaButton {
                        text: qsTr("Next")
                        enabled: centre.power !== undefined
                        helpText: qsTr("Switch to the next profile the daemon offers")
                        onClicked: centre.send("power", "cycle")
                    }
                }

                // Read-only on purpose: this shell is not a network or
                // Bluetooth manager, and a switch here would promise one.
                ControlRow {
                    label: qsTr("Network")
                    reading: centre.network && centre.network.connection !== undefined
                             ? centre.network.connection : qsTr("nothing is carrying this session")
                }

                ControlRow {
                    label: qsTr("Bluetooth")
                    reading: centre.bluetooth && centre.bluetooth.first !== undefined
                             ? centre.bluetooth.first : qsTr("nothing connected")
                }

                // Absent rather than stale: the provider withdraws a reading
                // the moment it stops being current, so this row simply has
                // nothing to say instead of showing an old temperature.
                ControlRow {
                    label: centre.weather && centre.weather.label !== undefined
                           ? qsTr("Weather — %1").arg(centre.weather.label)
                           : qsTr("Weather")
                    reading: centre.weather && centre.weather.celsius !== undefined
                             ? qsTr("%1 °C").arg(centre.weather.celsius)
                             : qsTr("no current reading")
                }

                MonthCalendar {
                    width: parent.width
                }
            }
        }
    }
}
