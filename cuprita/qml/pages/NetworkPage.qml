pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"
import "../dialogs"

// The Network section, in cards: the connection in use, the Wi-Fi and
// airplane switches, every other network the backend reports, and the VPNs.
// The model keeps every row; each card shows its own through a filter.
// Joining a protected Wi-Fi network without a saved profile asks for its
// passphrase first. The page reads its controller and model from the window,
// so tests can hand it stand-ins.
Item {
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
    // The connection the connection card shows: the first connected network
    // that is not a VPN (the model puts the connected first).
    readonly property string activeId: connectionRows.keptIds.length > 0
                                       ? connectionRows.keptIds[0] : ""

    function join(id, password) {
        page.joiningId = id
        page.controller.connect(id, password)
    }

    onActiveIdChanged: networkRows.refilter()
    Accessible.name: qsTr("Red")

    RowFilter {
        id: connectionRows
        model: page.networks
        firstOnly: true
        accepts: function(row) { return row.kind !== "vpn" && row.state === "connected" }

        delegate: ConnectionRow {
            inset: connectionCard.rowInset
            objectName: "connectionRow"
            onDisconnectRequested: function(id) { page.controller.disconnect(id) }
        }
    }

    RowFilter {
        id: networkRows
        model: page.networks
        accepts: function(row) { return row.kind !== "vpn" && row.id !== page.activeId }

        delegate: NetworkRow {
            id: networkRow
            width: ListView.view.width
            inset: networksCard.rowInset
            focused: list.activeFocus && ListView.isCurrentItem
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
        }
    }

    RowFilter {
        id: vpnRows
        model: page.networks
        accepts: function(row) { return row.kind === "vpn" }

        delegate: SettingRow {
            id: vpnRow

            // The NetworkModel row of one VPN.
            required property var model

            objectName: "vpnRow"
            inset: vpnCard.rowInset
            accessibleName: qsTr("Activar %1").arg(vpnRow.model.name)
            width: parent ? parent.width : 0
            label: vpnRow.model.name
            checked: vpnRow.model.state === "connected"
            separated: vpnRow.model.id !== vpnRows.keptIds[0]
            onToggled: function(on) { page.controller.setVpnActive(vpnRow.model.id, on) }
        }
    }

    PageScroll {
        id: scroll
        anchors.fill: parent

        SectionCard {
            id: connectionCard
            objectName: "connectionCard"
            width: parent.width
            title: qsTr("Conexión")
            working: page.controller.loaded && page.controller.busy

            Repeater { model: connectionRows }

            EmptyLine {
                objectName: "noConnection"
                inset: connectionCard.rowInset
                visible: page.controller.loaded && !connectionRows.pending
                         && connectionRows.count === 0
                text: qsTr("Sin conexión")
            }
        }

        SectionCard {
            id: wifiCard
            width: parent.width
            title: qsTr("Wi-Fi")

            SettingRow {
                objectName: "wifiRow"
                inset: wifiCard.rowInset
                width: parent.width
                label: qsTr("Wi-Fi")
                checked: page.controller.wifiEnabled
                onToggled: function(on) { page.controller.setWifiEnabled(on) }
            }

            SettingRow {
                objectName: "airplaneRow"
                inset: wifiCard.rowInset
                width: parent.width
                label: qsTr("Modo avión")
                separated: true
                checked: page.controller.airplane
                onToggled: function(on) { page.controller.setAirplane(on) }
            }
        }

        SectionCard {
            id: networksCard
            width: parent.width
            title: qsTr("Redes")
            working: page.controller.loaded && page.controller.busy

            ListView {
                id: list
                objectName: "networkList"
                width: parent.width
                height: list.contentHeight
                interactive: false
                model: networkRows
                // Keyboard: Tab enters the list once, the arrows walk it and
                // Enter runs the current row's primary action; the page
                // scrolls to keep the current row in view.
                activeFocusOnTab: true
                keyNavigationEnabled: true
                Keys.onReturnPressed: function(event) { list.primary(); event.accepted = true }
                Keys.onEnterPressed: function(event) { list.primary(); event.accepted = true }
                onCurrentItemChanged: if (list.activeFocus) scroll.reveal(list.currentItem)
                onActiveFocusChanged: if (list.activeFocus) scroll.reveal(list.currentItem)

                function primary() {
                    if (list.currentItem)
                        (list.currentItem as NetworkRow).primaryAction()
                }
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Redes")
            }

            LoadingLine {
                objectName: "networksLoading"
                visible: !page.controller.loaded
                text: qsTr("Buscando redes…")
            }

            EmptyLine {
                objectName: "noNetworks"
                inset: networksCard.rowInset
                visible: page.controller.loaded && !networkRows.pending && list.count === 0
                text: qsTr("Ninguna red disponible")
            }
        }

        SectionCard {
            id: vpnCard
            objectName: "vpnCard"
            width: parent.width
            visible: vpnRows.count > 0
            title: qsTr("VPN")

            Repeater { model: vpnRows }
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
