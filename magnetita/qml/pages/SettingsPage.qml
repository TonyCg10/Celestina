import QtQuick
import org.celestina.magnetita 1.0

Column {
    id: root

    required property DevicesModel devices

    spacing: 16

    Text {
        width: parent.width
        visible: !root.devices.settingsAvailable
        text: "El servicio Magnetita no está disponible."
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        wrapMode: Text.Wrap
    }

    ListSection {
        width: parent.width
        visible: root.devices.settingsAvailable
        title: "DISPOSITIVOS EMPAREJADOS"

        Text {
            width: parent.width
            leftPadding: CelestinaTheme.spaceLg
            rightPadding: CelestinaTheme.spaceLg
            topPadding: CelestinaTheme.spaceSm
            bottomPadding: CelestinaTheme.spaceSm
            visible: root.devices.pairedNames.length === 0
            text: "Ningún dispositivo emparejado todavía."
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.Wrap
        }

        Repeater {
            model: root.devices.pairedNames

            delegate: PairedDeviceRow {
                required property int index
                required property string modelData

                width: root.width
                deviceName: modelData
                fingerprint: index < root.devices.pairedFingerprints.length
                             ? root.devices.pairedFingerprints[index] : ""
                online: index < root.devices.pairedConnected.length
                        && root.devices.pairedConnected[index] === "true"
                onForgetRequested: root.devices.forgetPaired(index)
            }
        }
    }

    ListSection {
        width: parent.width
        visible: root.devices.settingsAvailable
        title: "PLUGINS"

        Repeater {
            model: root.devices.pluginLabels

            delegate: PluginRow {
                required property int index
                required property string modelData

                width: root.width
                label: modelData
                enabledFlag: index < root.devices.pluginEnabled.length
                             && root.devices.pluginEnabled[index] === "true"
                onToggleRequested: root.devices.togglePlugin(index)
            }
        }
    }
}
