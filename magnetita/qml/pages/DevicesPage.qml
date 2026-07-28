import QtQuick
import org.celestina.magnetita 1.0

Column {
    id: root

    required property var devices
    required property int mediaIndex
    required property int primaryIndex
    required property int mediaControlIndex
    spacing: 6

    Text {
        text: root.devices.deviceNames.length > 0
              ? root.devices.deviceNames.length + (root.devices.deviceNames.length === 1
                    ? " dispositivo conectado" : " dispositivos conectados")
              : "Ningún dispositivo conectado"
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        bottomPadding: 12
    }

    Text {
        width: parent.width
        visible: root.devices.deviceNames.length === 0
        text: "Abre KDE Connect en tu móvil y empareja.\nEl servicio (magnetitad) mantiene la conexión."
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
        wrapMode: Text.WordWrap
        lineHeight: 1.3
        bottomPadding: 8
    }

    Column {
        id: deviceBlock
        visible: root.devices.deviceNames.length > 0
        width: parent.width
        spacing: 8

        Repeater {
            model: root.devices.deviceNames

            delegate: ConnectedDeviceCard {
                required property int index
                required property string modelData

                width: deviceBlock.width
                deviceName: modelData
                stateText: index < root.devices.deviceStates.length
                           ? root.devices.deviceStates[index] : ""
                mountPath: index < root.devices.deviceMounts.length
                           ? root.devices.deviceMounts[index] : ""
                fingerprint: index < root.devices.deviceFingerprints.length
                             ? root.devices.deviceFingerprints[index] : ""
                batteryText: index < root.devices.deviceBattery.length
                             ? root.devices.deviceBattery[index] : ""
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

    ActivityLog {
        width: parent.width
        // Fill what remains below the preceding blocks. Basing this on the
        // actual laid-out y removes the old window-height magic constant.
        height: Math.max(120, root.height - y)
        devices: root.devices
    }
}
