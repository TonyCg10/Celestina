import QtQuick
import QtQuick.Window
import org.celestina.magnetita 1.0

Flickable {
    id: root

    required property DevicesModel devices
    required property int mediaIndex
    required property int primaryIndex
    required property int mediaControlIndex
    contentWidth: width
    contentHeight: content.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick

    function ensureFocusVisible(item) {
        if (!item)
            return
        let ancestor = item
        while (ancestor && ancestor !== content)
            ancestor = ancestor.parent
        if (ancestor !== content)
            return

        const point = item.mapToItem(content, 0, 0)
        const top = point.y
        const bottom = top + item.height
        if (top < contentY)
            contentY = Math.max(0, top - CelestinaTheme.spaceSm)
        else if (bottom > contentY + height)
            contentY = Math.min(Math.max(0, contentHeight - height),
                                bottom - height + CelestinaTheme.spaceSm)
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_PageDown) {
            contentY = Math.min(Math.max(0, contentHeight - height),
                                contentY + height * 0.8)
        } else if (event.key === Qt.Key_PageUp) {
            contentY = Math.max(0, contentY - height * 0.8)
        } else if (event.key === Qt.Key_Home) {
            contentY = 0
        } else if (event.key === Qt.Key_End) {
            contentY = Math.max(0, contentHeight - height)
        } else {
            return
        }
        event.accepted = true
    }

    Connections {
        target: root.Window.window

        function onActiveFocusItemChanged() {
            const item = root.Window.window
                         ? root.Window.window.activeFocusItem : null
            Qt.callLater(function() { root.ensureFocusVisible(item) })
        }
    }

    Column {
        id: content

        width: root.width
        spacing: 10

        Text {
            width: parent.width
            visible: !root.devices.devicesAvailable
                     || root.devices.deviceNames.length === 0
            text: root.devices.devicesAvailable
                  ? "Abre KDE Connect en tu móvil y empareja.\nEl servicio (magnetitad) mantiene la conexión."
                  : "El servicio Magnetita no está disponible."
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
}
