import QtQuick
import QtQuick.Window
import QtQuick.Controls
import org.celestina.magnetita

ApplicationWindow {
    id: window
    visible: true
    width: 460
    height: 680
    minimumWidth: 380
    minimumHeight: 480
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
            bottomPadding: 12
        }

        Text {
            width: parent.width
            visible: devices.deviceNames.length === 0
            text: "Abre KDE Connect en tu móvil y empareja.\nEl servicio (magnetitad) mantiene la conexión."
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCallout
            wrapMode: Text.WordWrap
            lineHeight: 1.3
            bottomPadding: 8
        }

        ListView {
            id: deviceList
            width: parent.width
            height: Math.min(contentHeight, window.height * 0.42)
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
                readonly property string fingerprint:
                    index < devices.deviceFingerprints.length ? devices.deviceFingerprints[index] : ""
                readonly property bool mounted: mount.length > 0
                readonly property bool paired:
                    index < devices.devicePaired.length && devices.devicePaired[index] === "true"
                readonly property string battery:
                    index < devices.deviceBattery.length ? devices.deviceBattery[index] : ""

                width: deviceList.width
                height: 106
                radius: CelestinaTheme.radiusMd
                color: CelestinaTheme.surface
                border.color: CelestinaTheme.border
                border.width: 1

                Column {
                    anchors.left: parent.left
                    anchors.leftMargin: 18
                    anchors.right: actions.left
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
                    Text {
                        visible: row.battery.length > 0
                        text: row.battery
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                    }
                    Text {
                        visible: row.fingerprint.length > 0
                        text: "🔑 " + row.fingerprint
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.monoFamily
                        font.pixelSize: CelestinaTheme.fontMini
                        elide: Text.ElideRight
                        width: parent.width
                    }
                }

                Column {
                    id: actions
                    anchors.right: parent.right
                    anchors.rightMargin: 14
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6

                    CelestinaButton {
                        width: 118
                        visible: row.mounted
                        primary: true
                        text: "Abrir"
                        onClicked: devices.openMount(row.index)
                    }
                    CelestinaButton {
                        width: 118
                        visible: !row.paired
                        primary: true
                        text: "Emparejar"
                        onClicked: devices.pairDevice(row.index)
                    }
                    CelestinaButton {
                        width: 118
                        visible: row.paired
                        text: "Sonar"
                        onClicked: devices.ringDevice(row.index)
                    }
                    CelestinaButton {
                        width: 118
                        visible: row.paired
                        text: "Desvincular"
                        onClicked: devices.unpairDevice(row.index)
                    }
                }
            }
        }

        // ── Connection log — "why won't it connect" ──────────────────
        Text {
            text: "ACTIVIDAD"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontMini
            font.letterSpacing: 1.4
            font.weight: CelestinaTheme.weightDemiBold
            topPadding: 14
            bottomPadding: 4
        }

        Rectangle {
            width: parent.width
            height: window.height - deviceList.height - 210
            radius: CelestinaTheme.radiusMd
            color: CelestinaTheme.canvasRaised
            border.color: CelestinaTheme.border
            border.width: 1

            ListView {
                id: logList
                anchors.fill: parent
                anchors.margins: 12
                spacing: 5
                clip: true
                model: devices.logLines

                delegate: Text {
                    required property int index
                    required property string modelData
                    readonly property bool failure:
                        index < devices.logFailures.length
                        && devices.logFailures[index] === "true"
                    width: logList.width
                    text: modelData
                    color: failure ? CelestinaTheme.danger : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    wrapMode: Text.WordWrap
                }
            }

            Text {
                anchors.centerIn: parent
                visible: devices.logLines.length === 0
                text: "Sin actividad todavía"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }
        }
    }
}
