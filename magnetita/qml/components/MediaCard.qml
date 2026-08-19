import QtQuick
import QtQuick.Effects
import org.celestina.magnetita 1.0

CelestinaSurface {
    id: root

    required property bool hasMedia
    required property string player
    required property string title
    required property string artist
    required property string album
    required property string artworkUrl
    required property real positionMs
    required property real lengthMs
    required property bool playing
    required property bool canPlay
    required property bool canPause
    required property bool canPrevious
    required property bool canNext
    required property string progressKind

    signal previousRequested
    signal playPauseRequested
    signal nextRequested

    readonly property bool progressAvailable: progressKind === "finite"
    readonly property bool live: progressKind === "live"
    readonly property real progressValue: progressAvailable ? positionMs / lengthMs : 0
    readonly property bool artworkReady: backdropImage.status === Image.Ready
    readonly property color foregroundInk: artworkReady
                                                   ? CelestinaTheme.mediaScrimInk
                                                   : CelestinaTheme.mediaSurfaceInk
    readonly property string secondaryLine: artist.length > 0 && album.length > 0
                                                    ? artist + " · " + album
                                                  : artist.length > 0 ? artist
                                                  : album.length > 0 ? album
                                                  : player.length > 0 ? player
                                                  : "Magnetita"

    function formatTime(milliseconds) {
        if (milliseconds < 0)
            return "--:--"
        const total = Math.floor(milliseconds / 1000)
        const seconds = total % 60
        const minutes = Math.floor(total / 60) % 60
        const hours = Math.floor(total / 3600)
        const paddedSeconds = seconds < 10 ? "0" + seconds : seconds.toString()
        if (hours > 0) {
            const paddedMinutes = minutes < 10 ? "0" + minutes : minutes.toString()
            return hours + ":" + paddedMinutes + ":" + paddedSeconds
        }
        return minutes + ":" + paddedSeconds
    }

    height: 192
    role: CelestinaSurface.Elevated
    background: null
    clip: true
    Accessible.role: Accessible.Pane
    Accessible.name: root.hasMedia ? root.title : "Control multimedia"
    Accessible.description: root.hasMedia
                              ? (root.live
                                 ? root.secondaryLine + ", emisión en directo"
                                 : root.secondaryLine)
                                          : "No hay contenido reproduciéndose"

    Rectangle {
        z: -3
        anchors.fill: parent
        radius: root.radius
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: CelestinaTheme.mediaSurfaceStart }
            GradientStop { position: 0.48; color: CelestinaTheme.mediaSurfaceMid }
            GradientStop { position: 1; color: CelestinaTheme.mediaSurfaceEnd }
        }
        Accessible.ignored: true
    }

    Rectangle {
        id: roundedCardMask

        anchors.fill: parent
        radius: root.radius
        visible: false
        layer.enabled: true
        Accessible.ignored: true
    }

    Image {
        id: backdropImage

        z: -2
        anchors.fill: parent
        source: root.artworkUrl
        sourceSize: Qt.size(640, 360)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: true
        visible: root.artworkUrl.length > 0
        Accessible.ignored: true
        layer.enabled: visible
        layer.effect: MultiEffect {
            autoPaddingEnabled: false
            blurEnabled: true
            blur: CelestinaTheme.glassBlur
            blurMax: CelestinaTheme.glassBlurMax
            blurMultiplier: CelestinaTheme.glassBlurMultiplier
            saturation: CelestinaTheme.glassSaturation
            maskEnabled: true
            maskSource: roundedCardMask
        }
    }

    Rectangle {
        z: -1
        anchors.fill: parent
        radius: root.radius
        color: CelestinaTheme.mediaScrim
        visible: root.artworkReady
        Accessible.ignored: true
    }

    Image {
        id: foregroundArtwork

        x: 16
        y: 17
        width: 112
        height: 63
        source: root.artworkUrl
        sourceSize: Qt.size(448, 252)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: true
        visible: status === Image.Ready
        Accessible.ignored: true
    }

    Column {
        id: metadata

        anchors.left: foregroundArtwork.visible
                      ? foregroundArtwork.right : parent.left
        anchors.leftMargin: 16
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.top: parent.top
        anchors.topMargin: 18
        spacing: 4

        CelestinaSectionLabel {
            text: root.hasMedia ? "AHORA SUENA" : "CONTROL MULTIMEDIA"
            color: root.foregroundInk
        }

        Text {
            // Peer-supplied text: never interpreted as markup.
            textFormat: Text.PlainText
            width: parent.width
            text: root.hasMedia && root.title.length > 0
                  ? root.title : "Nada reproduciéndose"
            color: root.foregroundInk
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideRight
        }

        Text {
            // Peer-supplied text: never interpreted as markup.
            textFormat: Text.PlainText
            width: parent.width
            text: root.hasMedia ? root.secondaryLine
                                : "Controla el audio del dispositivo"
            color: root.foregroundInk
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }
    }

    MediaProgress {
        id: progress

        z: 2
        anchors.left: metadata.left
        anchors.right: metadata.right
        y: 79
        height: 24
        value: root.progressValue
        visible: root.progressAvailable
        accessibleDescription: root.formatTime(root.positionMs)
                               + " de " + root.formatTime(root.lengthMs)
    }

    CelestinaSectionLabel {
        anchors.left: metadata.left
        y: 84
        text: "EN DIRECTO"
        color: root.foregroundInk
        visible: root.live
        Accessible.role: Accessible.StaticText
        Accessible.name: "Emisión en directo"
    }

    Text {
        anchors.left: progress.left
        anchors.top: progress.bottom
        text: root.formatTime(root.positionMs)
        color: root.foregroundInk
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontMini
        visible: root.progressAvailable
    }

    Text {
        anchors.right: progress.right
        anchors.top: progress.bottom
        text: root.formatTime(root.lengthMs)
        color: root.foregroundInk
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontMini
        visible: root.progressAvailable
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 13
        spacing: 5

        QuietIconButton {
            density: CelestinaButton.Regular
            iconName: "media-skip-backward"
            fallbackIcon: "media-skip-back"
            helpText: "Anterior"
            enabled: root.hasMedia && root.canPrevious
            onClicked: root.previousRequested()
        }

        QuietIconButton {
            density: CelestinaButton.Regular
            role: CelestinaButton.Primary
            iconName: root.playing ? "media-playback-pause"
                                   : "media-playback-start"
            fallbackIcon: root.playing ? "media-pause" : "media-play"
            helpText: root.playing ? "Pausar" : "Reproducir"
            enabled: root.hasMedia
                     && (root.playing ? root.canPause : root.canPlay)
            onClicked: root.playPauseRequested()
        }

        QuietIconButton {
            density: CelestinaButton.Regular
            iconName: "media-skip-forward"
            fallbackIcon: "media-skip-forward"
            helpText: "Siguiente"
            enabled: root.hasMedia && root.canNext
            onClicked: root.nextRequested()
        }
    }
}
