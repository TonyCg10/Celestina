pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.fluorita 1.0

// Which soundtrack, which subtitles, and how fast.
//
// Three questions with one thing in common: each is a short list of known
// answers with one of them in force. So they share a menu rather than growing
// three controls, and each section appears only when the file gives it
// something to choose between — a menu offering "Audio: pista 1" on a file with
// one soundtrack is furniture.
//
// Nothing here decides anything. What may be chosen and what is in force come
// from the player, which learned both from the backend's own reports.
GlassContextMenu {
    id: menu

    required property FluoritaPlayer player

    // The rates the domain offers. Read from it rather than spelled again here,
    // so a surface cannot present a speed the model would clamp away.
    readonly property var rates: [0.5, 0.75, 1.0, 1.25, 1.5, 2.0]

    CelestinaSectionLabel {
        visible: menu.player.choosableAudio
        text: qsTr("Audio")
    }

    Repeater {
        model: menu.player.choosableAudio ? menu.player.audioStreams : []

        delegate: GlassMenuItem {
            required property string modelData
            required property int index

            text: modelData
            icon.name: menu.player.audioStream === index ? "check" : ""
            icon.source: menu.player.audioStream === index
                ? CelestinaTheme.fallbackIcon("check") : ""
            Accessible.checked: menu.player.audioStream === index
            onTriggered: menu.player.selectAudioStream(index)
        }
    }

    CelestinaSectionLabel {
        visible: menu.player.choosableSubtitles
        text: qsTr("Subtítulos")
    }

    // Off is a row like any other, and it is the one most often wanted.
    GlassMenuItem {
        visible: menu.player.choosableSubtitles
        text: qsTr("Sin subtítulos")
        icon.name: menu.player.subtitleStream === -1 ? "check" : ""
        icon.source: menu.player.subtitleStream === -1
            ? CelestinaTheme.fallbackIcon("check") : ""
        Accessible.checked: menu.player.subtitleStream === -1
        onTriggered: menu.player.selectSubtitleStream(-1)
    }

    Repeater {
        model: menu.player.choosableSubtitles ? menu.player.subtitleStreams : []

        delegate: GlassMenuItem {
            required property string modelData
            required property int index

            text: modelData
            icon.name: menu.player.subtitleStream === index ? "check" : ""
            icon.source: menu.player.subtitleStream === index
                ? CelestinaTheme.fallbackIcon("check") : ""
            Accessible.checked: menu.player.subtitleStream === index
            onTriggered: menu.player.selectSubtitleStream(index)
        }
    }

    CelestinaSectionLabel {
        visible: menu.player.timed
        text: qsTr("Al terminar")
    }

    Repeater {
        // The order is the domain's list of modes, and the position *is* the
        // token: the words live here, the meaning lives there.
        model: menu.player.timed
            ? [qsTr("Detenerse"), qsTr("Seguir con la carpeta"), qsTr("Repetir")]
            : []

        delegate: GlassMenuItem {
            required property string modelData
            required property int index

            text: modelData
            icon.name: menu.player.continuation === index ? "check" : ""
            icon.source: menu.player.continuation === index
                ? CelestinaTheme.fallbackIcon("check") : ""
            Accessible.checked: menu.player.continuation === index
            onTriggered: menu.player.setContinuationMode(index)
        }
    }

    CelestinaSectionLabel {
        visible: menu.player.timed
        text: qsTr("Velocidad")
    }

    Repeater {
        model: menu.player.timed ? menu.rates : []

        delegate: GlassMenuItem {
            required property real modelData

            readonly property bool inForce: Math.abs(menu.player.speed - modelData) < 0.01

            text: modelData === 1.0 ? qsTr("Normal") : qsTr("%1×").arg(modelData)
            icon.name: inForce ? "check" : ""
            icon.source: inForce ? CelestinaTheme.fallbackIcon("check") : ""
            Accessible.checked: inForce
            onTriggered: menu.player.playAt(modelData)
        }
    }
}
