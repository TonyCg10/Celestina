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

    spacing: 10

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

    Column {
        id: deviceBlock
        visible: root.devices.devicesAvailable
                 && root.devices.deviceNames.length > 0
        width: parent.width
        spacing: 10

        Repeater {
            model: root.devices.deviceNames

            delegate: ConnectedDeviceCard {
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
            }
        }
    }

    DeviceControls {
        id: controlsBlock
        visible: root.devices.devicesAvailable && root.primaryIndex >= 0
        width: parent.width
        devices: root.devices
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
