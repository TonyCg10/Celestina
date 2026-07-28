import QtQuick
import org.celestina.magnetita 1.0

CelestinaSurface {
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

    height: controlsColumn.implicitHeight + 26
    role: CelestinaSurface.Tonal

    Column {
        id: controlsColumn
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.topMargin: 13
        anchors.leftMargin: 14
        anchors.rightMargin: 14
        spacing: 10

        Row {
            spacing: 8

            CelestinaButton {
                width: 116
                visible: root.mounted
                role: CelestinaButton.Primary
                text: "Abrir"
                onClicked: root.devices.openMount(root.primaryIndex)
            }

            CelestinaButton {
                width: 116
                visible: !root.paired
                role: CelestinaButton.Primary
                text: "Emparejar"
                onClicked: root.devices.pairDevice(root.primaryIndex)
            }

            CelestinaButton {
                width: 116
                visible: root.paired
                text: "Sonar"
                onClicked: root.devices.ringDevice(root.primaryIndex)
            }

            CelestinaButton {
                width: 116
                visible: root.paired
                text: "Desvincular"
                onClicked: root.devices.unpairDevice(root.primaryIndex)
            }
        }

        Rectangle {
            width: parent.width
            height: CelestinaTheme.borderHairline
            color: CelestinaTheme.divider
        }

        Item {
            width: parent.width
            height: 34

            CelestinaIcon {
                id: mediaIcon
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.iconSm
                height: width
                name: "audio-x-generic"
                fallbackName: "music"
                tone: root.mediaIndex >= 0
                      ? CelestinaIcon.Primary : CelestinaIcon.Secondary
            }

            Text {
                anchors.left: mediaIcon.right
                anchors.leftMargin: CelestinaTheme.spaceSm
                anchors.right: mediaRow.left
                anchors.rightMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                text: root.mediaIndex >= 0
                      ? root.devices.deviceMedia[root.mediaIndex]
                      : "Nada reproduciéndose"
                color: root.mediaIndex >= 0
                       ? CelestinaTheme.text : CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: root.mediaIndex >= 0
                             ? CelestinaTheme.weightDemiBold
                             : CelestinaTheme.weightRegular
                elide: Text.ElideRight
            }

            Row {
                id: mediaRow
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6

                CelestinaIconButton {
                    width: 44
                    iconName: "media-skip-backward"
                    fallbackIcon: "media-skip-back"
                    helpText: "Anterior"
                    onClicked: root.devices.mediaPrevious(root.mediaControlIndex)
                }

                CelestinaIconButton {
                    width: 44
                    role: root.playing ? CelestinaButton.Primary
                                       : CelestinaButton.Tonal
                    iconName: root.playing ? "media-playback-pause"
                                           : "media-playback-start"
                    fallbackIcon: root.playing ? "media-pause" : "media-play"
                    helpText: root.playing ? "Pausar" : "Reproducir"
                    onClicked: root.devices.mediaPlayPause(root.mediaControlIndex)
                }

                CelestinaIconButton {
                    width: 44
                    iconName: "media-skip-forward"
                    fallbackIcon: "media-skip-forward"
                    helpText: "Siguiente"
                    onClicked: root.devices.mediaNext(root.mediaControlIndex)
                }
            }
        }
    }
}
