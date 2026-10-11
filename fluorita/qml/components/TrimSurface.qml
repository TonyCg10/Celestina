pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.fluorita 1.0
// The render surface is hand-written C++ and therefore its own namespace.
import org.celestina.fluorita.render 1.0

// Trimming one video, in its edit window (ADR 0009 as amended, PRV-1).
//
// The film plays above a `TrimBar`; moving a handle asks the trim to move
// the span and seeks the picture to that frame, and play plays the span
// only: it starts at the span's start and stops, back at it, when the
// confirmed position reaches its end. Saving offers the picture editor's two
// outcomes by the author's names and nothing between them; while the child
// writes, its progress shows and the save can be cancelled. Leaving with a
// span chosen asks the editor's question.
//
// The trim is raster-class: the result is a new, re-encoded film, and the
// surface says so beside the span, with what it will leave out (another
// container, streams beyond the main video and one audio) before it is
// saved.
//
// It owns no truth: the span and the saving state are the trim's, the
// picture and the position the player's. The surface holds the player's
// render context for as long as the renderer is live (`holdsSurface`), and
// says `released` when it let go, so whoever closes the window waits for
// that rather than for a timer.
Item {
    id: surface

    required property FluoritaTrim trim
    required property FluoritaPlayer player

    // The trim is over: unchanged, discarded, or saved on the way out.
    signal closed()
    // The render context is gone; the player may be dropped.
    signal released()

    readonly property bool holdsSurface: video.rendererLive
    readonly property bool playing: surface.player.state === "reproduciendo"
    // Set while a save chosen on the way out is in flight.
    property bool leaving: false
    // A film starts playing when it opens; the trim pauses it once, at its
    // first confirmed state, so a window that opens is not a film that plays.
    property bool settled: false

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Recorte de vídeo")

    // Leaving: at once when no span was chosen, through the three choices
    // when one was. The window's close, the × and Escape all come here.
    function leave() {
        if (surface.trim.saving) {
            return
        }
        if (!surface.trim.edited) {
            surface.closed()
            return
        }
        closeQuestion.shown = true
    }

    function saveAndStay(replace) {
        surface.leaving = false
        surface.trim.saveTrim(replace)
    }

    function saveAndLeave(replace) {
        closeQuestion.shown = false
        surface.leaving = true
        surface.trim.saveTrim(replace)
    }

    // Plays the span: from its start unless the picture is already inside it.
    function togglePlay() {
        if (surface.playing) {
            surface.player.pause()
            return
        }
        const at = surface.player.positionSeconds
        if (at < surface.trim.trimStart || at >= surface.trim.trimEnd - surface.trim.minimumSeconds) {
            surface.player.seek(surface.trim.trimStart)
        }
        surface.player.play()
    }

    Connections {
        target: surface.player
        function onDurationSecondsChanged() {
            if (surface.player.durationSeconds > 0) {
                surface.trim.setLength(surface.player.durationSeconds)
            }
        }
        function onStateChanged() {
            if (surface.settled) {
                return
            }
            if (surface.player.state === "reproduciendo") {
                surface.settled = true
                surface.player.pause()
            } else if (surface.player.state === "pausado") {
                surface.settled = true
            }
        }
        // Play keeps to the span: the confirmed position reaching its end
        // stops the picture and takes it back to the start.
        function onPositionSecondsChanged() {
            if (surface.playing && surface.player.positionSeconds >= surface.trim.trimEnd) {
                surface.player.pause()
                surface.player.seek(surface.trim.trimStart)
            }
        }
    }

    Connections {
        target: surface.trim
        function onKeyChanged() {
            surface.settled = false
        }
        // A save chosen on the way out landed: the way out is taken.
        function onSavedKeyChanged() {
            if (surface.trim.savedKey !== "" && surface.leaving) {
                surface.leaving = false
                surface.closed()
            }
        }
        // One that failed or was cancelled leaves the trim open with its
        // notice, and a later close asks again. Asked a turn later, because
        // a save that lands stops "saving" a moment before its key arrives.
        function onSavingChanged() {
            if (!surface.trim.saving && surface.leaving) {
                Qt.callLater(surface.settleLeaving)
            }
        }
    }

    function settleLeaving() {
        if (surface.leaving && !surface.trim.saving && surface.trim.savedKey === "") {
            surface.leaving = false
        }
    }

    CelestinaBackdrop {
        anchors.fill: parent
    }

    MpvVideo {
        id: video

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: controls.top
        anchors.margins: CelestinaTheme.spaceLg
        handle: surface.player.renderHandle

        onContextCreated: surface.player.surfaceReady()
        onContextReleased: {
            surface.player.surfaceReleased()
            surface.released()
        }
        onContextFailed: surface.player.surfaceFailed()
    }

    // The player's own words when there is no picture: opening, or why not.
    CelestinaSectionLabel {
        anchors.centerIn: video
        visible: surface.player.errorMessage !== ""
        text: surface.player.errorMessage
    }

    Column {
        id: controls

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: CelestinaTheme.spaceLg
        spacing: CelestinaTheme.spaceSm

        // Where the span starts, how long it is and what the result is, and
        // where it ends.
        Item {
            width: parent.width
            height: kept.implicitHeight

            CelestinaSectionLabel {
                anchors.left: parent.left
                text: trimBar.clock(surface.trim.trimStart)
            }

            CelestinaSectionLabel {
                id: kept

                anchors.horizontalCenter: parent.horizontalCenter
                text: qsTr("Se conserva %1 · Vídeo nuevo").arg(
                    trimBar.clock(surface.trim.trimEnd - surface.trim.trimStart))
            }

            CelestinaSectionLabel {
                anchors.right: parent.right
                text: trimBar.clock(surface.trim.trimEnd)
            }
        }

        TrimBar {
            id: trimBar

            width: parent.width
            enabled: !surface.trim.saving && surface.trim.lengthSeconds > 0
            duration: surface.trim.lengthSeconds
            start: surface.trim.trimStart
            end: surface.trim.trimEnd
            minimumGap: surface.trim.minimumSeconds
            step: surface.trim.minimumSeconds
            position: surface.player.positionSeconds
            onStartMoved: function(seconds) {
                surface.trim.setSpan(seconds, surface.trim.trimEnd)
                surface.player.seek(surface.trim.trimStart)
            }
            onEndMoved: function(seconds) {
                surface.trim.setSpan(surface.trim.trimStart, seconds)
                surface.player.seek(surface.trim.trimEnd)
            }
        }

        // What the result leaves out, said before it is saved: another
        // container, or streams beyond the main video and one audio.
        CelestinaSectionLabel {
            objectName: "trimContainerNotice"
            anchors.horizontalCenter: parent.horizontalCenter
            visible: surface.trim.containerNotice !== ""
            text: surface.trim.containerNotice
        }

        // While the child writes: how far it is, and the way to stop it.
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: CelestinaTheme.spaceSm
            visible: surface.trim.saving

            Item {
                id: progress

                objectName: "trimProgress"
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(controls.width / 2, CelestinaTheme.space3xl * 8)
                height: CelestinaTheme.spaceXs
                visible: surface.trim.saving

                Accessible.role: Accessible.ProgressBar
                Accessible.name: qsTr("Guardando el recorte")

                Rectangle {
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusPill
                    color: CelestinaTheme.divider
                }

                Rectangle {
                    width: parent.width * Math.max(0, Math.min(1, surface.trim.progress))
                    height: parent.height
                    radius: CelestinaTheme.radiusPill
                    color: CelestinaTheme.accent
                }
            }

            CelestinaIconButton {
                iconName: "circle-stop"
                helpText: qsTr("Cancelar")
                onClicked: surface.trim.cancel()
            }
        }

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: CelestinaTheme.spaceXs

            CelestinaIconButton {
                iconName: surface.playing ? "media-pause" : "media-play"
                helpText: surface.playing ? qsTr("Pausar") : qsTr("Reproducir")
                enabled: !surface.trim.saving && surface.trim.lengthSeconds > 0
                onClicked: surface.togglePlay()
            }

            // The two outcomes, and nothing between them, by the names the
            // author gave them; a span that is the whole film saves nothing.
            CelestinaIconButton {
                iconName: "copy"
                helpText: qsTr("Guardar ambas")
                role: CelestinaButton.Primary
                enabled: surface.trim.edited && !surface.trim.saving
                onClicked: surface.saveAndStay(false)
            }

            CelestinaIconButton {
                iconName: "check"
                helpText: qsTr("Guardar solo la editada")
                enabled: surface.trim.edited && !surface.trim.saving
                onClicked: surface.saveAndStay(true)
            }

            CelestinaIconButton {
                iconName: "x"
                helpText: qsTr("Cerrar")
                enabled: !surface.trim.saving
                onClicked: surface.leave()
            }
        }

        CelestinaSectionLabel {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: surface.trim.notice !== ""
            text: surface.trim.notice
        }
    }

    EditCloseQuestion {
        id: closeQuestion

        objectName: "trimCloseQuestion"
        anchors.fill: parent
        z: 2
        message: qsTr("Este vídeo tiene cambios sin guardar.")
        onCopyChosen: surface.saveAndLeave(false)
        onReplaceChosen: surface.saveAndLeave(true)
        onDiscardChosen: {
            closeQuestion.shown = false
            surface.closed()
        }
    }

    // Space plays and pauses the span, Escape leaves, Ctrl+S keeps both.
    // The handles take the arrows; what they do not take arrives here.
    Keys.onPressed: function(event) {
        if (surface.trim.saving) {
            return
        }
        if (event.key === Qt.Key_Space) {
            surface.togglePlay()
            event.accepted = true
        } else if (event.key === Qt.Key_Escape) {
            surface.leave()
            event.accepted = true
        } else if (event.matches(StandardKey.Save) && surface.trim.edited) {
            surface.saveAndStay(false)
            event.accepted = true
        }
    }
}
