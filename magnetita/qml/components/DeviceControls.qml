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

    height: actionRow.height + 10 + mediaCard.height

    RowLayout {
        id: actionRow
        width: parent.width
        height: CelestinaTheme.controlHeightXl
        spacing: 8

        CelestinaButton {
            Layout.fillWidth: true
            density: CelestinaButton.Prominent
            visible: root.mounted
            role: CelestinaButton.Primary
            text: "Abrir archivos"
            onClicked: root.devices.openMount(root.primaryIndex)
        }

        CelestinaButton {
            Layout.fillWidth: true
            density: CelestinaButton.Prominent
            visible: !root.paired
            role: CelestinaButton.Primary
            text: "Emparejar"
            onClicked: root.devices.pairDevice(root.primaryIndex)
        }

        CelestinaButton {
            Layout.fillWidth: true
            density: CelestinaButton.Prominent
            visible: root.paired
            text: "Hacer sonar"
            onClicked: root.devices.ringDevice(root.primaryIndex)
        }

        CelestinaButton {
            Layout.fillWidth: true
            density: CelestinaButton.Prominent
            visible: root.paired
            text: "Desvincular"
            onClicked: root.devices.unpairDevice(root.primaryIndex)
        }
    }

    MediaCard {
        id: mediaCard
        anchors.top: actionRow.bottom
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
