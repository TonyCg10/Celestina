pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"

// The Network section: the Wi-Fi and airplane switches, then every network
// the backend reports — the connection in use first. The page reads its
// controller and model from the window, so tests can hand it stand-ins.
CelestinaSurface {
    id: page

    // NetworkController: wifiEnabled, airplane and the commands.
    required property var controller
    // NetworkModel: one row per network.
    required property var networks

    role: CelestinaSurface.Grouped
    Accessible.name: qsTr("Red")

    Column {
        id: switches
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: CelestinaTheme.spaceCardInset

        SettingRow {
            objectName: "wifiRow"
            width: parent.width
            label: qsTr("Wi-Fi")
            checked: page.controller.wifiEnabled
            onToggled: function(on) { page.controller.setWifiEnabled(on) }
        }

        SettingRow {
            objectName: "airplaneRow"
            width: parent.width
            label: qsTr("Modo avión")
            checked: page.controller.airplane
            onToggled: function(on) { page.controller.setAirplane(on) }
        }
    }

    ListView {
        id: list
        objectName: "networkList"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: switches.bottom
        anchors.bottom: parent.bottom
        anchors.margins: CelestinaTheme.spaceCardInset
        clip: true
        model: page.networks
        boundsBehavior: Flickable.StopAtBounds
        CelestinaWheelScroll { view: list }
        Accessible.role: Accessible.List
        Accessible.name: qsTr("Redes")

        delegate: NetworkRow {
            width: ListView.view.width
            onConnectRequested: function(id) { page.controller.connect(id, "") }
            onDisconnectRequested: function(id) { page.controller.disconnect(id) }
            onForgetRequested: function(id) { page.controller.forget(id) }
            onVpnRequested: function(id, on) { page.controller.setVpnActive(id, on) }
        }
    }
}
