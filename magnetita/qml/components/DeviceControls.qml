import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

Item {
    id: root

    required property var devices
    required property int primaryIndex
    required property int mediaIndex
    required property int mediaControlIndex

    readonly property bool paired: primaryIndex >= 0
                                   && primaryIndex < devices.devicePaired.length
                                   && devices.devicePaired[primaryIndex] === "true"
    readonly property bool mounted: primaryIndex >= 0
                                    && primaryIndex < devices.deviceMounts.length
                                    && devices.deviceMounts[primaryIndex].length > 0
    readonly property bool playing: mediaIndex >= 0
                                    && devices.deviceMediaPlaying[mediaIndex] === "true"
    readonly property string mediaLine: mediaIndex >= 0
                                        ? devices.deviceMedia[mediaIndex] : ""
    readonly property int mediaSeparator: mediaLine.indexOf(" — ")
    readonly property string mediaArtist: mediaSeparator >= 0
                                          ? mediaLine.substring(0, mediaSeparator)
                                          : "Magnetita"
    readonly property string mediaTitle: mediaLine.length === 0
                                         ? "Nada reproduciéndose"
                                         : mediaSeparator >= 0
                                           ? mediaLine.substring(mediaSeparator + 3)
                                           : mediaLine

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

    CelestinaSurface {
        id: mediaCard
        anchors.top: actionRow.bottom
        anchors.topMargin: 10
        width: parent.width
        height: 146
        role: CelestinaSurface.Elevated
        clip: true

        Rectangle {
            anchors.fill: parent
            radius: mediaCard.radius
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: CelestinaTheme.mediaSurfaceStart }
                GradientStop { position: 0.48; color: CelestinaTheme.mediaSurfaceMid }
                GradientStop { position: 1; color: CelestinaTheme.mediaSurfaceEnd }
            }
        }

        Rectangle {
            id: artwork
            x: 15
            anchors.verticalCenter: parent.verticalCenter
            width: 92
            height: 116
            radius: CelestinaTheme.radiusButton
            gradient: Gradient {
                orientation: Gradient.Vertical
                GradientStop { position: 0; color: CelestinaTheme.mediaArtworkStart }
                GradientStop { position: 0.55; color: CelestinaTheme.mediaArtworkMid }
                GradientStop { position: 1; color: CelestinaTheme.mediaArtworkEnd }
            }

            CelestinaIcon {
                anchors.centerIn: parent
                width: CelestinaTheme.glyphTile
                height: width
                name: "audio-x-generic"
                fallbackName: "music"
                tone: CelestinaIcon.Overlay
            }
        }

        Column {
            anchors.left: artwork.right
            anchors.leftMargin: 15
            anchors.right: parent.right
            anchors.rightMargin: 15
            anchors.top: parent.top
            anchors.topMargin: 19
            spacing: 4

            CelestinaSectionLabel {
                text: root.mediaLine.length > 0 ? "AHORA SUENA" : "CONTROL MULTIMEDIA"
            }

            Text {
                width: parent.width
                text: root.mediaTitle
                color: CelestinaTheme.mediaSurfaceInk
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: root.mediaLine.length > 0
                      ? root.mediaArtist : "Controla el audio del dispositivo"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
            }
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 14
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 13
            spacing: 5

            CelestinaIconButton {
                density: CelestinaButton.Regular
                iconName: "media-skip-backward"
                fallbackIcon: "media-skip-back"
                helpText: "Anterior"
                onClicked: root.devices.mediaPrevious(root.mediaControlIndex)
            }

            CelestinaIconButton {
                density: CelestinaButton.Regular
                role: CelestinaButton.Primary
                iconName: root.playing ? "media-playback-pause"
                                       : "media-playback-start"
                fallbackIcon: root.playing ? "media-pause" : "media-play"
                helpText: root.playing ? "Pausar" : "Reproducir"
                onClicked: root.devices.mediaPlayPause(root.mediaControlIndex)
            }

            CelestinaIconButton {
                density: CelestinaButton.Regular
                iconName: "media-skip-forward"
                fallbackIcon: "media-skip-forward"
                helpText: "Siguiente"
                onClicked: root.devices.mediaNext(root.mediaControlIndex)
            }
        }
    }
}
