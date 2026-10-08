pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"
import "../dialogs"

// The Network section: the Wi-Fi and airplane switches, then every network
// the backend reports — the connection in use first. Joining a protected
// Wi-Fi network without a saved profile asks for its passphrase first. The
// page reads its controller and model from the window, so tests can hand it
// stand-ins.
CelestinaSurface {
    id: page

    // NetworkController: wifiEnabled, airplane and the commands.
    required property var controller
    // NetworkModel: one row per network.
    required property var networks
    // What the password dialog's glass samples.
    required property Item backdropSource

    // The network a join was just asked for: a second request for it is
    // ignored until the controller has finished its commands.
    property string joiningId: ""

    function join(id, password) {
        page.joiningId = id
        page.controller.connect(id, password)
    }

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
            id: networkRow
            width: ListView.view.width
            onConnectRequested: function(id) {
                if (networkRow.model.state === "connecting" || page.joiningId === id)
                    return
                if (networkRow.model.security === "psk" && !networkRow.model.known)
                    passwordDialog.ask(id, networkRow.model.name)
                else
                    page.join(id, "")
            }
            onDisconnectRequested: function(id) { page.controller.disconnect(id) }
            onForgetRequested: function(id) { page.controller.forget(id) }
            onVpnRequested: function(id, on) { page.controller.setVpnActive(id, on) }
        }
    }

    WifiPasswordDialog {
        id: passwordDialog
        objectName: "wifiPasswordDialog"
        anchors.fill: parent
        backdrop: page.backdropSource
        onConnectRequested: function(id, password) { page.join(id, password) }
    }

    Connections {
        target: page.controller
        function onBusyChanged() {
            if (!page.controller.busy)
                page.joiningId = ""
        }
        // A command that could not even be queued reports only a notice.
        function onNotice(kind, text) { page.joiningId = "" }
    }
}
