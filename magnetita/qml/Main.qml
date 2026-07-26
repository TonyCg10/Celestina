import QtQuick
import QtQuick.Window
import QtQuick.Controls
import org.celestina.magnetita

ApplicationWindow {
    id: window
    visible: true
    width: 440
    height: 600
    minimumWidth: 360
    minimumHeight: 420
    title: "Magnetita"
    color: CelestinaTheme.canvas

    DevicesModel {
        id: devices
        Component.onCompleted: reload()
    }

    Column {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 6

        Text {
            text: "Magnetita"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontTitle
            font.weight: CelestinaTheme.weightBold
        }

        Text {
            text: devices.deviceNames.length > 0
                  ? devices.deviceNames.length + (devices.deviceNames.length === 1
                        ? " dispositivo conectado" : " dispositivos conectados")
                  : "Ningún dispositivo conectado"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            bottomPadding: 14
        }

        // Empty state.
        Text {
            width: parent.width
            visible: devices.deviceNames.length === 0
            text: "Abre KDE Connect en tu móvil y empareja.\nEl servicio (magnetitad) mantiene la conexión."
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCallout
            wrapMode: Text.WordWrap
            lineHeight: 1.3
        }

        ListView {
            id: list
            width: parent.width
            height: window.height - 150
            spacing: 10
            clip: true
            model: devices.deviceNames

            delegate: Rectangle {
                id: row
                required property int index
                required property string modelData
                readonly property string state:
                    index < devices.deviceStates.length ? devices.deviceStates[index] : ""
                readonly property string mount:
                    index < devices.deviceMounts.length ? devices.deviceMounts[index] : ""
                readonly property bool mounted: mount.length > 0

                width: list.width
                height: 66
                radius: CelestinaTheme.radiusMd
                color: CelestinaTheme.surface
                border.color: CelestinaTheme.border
                border.width: 1

                Column {
                    anchors.left: parent.left
                    anchors.leftMargin: 18
                    anchors.right: openButton.left
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 3

                    Text {
                        text: row.modelData
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCallout
                        font.weight: CelestinaTheme.weightDemiBold
                        elide: Text.ElideRight
                        width: parent.width
                    }
                    Text {
                        text: row.mounted ? row.state + " · " + row.mount : row.state
                        color: row.mounted ? CelestinaTheme.accent : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        elide: Text.ElideMiddle
                        width: parent.width
                    }
                }

                Rectangle {
                    id: openButton
                    anchors.right: parent.right
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    visible: row.mounted
                    width: openLabel.implicitWidth + 26
                    height: 34
                    radius: CelestinaTheme.radiusSm
                    color: openMouse.containsMouse ? CelestinaTheme.accentStrong
                                                   : CelestinaTheme.accent

                    Text {
                        id: openLabel
                        anchors.centerIn: parent
                        text: "Abrir"
                        color: "white"
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontLabel
                        font.weight: CelestinaTheme.weightDemiBold
                    }

                    MouseArea {
                        id: openMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: devices.openMount(row.index)
                    }
                }
            }
        }
    }
}
