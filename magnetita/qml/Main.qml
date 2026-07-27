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

    // The primary device (the first connected) whose controls the second block
    // drives; and the player the transport targets — the one playing, else the
    // primary (so ▶ can wake the phone's last player).
    readonly property int primaryIndex: devices.deviceNames.length > 0 ? 0 : -1
    readonly property int mediaControlIndex: mediaIndex >= 0 ? mediaIndex : primaryIndex

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
                font.pixelSize: CelestinaTheme.fontHeaderCollapsed
                font.weight: CelestinaTheme.weightDemiBold
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
            font.pixelSize: CelestinaTheme.fontRowTitle
            wrapMode: Text.WordWrap
            lineHeight: 1.3
            bottomPadding: 8
        }

        // ── Bloque 1: dispositivo(s) — solo información, sin scroll ───
        Column {
            id: deviceBlock
            visible: !window.settingsOpen && devices.deviceNames.length > 0
            width: parent.width
            spacing: 8

            Repeater {
                model: devices.deviceNames

                delegate: Rectangle {
                    id: devRow
                    required property int index
                    required property string modelData
                    readonly property string state:
                        index < devices.deviceStates.length ? devices.deviceStates[index] : ""
                    readonly property string mount:
                        index < devices.deviceMounts.length ? devices.deviceMounts[index] : ""
                    readonly property string fingerprint:
                        index < devices.deviceFingerprints.length ? devices.deviceFingerprints[index] : ""
                    readonly property bool mounted: mount.length > 0
                    readonly property string battery:
                        index < devices.deviceBattery.length ? devices.deviceBattery[index] : ""

                    width: deviceBlock.width
                    height: 96
                    radius: CelestinaTheme.radiusMd
                    color: CelestinaTheme.surface
                    border.color: CelestinaTheme.divider
                    border.width: 1

                    Column {
                        anchors.left: parent.left
                        anchors.leftMargin: 18
                        anchors.right: parent.right
                        anchors.rightMargin: 18
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 3

                        Text {
                            width: parent.width
                            text: devRow.modelData
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontRowTitle
                            font.weight: CelestinaTheme.weightDemiBold
                            elide: Text.ElideRight
                        }
                        Text {
                            width: parent.width
                            text: devRow.mounted ? devRow.state + " · " + devRow.mount : devRow.state
                            color: devRow.mounted ? CelestinaTheme.accent : CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                            elide: Text.ElideMiddle
                        }
                        Text {
                            visible: devRow.battery.length > 0
                            text: devRow.battery
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                        }
                        Text {
                            width: parent.width
                            visible: devRow.fingerprint.length > 0
                            text: "🔑 " + devRow.fingerprint
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.monoFamily
                            font.pixelSize: CelestinaTheme.fontMini
                            elide: Text.ElideRight
                        }
                    }
                }
            }
        }

        // ── Bloque 2: controles + medios del dispositivo principal ────
        Rectangle {
            id: controlsBlock
            visible: !window.settingsOpen && window.primaryIndex >= 0
            width: parent.width
            height: controlsColumn.implicitHeight + 26
            radius: CelestinaTheme.radiusMd
            color: CelestinaTheme.surface
            border.color: CelestinaTheme.divider
            border.width: 1

            Column {
                id: controlsColumn
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.topMargin: 13
                anchors.leftMargin: 14
                anchors.rightMargin: 14
                spacing: 10

                // Acciones del dispositivo principal.
                Row {
                    id: actionRow
                    spacing: 8
                    readonly property bool paired:
                        window.primaryIndex >= 0
                        && window.primaryIndex < devices.devicePaired.length
                        && devices.devicePaired[window.primaryIndex] === "true"
                    readonly property bool mounted:
                        window.primaryIndex >= 0
                        && window.primaryIndex < devices.deviceMounts.length
                        && devices.deviceMounts[window.primaryIndex].length > 0

                    CelestinaButton {
                        width: 116
                        visible: actionRow.mounted
                        primary: true
                        text: "Abrir"
                        onClicked: devices.openMount(window.primaryIndex)
                    }
                    CelestinaButton {
                        width: 116
                        visible: !actionRow.paired
                        primary: true
                        text: "Emparejar"
                        onClicked: devices.pairDevice(window.primaryIndex)
                    }
                    CelestinaButton {
                        width: 116
                        visible: actionRow.paired
                        text: "Sonar"
                        onClicked: devices.ringDevice(window.primaryIndex)
                    }
                    CelestinaButton {
                        width: 116
                        visible: actionRow.paired
                        text: "Desvincular"
                        onClicked: devices.unpairDevice(window.primaryIndex)
                    }
                }

                Rectangle {
                    width: parent.width
                    height: 1
                    color: CelestinaTheme.divider
                }

                // Medios — siempre visible; "Nada reproduciéndose" cuando no suena.
                Item {
                    id: mediaLine
                    width: parent.width
                    height: 34
                    readonly property bool playing:
                        window.mediaIndex >= 0
                        && devices.deviceMediaPlaying[window.mediaIndex] === "true"

                    Text {
                        anchors.left: parent.left
                        anchors.right: mediaRow.left
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        text: window.mediaIndex >= 0
                              ? "♪ " + devices.deviceMedia[window.mediaIndex]
                              : "♪ Nada reproduciéndose"
                        color: window.mediaIndex >= 0
                               ? CelestinaTheme.text : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowTitle
                        font.weight: window.mediaIndex >= 0
                                     ? CelestinaTheme.weightDemiBold : Font.Normal
                        elide: Text.ElideRight
                    }

                    Row {
                        id: mediaRow
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6

                        CelestinaButton {
                            width: 44
                            text: "⏮"
                            onClicked: devices.mediaPrevious(window.mediaControlIndex)
                        }
                        CelestinaButton {
                            width: 44
                            primary: mediaLine.playing
                            text: mediaLine.playing ? "⏸" : "▶"
                            onClicked: devices.mediaPlayPause(window.mediaControlIndex)
                        }
                        CelestinaButton {
                            width: 44
                            text: "⏭"
                            onClicked: devices.mediaNext(window.mediaControlIndex)
                        }
                    }
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
            // Fill the rest: the log is the only scrollable block.
            height: Math.max(120, window.height - deviceBlock.height
                    - controlsBlock.height - 196)
            radius: CelestinaTheme.radiusMd
            color: CelestinaTheme.card
            border.color: CelestinaTheme.divider
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
                    border.color: CelestinaTheme.divider
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
                            font.pixelSize: CelestinaTheme.fontRowTitle
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
                    border.color: CelestinaTheme.divider
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
                        font.pixelSize: CelestinaTheme.fontRowTitle
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
