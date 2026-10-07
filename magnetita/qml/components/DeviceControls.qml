import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

Item {
    id: root

    required property DevicesModel devices
    // The item the menu blurs: the page surface, which is not an ancestor of
    // the overlay the menu lives in, so the menu never captures itself.
    required property Item backdrop
    required property int primaryIndex
    required property int mediaIndex
    required property int mediaControlIndex

    function valueAt(values, index, fallback) {
        return index >= 0 && index < values.length ? values[index] : fallback
    }

    readonly property bool paired: primaryIndex >= 0
                                   && primaryIndex < devices.devicePaired.length
                                   && devices.devicePaired[primaryIndex] === "true"
    readonly property bool mounted: primaryIndex >= 0
                                    && primaryIndex < devices.deviceMounts.length
                                    && devices.deviceMounts[primaryIndex].length > 0
    readonly property bool playing: valueAt(devices.deviceMediaPlaying,
                                            mediaIndex, "false") === "true"
    readonly property bool hasMedia: mediaIndex >= 0
    readonly property string mediaPlayer: valueAt(devices.deviceMediaPlayers,
                                                   mediaIndex, "")
    readonly property string mediaTitle: valueAt(devices.deviceMediaTitles,
                                                  mediaIndex, "")
    readonly property string mediaArtist: valueAt(devices.deviceMediaArtists,
                                                   mediaIndex, "")
    readonly property string mediaAlbum: valueAt(devices.deviceMediaAlbums,
                                                  mediaIndex, "")
    readonly property string mediaNowPlaying: valueAt(
                                                   devices.deviceMediaNowPlaying,
                                                   mediaIndex, "")
    readonly property string mediaArtwork: valueAt(devices.deviceMediaArtwork,
                                                    mediaIndex, "")
    readonly property real mediaLength: Number(valueAt(devices.deviceMediaLengths,
                                                        mediaIndex, "-1"))
    readonly property real mediaPosition: Number(valueAt(devices.deviceMediaPositions,
                                                          mediaIndex, "-1"))
    readonly property bool mediaNext: valueAt(devices.deviceMediaNext,
                                              mediaIndex, "false") === "true"
    readonly property bool mediaPrevious: valueAt(devices.deviceMediaPrevious,
                                                  mediaIndex, "false") === "true"
    readonly property bool mediaCanPlay: valueAt(devices.deviceMediaPlay,
                                                 mediaIndex, "false") === "true"
    readonly property bool mediaCanPause: valueAt(devices.deviceMediaPause,
                                                  mediaIndex, "false") === "true"
    readonly property string mediaProgress: valueAt(devices.deviceMediaProgress,
                                                     mediaIndex, "unavailable")

    // The mirror's status line lives in the options menu, so the page only
    // keeps the pairing row between the actions and the media, which takes no
    // height at all while it has nothing to show.
    readonly property bool mirrorSpeaks: root.devices.mirrorLabel.length > 0
                                         && root.devices.mirrorLabel !== "Listo para reflejar"

    height: actionRow.height + pairRow.height + 10 + mediaCard.height

    // Icon-first: every action is its own glyph with the suite's uniform hover
    // circle, and the mirror pair — open it, configure it — sits together as
    // one capsule because they are two halves of the same thing.
    //
    // The mirror lives here rather than in a card of its own below the media:
    // it is a thing you do to the phone, exactly like ringing it or opening its
    // files, and it belongs in the row where those live.
    RowLayout {
        id: actionRow
        width: parent.width
        height: CelestinaTheme.controlHeightXl
        spacing: CelestinaTheme.spaceSm

        // The mirror's state moves on the phone's schedule, not on a bus event,
        // so the daemon publishes no change signal for it and this polls while
        // the controls are on screen.
        Timer {
            interval: 2000
            running: root.visible
            repeat: true
            triggeredOnStart: true
            onTriggered: root.devices.reloadMirror()
        }

        CelestinaIconButton {
            iconName: "folder-open"
            density: CelestinaButton.Prominent
            visible: root.mounted
            role: CelestinaButton.Primary
            helpText: qsTr("Abrir los archivos del móvil")
            onClicked: root.devices.openMount(root.primaryIndex)
        }

        CelestinaIconButton {
            iconName: "key"
            density: CelestinaButton.Prominent
            visible: !root.paired
            role: CelestinaButton.Primary
            helpText: qsTr("Emparejar el móvil")
            onClicked: root.devices.pairDevice(root.primaryIndex)
        }

        CelestinaIconButton {
            iconName: "bell"
            density: CelestinaButton.Prominent
            visible: root.paired
            helpText: qsTr("Hacer sonar el móvil")
            onClicked: root.devices.ringDevice(root.primaryIndex)
        }

        CelestinaIconButton {
            iconName: "unplug"
            density: CelestinaButton.Prominent
            visible: root.paired
            helpText: qsTr("Desvincular el móvil")
            onClicked: root.devices.unpairDevice(root.primaryIndex)
        }

        Item { Layout.fillWidth: true }

        CelestinaIconButton {
            id: mirrorButton
            iconName: "monitor"
            density: CelestinaButton.Prominent
            enabled: root.devices.mirrorAvailable
            role: CelestinaButton.Primary
            // A true toggle: the shared button paints a checked one Selected.
            checkable: true
            checked: root.devices.mirrorActive
            helpText: root.devices.mirrorActive
                      ? qsTr("Detener el espejo") : qsTr("Reflejar la pantalla del móvil")
            // A checkable Button flips `checked` on click; re-bind so the glyph
            // keeps showing the daemon's confirmed state, not an optimistic one.
            onClicked: {
                checked = Qt.binding(function() { return root.devices.mirrorActive })
                if (root.devices.mirrorActive)
                    root.devices.stopMirror()
                else
                    root.devices.startMirror()
            }
        }

        CelestinaIconButton {
            id: settingsButton
            iconName: "settings"
            density: CelestinaButton.Prominent
            enabled: root.devices.mirrorAvailable
            role: CelestinaButton.Ghost
            checkable: true
            checked: mirrorMenu.visible
            helpText: qsTr("Ajustes del espejo")
            onClicked: mirrorMenu.popupBeside(settingsButton, false)
        }
    }

    // The pairing code, only while the phone is actually showing one: that is
    // the only moment a code exists to type, and the only time this is not
    // clutter.
    RowLayout {
        id: pairRow
        anchors.top: actionRow.bottom
        anchors.topMargin: visible ? CelestinaTheme.spaceXs : 0
        width: parent.width
        visible: root.devices.mirrorCanPair
        height: visible ? implicitHeight : 0
        spacing: CelestinaTheme.spaceSm

        Text {
            text: qsTr("Código de vinculación")
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
        }

        CelestinaTextField {
            id: codeField
            Layout.preferredWidth: 120
            inputMethodHints: Qt.ImhDigitsOnly
            maximumLength: 6
            validator: RegularExpressionValidator { regularExpression: /[0-9]{0,6}/ }
            onAccepted: pairButton.clicked()
        }

        CelestinaIconButton {
            id: pairButton
            iconName: "link"
            role: CelestinaButton.Primary
            helpText: qsTr("Vincular")
            enabled: codeField.text.length === 6
            onClicked: {
                root.devices.pairMirror(codeField.text)
                codeField.text = ""
            }
        }
    }

    // Everything about the mirror that is not start/stop: its status in words,
    // where the sound plays and whether the phone's screen goes dark. Nothing is
    // painted optimistically: the marks follow the daemon's confirmed values,
    // and a press only asks. The blur samples the page surface, which holds the
    // backdrop and the cards the menu floats over.
    GlassContextMenu {
        id: mirrorMenu
        backdropSource: root.backdrop

        // The phone's state moves on its own schedule; this is where failures
        // and work in progress are said, since the icon cannot.
        GlassMenuItem {
            visible: root.mirrorSpeaks
            enabled: false
            height: visible ? implicitHeight : 0
            text: root.devices.mirrorLabel
        }

        GlassMenuSection { text: qsTr("Sonido") }

        GlassMenuItem {
            text: qsTr("En el móvil")
            icon.name: root.devices.mirrorAudio === "phone" ? "check" : ""
            icon.source: root.devices.mirrorAudio === "phone"
                ? CelestinaTheme.fallbackIcon("check") : ""
            Accessible.checked: root.devices.mirrorAudio === "phone"
            onTriggered: root.devices.setMirrorOption("audio", "phone")
        }

        GlassMenuItem {
            text: qsTr("En el PC")
            icon.name: root.devices.mirrorAudio === "desktop" ? "check" : ""
            icon.source: root.devices.mirrorAudio === "desktop"
                ? CelestinaTheme.fallbackIcon("check") : ""
            Accessible.checked: root.devices.mirrorAudio === "desktop"
            onTriggered: root.devices.setMirrorOption("audio", "desktop")
        }

        GlassMenuItem {
            text: qsTr("Apagar la pantalla del móvil")
            icon.name: root.devices.mirrorScreenOff ? "check" : ""
            icon.source: root.devices.mirrorScreenOff
                ? CelestinaTheme.fallbackIcon("check") : ""
            Accessible.checked: root.devices.mirrorScreenOff
            onTriggered: root.devices.setMirrorOption(
                             "screenOff",
                             root.devices.mirrorScreenOff ? "false" : "true")
        }

        // scrcpy cannot be reconfigured mid-stream, so a change made while the
        // mirror is up applies the next time it opens.
        GlassMenuItem {
            visible: root.devices.mirrorActive
            enabled: false
            height: visible ? implicitHeight : 0
            text: qsTr("El espejo está abierto: los cambios se aplican la próxima vez.")
        }
    }

    MediaCard {
        id: mediaCard
        anchors.top: pairRow.bottom
        anchors.topMargin: 10
        width: parent.width
        hasMedia: root.hasMedia
        player: root.mediaPlayer
        title: root.mediaTitle.length > 0 ? root.mediaTitle
               : root.mediaNowPlaying.length > 0 ? root.mediaNowPlaying
               : root.mediaPlayer
        artist: root.mediaArtist
        album: root.mediaAlbum
        artworkUrl: root.mediaArtwork
        positionMs: root.mediaPosition
        lengthMs: root.mediaLength
        playing: root.playing
        canPlay: root.mediaCanPlay
        canPause: root.mediaCanPause
        canPrevious: root.mediaPrevious
        canNext: root.mediaNext
        progressKind: root.mediaProgress
        onPreviousRequested: root.devices.mediaPrevious(root.mediaControlIndex)
        onPlayPauseRequested: root.devices.mediaPlayPause(root.mediaControlIndex)
        onNextRequested: root.devices.mediaNext(root.mediaControlIndex)
    }
}
