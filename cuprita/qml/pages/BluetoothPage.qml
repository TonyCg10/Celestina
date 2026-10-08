pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"

// The Bluetooth section: the adapter switch and a search button, then the
// devices the adapter knows or sees.
CelestinaSurface {
    id: page

    // BluetoothController: powered, discovering and the commands.
    required property var controller
    // DeviceModel: one row per device.
    required property var devices

    role: CelestinaSurface.Grouped
    Accessible.name: qsTr("Bluetooth")

    Column {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: CelestinaTheme.spaceCardInset

        SettingRow {
            objectName: "poweredRow"
            width: parent.width
            label: qsTr("Bluetooth")
            checked: page.controller.powered
            onToggled: function(on) { page.controller.setPowered(on) }
        }

        Item {
            width: parent.width
            height: CelestinaTheme.rowHeight

            CelestinaButton {
                objectName: "searchButton"
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                enabled: page.controller.powered
                text: page.controller.discovering ? qsTr("Detener búsqueda") : qsTr("Buscar")
                onClicked: page.controller.setDiscovering(!page.controller.discovering)
            }
        }
    }

    ListView {
        id: list
        objectName: "deviceList"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.margins: CelestinaTheme.spaceCardInset
        clip: true
        model: page.devices
        boundsBehavior: Flickable.StopAtBounds
        CelestinaWheelScroll { view: list }
        Accessible.role: Accessible.List
        Accessible.name: qsTr("Dispositivos")

        delegate: DeviceRow {
            width: ListView.view.width
            onPairRequested: function(address) { page.controller.pair(address) }
            onConnectRequested: function(address) { page.controller.connect(address) }
            onDisconnectRequested: function(address) { page.controller.disconnect(address) }
            onForgetRequested: function(address) { page.controller.forget(address) }
        }
    }
}
