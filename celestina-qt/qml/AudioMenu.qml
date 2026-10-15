// What the session plays through and listens with, and every application that
// is currently making or taking sound.
//
// A card rather than a menu, for the same reason the wallpaper gallery and the
// calendar are: these are levels, and a level is moved rather than chosen. A
// slider inside a real `Menu` row fights the row's own click-to-activate.
//
// The device inventory and the per-application levels are asked for exactly
// once per opening. The provider deliberately keeps `wpctl status` and the
// per-node reads out of its two-second poll (2026-08-12 performance audit),
// so this card's first act is to ask for a fresh list.
//
// Nothing is painted optimistically: moving a slider asks, and the next
// provider snapshot answers. Applications are addressed by their node id,
// never by name — two ALSA clients share one.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "ProviderReading.js" as ProviderReading

SoftCard {
    id: root

    required property var providerSource

    readonly property var audio: ProviderReading.read(root.providerSource, "audio")
    readonly property bool hasReading: root.audio !== undefined
                                       && root.audio.volume !== undefined
    readonly property int volume: root.hasReading ? root.audio.volume : 0
    readonly property bool muted: root.hasReading && root.audio.muted === true
    readonly property bool hasMic: root.hasReading
                                   && root.audio.micVolume !== undefined
    readonly property int micVolume: root.hasMic ? root.audio.micVolume : 0
    readonly property bool micMuted: root.hasMic && root.audio.micMuted === true

    readonly property var outputs: root.hasReading
                                   && root.audio.outputs !== undefined
                                   ? root.audio.outputs : []
    readonly property var inputs: root.hasReading
                                  && root.audio.inputs !== undefined
                                  ? root.audio.inputs : []
    readonly property var playbackApps: root.hasReading
                                        && root.audio.playbackApps !== undefined
                                        ? root.audio.playbackApps : []
    readonly property var captureApps: root.hasReading
                                       && root.audio.captureApps !== undefined
                                       ? root.audio.captureApps : []

    function defaultName(devices, fallback) {
        for (const device of devices) {
            if (device.default === true)
                return device.name;
        }
        return fallback;
    }

    // The default device's own node id, or -1 before the inventory arrives.
    // The master sliders address it by id through the same verb an
    // application's slider uses: the session verbs are steps, and a slider
    // knows where it was put, so a delta would race the reading it was drawn
    // from. Muting keeps the session verb, which is about the session's
    // default rather than about one node.
    function defaultId(devices) {
        for (const device of devices) {
            if (device.default === true)
                return device.id;
        }
        return -1;
    }
    readonly property int defaultOutputId: root.defaultId(root.outputs)
    readonly property int defaultInputId: root.defaultId(root.inputs)

    // Whether the device pickers are showing. Collapsed by default: the usual
    // question is "how loud", not "through what", and the list is one tap away.
    property bool showingOutputs: false
    property bool showingInputs: false

    function send(verb, options) {
        if (root.providerSource)
            root.providerSource.sendCommand("audio", verb, options);
    }

    title: qsTr("Audio")
    subtitle: root.hasReading
              ? (root.muted ? qsTr("Silenciado, %1 %").arg(root.volume)
                            : qsTr("Volumen %1 %").arg(root.volume))
              : qsTr("Sin lectura de audio")
    iconName: root.muted ? "media-volume-muted" : "media-volume"

    // One `wpctl status` plus one read per application, which is this card's
    // whole subprocess budget.
    Component.onCompleted: root.send("devices-refresh", {})

    headerActions: [
        // The tools section this card used to end with, reduced to the one
        // thing it held: a section label and a whole row for a single action
        // was furniture, and the hierarchy is icon-first.
        BackdropIconButton {
            objectName: "celestina-audio-mixer-button"
            width: CelestinaTheme.controlHeightXs
            height: width
            ink: root.ink
            iconName: "settings"
            helpText: qsTr("Abrir el mezclador")
            Accessible.name: helpText
            onClicked: {
                root.send("open-mixer", {});
                root.dismissed();
            }
        }
    ]

    DevicePicker {
        width: parent.width
        ink: root.ink
        label: qsTr("Salida")
        devices: root.outputs
        expanded: root.showingOutputs
        onToggled: root.showingOutputs = !root.showingOutputs
        onChosen: (id) => root.send("set-default", {"id": id})
    }

    LevelRow {
        width: parent.width
        ink: root.ink
        label: root.defaultName(root.outputs, qsTr("Salida"))
        iconName: root.muted ? "media-volume-muted" : "media-volume"
        level: root.volume
        known: root.hasReading
        enabled: root.defaultOutputId >= 0
        actionIcon: root.muted ? "media-volume-muted" : "media-volume"
        actionHelpText: root.muted ? qsTr("Activar el sonido")
                                   : qsTr("Silenciar")
        actionSelected: root.muted
        onMoved: (target) => root.send(
            "node-volume", {"id": root.defaultOutputId, "percent": target})
        onActionTriggered: root.send("mute-toggle", {})
    }

    DevicePicker {
        width: parent.width
        visible: root.hasMic
        ink: root.ink
        label: qsTr("Entrada")
        devices: root.inputs
        expanded: root.showingInputs
        onToggled: root.showingInputs = !root.showingInputs
        onChosen: (id) => root.send("set-default", {"id": id})
    }

    LevelRow {
        width: parent.width
        visible: root.hasMic
        ink: root.ink
        label: root.defaultName(root.inputs, qsTr("Entrada"))
        iconName: root.micMuted ? "mic-off" : "mic"
        level: root.micVolume
        known: root.hasMic
        enabled: root.defaultInputId >= 0
        actionIcon: root.micMuted ? "mic-off" : "mic"
        actionHelpText: root.micMuted ? qsTr("Activar el micrófono")
                                      : qsTr("Silenciar el micrófono")
        actionSelected: root.micMuted
        onMoved: (target) => root.send(
            "node-volume", {"id": root.defaultInputId, "percent": target})
        onActionTriggered: root.send("mic-mute-toggle", {})
    }

    Text {
        width: parent.width
        visible: root.playbackApps.length > 0 || root.captureApps.length > 0
        height: CelestinaTheme.controlHeightXs
        verticalAlignment: Text.AlignVCenter
        text: qsTr("Aplicaciones")
        color: root.ink.muted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontMini
        font.weight: CelestinaTheme.weightDemiBold
    }

    // Counted, not listed, and the row reads its own entry.
    //
    // A `Repeater` given a JavaScript array rebuilds every delegate when that
    // array is replaced, and this provider replaces it after each command it
    // carries out as well as on its own poll. That destroyed the row under the
    // pointer mid-drag — taking its grab, its drag and everything it had asked
    // for — and the rebuilt row started again from the reading. The count only
    // changes when an application actually comes or goes.
    Repeater {
        model: root.playbackApps.length

        delegate: LevelRow {
            id: playbackRow

            required property int index

            readonly property var app: playbackRow.index < root.playbackApps.length
                                       ? root.playbackApps[playbackRow.index]
                                       : null

            objectName: playbackRow.app
                        ? "celestina-level-row-" + playbackRow.app.id : ""
            width: parent.width
            visible: playbackRow.app !== null
            ink: root.ink
            label: playbackRow.app ? playbackRow.app.name : ""
            iconName: playbackRow.app && playbackRow.app.muted
                      ? "media-volume-muted" : "media-volume"
            level: playbackRow.app ? playbackRow.app.volume : 0
            known: playbackRow.app !== null
            actionIcon: playbackRow.app && playbackRow.app.muted
                        ? "media-volume-muted" : "media-volume"
            actionHelpText: playbackRow.app && playbackRow.app.muted
                            ? qsTr("Activar el sonido") : qsTr("Silenciar")
            actionSelected: playbackRow.app !== null && playbackRow.app.muted
            onMoved: (target) => {
                if (playbackRow.app) {
                    root.send("node-volume",
                              {"id": playbackRow.app.id, "percent": target});
                }
            }
            onActionTriggered: {
                if (playbackRow.app)
                    root.send("node-mute-toggle", {"id": playbackRow.app.id});
            }
        }
    }

    // Counted rather than listed, for the reason above.
    Repeater {
        model: root.captureApps.length

        delegate: LevelRow {
            id: captureRow

            required property int index

            readonly property var app: captureRow.index < root.captureApps.length
                                       ? root.captureApps[captureRow.index]
                                       : null

            objectName: captureRow.app
                        ? "celestina-level-row-" + captureRow.app.id : ""
            width: parent.width
            visible: captureRow.app !== null
            ink: root.ink
            label: captureRow.app ? captureRow.app.name : ""
            iconName: captureRow.app && captureRow.app.muted ? "mic-off" : "mic"
            level: captureRow.app ? captureRow.app.volume : 0
            known: captureRow.app !== null
            actionIcon: captureRow.app && captureRow.app.muted ? "mic-off" : "mic"
            actionHelpText: captureRow.app && captureRow.app.muted
                            ? qsTr("Activar el micrófono")
                            : qsTr("Silenciar el micrófono")
            actionSelected: captureRow.app !== null && captureRow.app.muted
            onMoved: (target) => {
                if (captureRow.app) {
                    root.send("node-volume",
                              {"id": captureRow.app.id, "percent": target});
                }
            }
            onActionTriggered: {
                if (captureRow.app)
                    root.send("node-mute-toggle", {"id": captureRow.app.id});
            }
        }
    }

    Text {
        width: parent.width
        visible: root.hasReading
                 && root.playbackApps.length === 0
                 && root.captureApps.length === 0
        text: qsTr("Ninguna aplicación está usando el audio")
        color: root.ink.faint
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontMini
        wrapMode: Text.WordWrap
    }

    Text {
        width: parent.width
        visible: !root.hasReading
        text: qsTr("Sin dispositivo de audio legible")
        color: root.ink.faint
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontMini
        wrapMode: Text.WordWrap
    }
}
