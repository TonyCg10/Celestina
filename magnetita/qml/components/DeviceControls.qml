import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

Item {
    id: root

    required property DevicesModel devices
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

    // The mirror surfaces sit between the actions and the media, each taking
    // no height at all while it has nothing to show.
    readonly property bool mirrorSpeaks: root.devices.mirrorLabel.length > 0
                                         && root.devices.mirrorLabel !== "Listo para reflejar"

    height: actionRow.height + mirrorLine.height + pairRow.height
            + mirrorSettings.height + 10 + mediaCard.height

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
            iconName: "settings"
            density: CelestinaButton.Prominent
            enabled: root.devices.mirrorAvailable
            role: CelestinaButton.Ghost
            checkable: true
            checked: mirrorSettings.visible
            helpText: qsTr("Ajustes del espejo")
            // The sheet is the single truth; re-bind after the click so the
            // button never holds a `checked` of its own.
            onClicked: {
                checked = Qt.binding(function() { return mirrorSettings.visible })
                mirrorSettings.visible = !mirrorSettings.visible
            }
        }
    }

    // The mirror's own state, in words, under the row that owns it. Only when
    // it has something to say the icon cannot: a failure, or work in progress.
    Text {
        id: mirrorLine
        anchors.top: actionRow.bottom
        anchors.topMargin: root.mirrorSpeaks ? CelestinaTheme.spaceXs : 0
        width: parent.width
        visible: root.mirrorSpeaks
        height: visible ? implicitHeight : 0
        text: root.devices.mirrorLabel
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
        wrapMode: Text.WordWrap
    }

    // The pairing code, only while the phone is actually showing one: that is
    // the only moment a code exists to type, and the only time this is not
    // clutter.
    RowLayout {
        id: pairRow
        anchors.top: mirrorLine.bottom
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

    MirrorSettingsSheet {
        id: mirrorSettings
        anchors.top: pairRow.bottom
        anchors.topMargin: visible ? CelestinaTheme.spaceSm : 0
        width: parent.width
        visible: false
        height: visible ? implicitHeight : 0
        devices: root.devices
    }

    MediaCard {
        id: mediaCard
        anchors.top: mirrorSettings.bottom
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
