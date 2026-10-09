// The delegates below reach the page's `root` id, which a delegate may only
// do under bound component behaviour; each one already declares the
// `index`/`modelData` it takes from the model.
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

ScrollPage {
    id: root

    required property DevicesModel devices
    required property int mediaIndex
    required property int primaryIndex
    required property int mediaControlIndex
    required property Item backdrop

    spacing: 10

    // Files dragged from Siderita onto a device row or this page go to that
    // device. The notice is `--send`'s chooser's own wording: sending, then a
    // failure if there was one; what was not a local file is only reported.
    property bool dropSending: false
    property string dropNotice: ""

    // The dropped URLs as plain strings, as Siderita's `droppedUris`; a loop
    // inside a drop handler made qmllint stop reporting the file.
    function droppedUris(urls) {
        const out = []
        for (let i = 0; i < urls.length; i++)
            out.push(urls[i].toString())
        return out
    }

    function sendDrop(index, drop) {
        if (!drop.hasUrls || index < 0)
            return
        // One drop at a time: a fast second send would otherwise overwrite
        // the first one's outcome before it was read.
        if (root.dropSending) {
            root.dropNotice = qsTr("Hay un envío en curso")
            dropNoticeTimer.restart()
            return
        }
        // Not decoded here: Rust reads each URI by bytes.
        const uris = root.droppedUris(drop.urls)
        root.dropSending = true
        root.dropNotice = ""
        dropNoticeTimer.stop()
        root.devices.sendDropped(index, uris)
        drop.accept(Qt.CopyAction)
    }

    Connections {
        target: root.devices

        function onDropSendFinished(error, ignored) {
            root.dropSending = false
            const parts = []
            if (error !== "")
                parts.push(qsTr("No se pudo enviar: %1").arg(error))
            if (ignored > 0)
                parts.push(qsTr("Solo se envían archivos locales: %n elemento(s) ignorado(s)",
                                "", ignored))
            root.dropNotice = parts.join("\n")
            dropNoticeTimer.restart()
        }
    }

    Timer {
        id: dropNoticeTimer
        interval: 4000
        onTriggered: root.dropNotice = ""
    }

    Text {
        width: parent.width
        visible: root.dropSending || root.dropNotice.length > 0
        // The daemon's message is development text; the line before it is
        // what the person reads.
        textFormat: Text.PlainText
        text: root.dropNotice.length > 0 ? root.dropNotice : qsTr("Enviando…")
        color: root.dropNotice.length > 0 ? CelestinaTheme.danger : CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        wrapMode: Text.Wrap
    }

    // Pairing over the own wire starts here, with or without a phone in the
    // list: the QR is the way in, and the same action re-pairs a phone that
    // forgot this desktop.
    RowLayout {
        width: parent.width
        visible: root.devices.devicesAvailable && !root.devices.pairingActive
        spacing: CelestinaTheme.spaceSm

        Text {
            Layout.fillWidth: true
            visible: root.devices.deviceNames.length === 0
            text: qsTr("Sin ningún teléfono. Vincula uno con su código.")
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            wrapMode: Text.WordWrap
        }

        Item { Layout.fillWidth: true; visible: root.devices.deviceNames.length > 0 }

        CelestinaIconButton {
            iconName: "link"
            density: CelestinaButton.Prominent
            role: CelestinaButton.Primary
            helpText: qsTr("Vincular un teléfono")
            onClicked: root.devices.startPairing()
        }
    }

    PairingSheet {
        width: parent.width
        visible: root.devices.pairingActive
        devices: root.devices
        onDismissRequested: root.devices.dismissPairing()
    }

    Text {
        width: parent.width
        visible: !root.devices.devicesAvailable
        text: "El servicio Magnetita no está disponible."
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
        wrapMode: Text.WordWrap
        lineHeight: 1.3
        bottomPadding: CelestinaTheme.spaceMd
    }

    // The phone's call, while there is one: who, and the three things the
    // desktop may do about it, as glyphs.
    CelestinaSurface {
        readonly property string callState: root.primaryIndex >= 0 && root.primaryIndex < root.devices.deviceCallStates.length
                                            ? root.devices.deviceCallStates[root.primaryIndex] : ""
        readonly property string callName: root.primaryIndex >= 0 && root.primaryIndex < root.devices.deviceCallNames.length
                                           ? root.devices.deviceCallNames[root.primaryIndex] : ""
        id: callBanner
        width: parent.width
        visible: callState.length > 0
        height: visible ? 64 : 0
        role: CelestinaSurface.Tonal

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: CelestinaTheme.spaceMd
            anchors.rightMargin: CelestinaTheme.spaceSm
            spacing: CelestinaTheme.spaceSm

            CelestinaIcon {
                Layout.preferredWidth: CelestinaTheme.iconMd
                Layout.preferredHeight: CelestinaTheme.iconMd
                name: "phone"
                fallbackName: "phone"
                tone: CelestinaIcon.Device
            }

            Text {
                Layout.fillWidth: true
                // Peer-supplied text: never interpreted as markup.
                textFormat: Text.PlainText
                text: (callBanner.callState === "ringing" ? qsTr("Llamada entrante: ")
                       : callBanner.callState === "answered" ? qsTr("En llamada: ")
                       : qsTr("Llamada perdida: ")) + callBanner.callName
                color: callBanner.ink
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideRight
            }

            CelestinaIconButton {
                iconName: "bell-off"
                visible: callBanner.callState === "ringing"
                helpText: qsTr("Silenciar")
                onClicked: root.devices.callAction(root.primaryIndex, "Mute")
            }

            CelestinaIconButton {
                iconName: "phone"
                visible: callBanner.callState === "ringing"
                role: CelestinaButton.Primary
                helpText: qsTr("Responder")
                onClicked: root.devices.callAction(root.primaryIndex, "Answer")
            }

            CelestinaIconButton {
                iconName: "x"
                visible: callBanner.callState === "ringing" || callBanner.callState === "answered"
                helpText: qsTr("Colgar")
                onClicked: root.devices.callAction(root.primaryIndex, "HangUp")
            }
        }
    }

    Column {
        id: deviceBlock
        visible: root.devices.devicesAvailable
                 && root.devices.deviceNames.length > 0
        width: parent.width
        spacing: 10

        Repeater {
            model: root.devices.deviceNames

            delegate: ConnectedDeviceCard {
                id: deviceCard
                required property int index
                required property string modelData

                width: deviceBlock.width
                deviceName: modelData
                deviceType: index < root.devices.deviceTypes.length
                            ? root.devices.deviceTypes[index] : ""
                stateText: index < root.devices.deviceStates.length
                           ? root.devices.deviceStates[index] : ""
                mountPath: index < root.devices.deviceMounts.length
                           ? root.devices.deviceMounts[index] : ""
                verificationKey: index < root.devices.deviceVerificationKeys.length
                                 ? root.devices.deviceVerificationKeys[index] : ""
                batteryText: index < root.devices.deviceBattery.length
                             ? root.devices.deviceBattery[index] : ""
                charging: index < root.devices.deviceCharging.length
                          && root.devices.deviceCharging[index] === "true"
                onOpenMountRequested: root.devices.openMount(index)

                // A drop on this row sends to this device.
                DropArea {
                    id: cardDrop
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onEntered: function(drag) {
                        if (!drag.hasUrls)
                            drag.accepted = false
                    }
                    onDropped: function(drop) {
                        root.sendDrop(deviceCard.index, drop)
                    }

                    // Siderita's drop highlight: an accent rim, nothing filled.
                    Rectangle {
                        anchors.fill: parent
                        visible: cardDrop.containsDrag
                        color: CelestinaTheme.clear
                        border.width: CelestinaTheme.borderFocus
                        border.color: CelestinaTheme.accent
                        radius: deviceCard.radius
                    }
                }
            }
        }
    }

    DeviceControls {
        id: controlsBlock
        visible: root.devices.devicesAvailable && root.primaryIndex >= 0
        width: parent.width
        devices: root.devices
        backdrop: root.backdrop
        primaryIndex: root.primaryIndex
        mediaIndex: root.mediaIndex
        mediaControlIndex: root.mediaControlIndex
    }

    ActivityLog {
        width: parent.width
        // Fill what remains below the preceding blocks. When content grows,
        // the Flickable exposes the whole page instead of clipping controls.
        height: Math.max(146, root.height - y)
        devices: root.devices
    }
}
