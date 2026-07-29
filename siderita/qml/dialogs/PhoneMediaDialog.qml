import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

CelestinaModalLayer {
    id: root

    required property var controller
    // FolderView contract: width, height and focusView().
    required property var owner
    required property Item backdrop
    property bool requested: false
    property string deviceId: ""
    readonly property int phoneIndex: {
        controller.phoneRevision
        for (let index = 0; index < controller.phoneNames.length; ++index) {
            const candidate = controller.phoneInfo(index)
            if (candidate.length > 0 && candidate[0] === root.deviceId)
                return index
        }
        return -1
    }
    readonly property var info: {
        controller.phoneRevision
        return phoneIndex >= 0 ? controller.phoneInfo(phoneIndex) : []
    }
    readonly property string phoneName: info.length > 1 ? info[1] : "Móvil"
    readonly property bool connected: info.length > 3 && info[3] === "1"
    readonly property string player: info.length > 6 ? info[6] : ""
    readonly property string title: info.length > 7 ? info[7] : ""
    readonly property string artist: info.length > 8 ? info[8] : ""
    readonly property string artworkUrl: info.length > 10 ? info[10] : ""
    readonly property bool playing: info.length > 11 && info[11] === "1"
    readonly property bool canPause: info.length > 12 && info[12] === "1"
    readonly property bool canNext: info.length > 13 && info[13] === "1"
    readonly property bool canPrevious: info.length > 14 && info[14] === "1"
    readonly property real lengthMs: info.length > 15 ? Number(info[15]) : -1
    readonly property real positionMs: info.length > 16 ? Number(info[16]) : -1
    readonly property bool hasMedia: player.length > 0 || title.length > 0
    readonly property bool hasProgress: lengthMs > 0 && positionMs >= 0
    readonly property real progress: hasProgress
            ? Math.max(0, Math.min(1, positionMs / lengthMs)) : 0
    readonly property string mediaDescription:
            !hasMedia ? "Nada reproduciéndose"
          : artist.length > 0 ? title + ", " + artist : title

    anchors.fill: parent
    z: 72
    shown: requested && phoneIndex >= 0
    onDismissRequested: closeDialog()
    onPhoneIndexChanged: if (phoneIndex < 0) requested = false
    onShownChanged: {
        if (shown)
            Qt.callLater(closeButton.forceActiveFocus)
        else if (owner)
            Qt.callLater(owner.focusView)
    }

    function openPhone(index) {
        const candidate = controller.phoneInfo(index)
        if (candidate.length === 0)
            return
        deviceId = candidate[0]
        requested = true
    }

    function closeDialog() {
        requested = false
    }

    Shortcut {
        sequence: "Escape"
        enabled: root.shown
        onActivated: root.closeDialog()
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(500, root.owner.width - 48)
        height: Math.min(280, root.owner.height - 48)
        backdropSource: root.backdrop
        Accessible.role: Accessible.Dialog
        Accessible.name: "Multimedia de " + root.phoneName
        Accessible.description: root.mediaDescription

        MouseArea { anchors.fill: parent }

        Text {
            id: heading

            x: 20
            y: 17
            width: closeButton.x - x - 12
            text: root.phoneName
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideRight
        }

        Row {
            anchors.left: heading.left
            anchors.top: heading.bottom
            anchors.topMargin: 4
            spacing: CelestinaTheme.spaceXs

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 7
                height: width
                radius: width / 2
                color: root.connected
                       ? CelestinaTheme.success : CelestinaTheme.danger
                Accessible.ignored: true
            }

            Text {
                text: root.connected ? "Conectado" : "Desconectado"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
            }
        }

        CelestinaIconButton {
            id: closeButton

            anchors.right: parent.right
            anchors.rightMargin: 14
            anchors.top: parent.top
            anchors.topMargin: 12
            iconName: "x"
            Accessible.name: "Cerrar"
            onClicked: root.closeDialog()
        }

        CelestinaSurface {
            id: mediaCard

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 20
            anchors.rightMargin: 20
            y: 70
            height: 140
            role: CelestinaSurface.Elevated
            Accessible.role: Accessible.Pane
            Accessible.name: "Control multimedia"
            Accessible.description: root.mediaDescription

            Rectangle {
                id: artwork

                x: 14
                anchors.verticalCenter: parent.verticalCenter
                width: 112
                height: width
                radius: CelestinaTheme.radiusButton
                color: CelestinaTheme.mediaArtworkMid
                clip: true

                Image {
                    id: artworkImage

                    anchors.fill: parent
                    source: root.artworkUrl
                    sourceSize: Qt.size(384, 384)
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    visible: status === Image.Ready
                    Accessible.ignored: true
                }

                CelestinaIcon {
                    anchors.centerIn: parent
                    width: CelestinaTheme.glyphTile
                    height: width
                    name: "music"
                    fallbackName: "audio-x-generic"
                    tone: CelestinaIcon.Overlay
                    visible: artworkImage.status !== Image.Ready
                    Accessible.ignored: true
                }
            }

            Item {
                anchors.left: artwork.right
                anchors.leftMargin: 18
                anchors.right: parent.right
                anchors.rightMargin: 18
                anchors.top: parent.top
                anchors.bottom: parent.bottom

                Text {
                    id: mediaTitle

                    anchors.left: parent.left
                    anchors.right: parent.right
                    y: 14
                    text: root.hasMedia && root.title.length > 0
                          ? root.title : "Nada reproduciéndose"
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideRight
                }

                Text {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: mediaTitle.bottom
                    anchors.topMargin: 2
                    text: root.artist
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    elide: Text.ElideRight
                }

                Rectangle {
                    id: progressTrack

                    anchors.left: parent.left
                    anchors.right: parent.right
                    y: 62
                    height: 4
                    radius: height / 2
                    color: CelestinaTheme.mediaProgressTrack
                    Accessible.role: Accessible.ProgressBar
                    Accessible.name: "Progreso de la reproducción"
                    Accessible.description: root.hasProgress
                            ? Math.round(root.progress * 100) + " por ciento"
                            : "Progreso no disponible"

                    Rectangle {
                        width: parent.width * root.progress
                        height: parent.height
                        radius: parent.radius
                        color: CelestinaTheme.mediaProgress
                        Accessible.ignored: true
                    }
                }

                Row {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.top: progressTrack.bottom
                    anchors.topMargin: 14
                    spacing: CelestinaTheme.spaceSm

                    CelestinaIconButton {
                        density: CelestinaButton.Regular
                        iconName: "media-skip-backward"
                        fallbackIcon: "media-skip-back"
                        Accessible.name: "Anterior"
                        enabled: root.connected && root.hasMedia
                                 && root.canPrevious
                        onClicked: root.controller.controlPhoneMedia(
                                       root.phoneIndex, "Previous")
                    }

                    CelestinaIconButton {
                        density: CelestinaButton.Regular
                        role: CelestinaButton.Primary
                        iconName: root.playing ? "media-playback-pause"
                                               : "media-playback-start"
                        fallbackIcon: root.playing ? "media-pause" : "media-play"
                        Accessible.name: root.playing ? "Pausar" : "Reproducir"
                        enabled: root.connected && root.hasMedia
                                 && (!root.playing || root.canPause)
                        onClicked: root.controller.controlPhoneMedia(
                                       root.phoneIndex, "PlayPause")
                    }

                    CelestinaIconButton {
                        density: CelestinaButton.Regular
                        iconName: "media-skip-forward"
                        fallbackIcon: "media-skip-forward"
                        Accessible.name: "Siguiente"
                        enabled: root.connected && root.hasMedia && root.canNext
                        onClicked: root.controller.controlPhoneMedia(
                                       root.phoneIndex, "Next")
                    }
                }
            }
        }

        CelestinaButton {
            anchors.left: mediaCard.left
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 18
            text: "Sonar"
            Accessible.name: "Hacer sonar el móvil para encontrarlo"
            enabled: root.connected
            onClicked: root.controller.ringPhone(root.phoneIndex)
        }

        CelestinaButton {
            anchors.right: mediaCard.right
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 18
            text: "Cerrar"
            role: CelestinaButton.Primary
            onClicked: root.closeDialog()
        }
    }
}
