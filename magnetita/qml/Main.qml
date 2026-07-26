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

    // The first connected device that is playing something, or -1 — the phone
    // whose media the "Ahora suena" card controls.
    readonly property int mediaIndex: {
        for (var i = 0; i < devices.deviceMedia.length; i++) {
            if (devices.deviceMedia[i].length > 0)
                return i
        }
        return -1
    }

    // Whether the Settings surface (paired devices + plugin toggles) is showing
    // instead of the device list.
    property bool settingsOpen: false

    Column {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 6

        // Header: the title and the gear that flips to the Settings surface.
        Item {
            width: parent.width
            height: 34

            Text {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: window.settingsOpen ? "Ajustes" : "Magnetita"
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightBold
            }

            CelestinaButton {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: 116
                text: window.settingsOpen ? "‹ Volver" : "⚙ Ajustes"
                onClicked: {
                    window.settingsOpen = !window.settingsOpen
                    if (window.settingsOpen)
                        devices.reloadSettings()
                }
            }
        }

        Text {
            visible: !window.settingsOpen
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
            visible: !window.settingsOpen && devices.deviceNames.length === 0
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
            visible: !window.settingsOpen
            width: parent.width
            height: visible ? Math.min(contentHeight, window.height * 0.42) : 0
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

        // ── Ahora suena — media control of the phone ─────────────────
        Rectangle {
            id: mediaCard
            width: parent.width
            visible: !window.settingsOpen && window.mediaIndex >= 0
            height: 72
            radius: CelestinaTheme.radiusMd
            color: CelestinaTheme.surface
            border.color: CelestinaTheme.border
            border.width: 1

            Column {
                anchors.left: parent.left
                anchors.leftMargin: 16
                anchors.right: transport.left
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                spacing: 3

                Text {
                    text: "♪ AHORA SUENA"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    font.letterSpacing: 1.4
                    font.weight: CelestinaTheme.weightDemiBold
                }
                Text {
                    width: parent.width
                    text: window.mediaIndex >= 0 ? devices.deviceMedia[window.mediaIndex] : ""
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCallout
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideRight
                }
            }

            Row {
                id: transport
                anchors.right: parent.right
                anchors.rightMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                spacing: 6

                CelestinaButton {
                    width: 46
                    visible: window.mediaIndex >= 0
                             && devices.deviceMediaPrevious[window.mediaIndex] === "true"
                    text: "⏮"
                    onClicked: devices.mediaPrevious(window.mediaIndex)
                }
                CelestinaButton {
                    width: 46
                    primary: true
                    text: (window.mediaIndex >= 0
                           && devices.deviceMediaPlaying[window.mediaIndex] === "true")
                          ? "⏸" : "▶"
                    onClicked: devices.mediaPlayPause(window.mediaIndex)
                }
                CelestinaButton {
                    width: 46
                    visible: window.mediaIndex >= 0
                             && devices.deviceMediaNext[window.mediaIndex] === "true"
                    text: "⏭"
                    onClicked: devices.mediaNext(window.mediaIndex)
                }
            }
        }

        // ── Connection log — "why won't it connect" ──────────────────
        Text {
            visible: !window.settingsOpen
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
            visible: !window.settingsOpen
            width: parent.width
            height: window.height - deviceList.height
                    - (window.mediaIndex >= 0 ? mediaCard.height + 6 : 0) - 210
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

        // ── Settings surface: paired devices + per-plugin toggles ────
        Column {
            id: settingsView
            visible: window.settingsOpen
            width: parent.width
            spacing: 10

            Text {
                text: "DISPOSITIVOS EMPAREJADOS"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                font.letterSpacing: 1.4
                font.weight: CelestinaTheme.weightDemiBold
                topPadding: 4
            }

            Text {
                width: parent.width
                visible: devices.pairedNames.length === 0
                text: "Ningún dispositivo emparejado todavía."
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }

            Repeater {
                model: devices.pairedNames
                delegate: Rectangle {
                    required property int index
                    required property string modelData
                    readonly property string fingerprint:
                        index < devices.pairedFingerprints.length ? devices.pairedFingerprints[index] : ""
                    readonly property bool online:
                        index < devices.pairedConnected.length && devices.pairedConnected[index] === "true"

                    width: settingsView.width
                    height: 74
                    radius: CelestinaTheme.radiusMd
                    color: CelestinaTheme.surface
                    border.color: CelestinaTheme.border
                    border.width: 1

                    Column {
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.right: forget.left
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 3

                        Text {
                            width: parent.width
                            text: (online ? "🟢 " : "⚪ ") + modelData
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCallout
                            font.weight: CelestinaTheme.weightDemiBold
                            elide: Text.ElideRight
                        }
                        Text {
                            width: parent.width
                            text: "🔑 " + fingerprint
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.monoFamily
                            font.pixelSize: CelestinaTheme.fontMini
                            elide: Text.ElideRight
                        }
                    }

                    CelestinaButton {
                        id: forget
                        anchors.right: parent.right
                        anchors.rightMargin: 14
                        anchors.verticalCenter: parent.verticalCenter
                        width: 100
                        text: "Olvidar"
                        onClicked: devices.forgetPaired(index)
                    }
                }
            }

            Text {
                text: "PLUGINS"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                font.letterSpacing: 1.4
                font.weight: CelestinaTheme.weightDemiBold
                topPadding: 10
            }

            Repeater {
                model: devices.pluginLabels
                delegate: Rectangle {
                    required property int index
                    required property string modelData
                    readonly property bool enabledFlag:
                        index < devices.pluginEnabled.length && devices.pluginEnabled[index] === "true"

                    width: settingsView.width
                    height: 48
                    radius: CelestinaTheme.radiusMd
                    color: CelestinaTheme.surface
                    border.color: CelestinaTheme.border
                    border.width: 1

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.right: toggle.left
                        anchors.rightMargin: 12
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCallout
                        elide: Text.ElideRight
                    }

                    CelestinaButton {
                        id: toggle
                        anchors.right: parent.right
                        anchors.rightMargin: 14
                        anchors.verticalCenter: parent.verticalCenter
                        width: 120
                        primary: enabledFlag
                        text: enabledFlag ? "Activado" : "Desactivado"
                        onClicked: devices.togglePlugin(index)
                    }
                }
            }
        }
    }
}
