import QtQuick
import org.celestina.magnetita 1.0

Column {
    id: root

    required property var devices
    required property int mediaIndex
    required property int primaryIndex
    required property int mediaControlIndex
    spacing: 10

    Text {
        width: parent.width
        visible: root.devices.deviceNames.length === 0
        text: "Abre KDE Connect en tu móvil y empareja.\nEl servicio (magnetitad) mantiene la conexión."
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
        wrapMode: Text.WordWrap
        lineHeight: 1.3
        bottomPadding: CelestinaTheme.spaceMd
    }

    Column {
        id: deviceBlock
        visible: root.devices.deviceNames.length > 0
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
                fingerprint: index < root.devices.deviceFingerprints.length
                             ? root.devices.deviceFingerprints[index] : ""
                batteryText: index < root.devices.deviceBattery.length
                             ? root.devices.deviceBattery[index] : ""
                charging: index < root.devices.deviceCharging.length
                          && root.devices.deviceCharging[index] === "true"
            }
        }
    }

    DeviceControls {
        id: controlsBlock
        visible: root.primaryIndex >= 0
        width: parent.width
        devices: root.devices
        primaryIndex: root.primaryIndex
        mediaIndex: root.mediaIndex
        mediaControlIndex: root.mediaControlIndex
    }

    ActivityLog {
        width: parent.width
        // Fill what remains below the preceding blocks. Basing this on the
        // actual laid-out y removes the old window-height magic constant.
        height: Math.max(146, root.height - y)
        devices: root.devices
    }
}
