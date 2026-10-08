pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"
import "../dialogs"

// The Bluetooth section: the adapter switch and a search button that spins
// while the adapter searches, then the devices the adapter knows or sees.
// What the pairing agent asks arrives as the controller's `agentRequest` and
// is answered in PairingDialog; the page reads its controller and model from
// the window, so tests can hand it stand-ins.
CelestinaSurface {
    id: page

    // BluetoothController: powered, discovering and the commands.
    required property var controller
    // DeviceModel: one row per device.
    required property var devices
    // What the pairing dialog's and the row menu's glass samples.
    required property Item backdropSource

    // The device the row menu was opened for.
    property string menuAddress: ""

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
            // Airplane mode keeps the adapter off until it ends.
            enabled: !page.controller.airplane
            onToggled: function(on) { page.controller.setPowered(on) }
        }

        Item {
            width: parent.width
            height: CelestinaTheme.rowHeight

            CelestinaIcon {
                id: spinner
                objectName: "searchSpinner"
                anchors.right: searchButton.left
                anchors.rightMargin: CelestinaTheme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.iconMd
                height: CelestinaTheme.iconMd
                name: "view-refresh"
                visible: page.controller.discovering
                Accessible.ignored: true

                // The glyph is symmetric under a half turn: one half turn
                // per cycle reads as continuous spinning.
                RotationAnimator on rotation {
                    from: 0
                    to: 180
                    duration: CelestinaTheme.motionCeiling
                    loops: Animation.Infinite
                    running: spinner.visible && !CelestinaTheme.reducedMotion
                }
            }

            CelestinaButton {
                id: searchButton
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
            onMenuRequested: function(address, anchor) {
                page.menuAddress = address
                deviceMenu.popupBeside(anchor, false)
            }
        }
    }

    GlassContextMenu {
        id: deviceMenu
        objectName: "deviceMenu"
        backdropSource: page.backdropSource

        GlassMenuItem {
            objectName: "forgetItem"
            text: qsTr("Olvidar")
            onTriggered: page.controller.forget(page.menuAddress)
        }
    }

    PairingDialog {
        id: pairingDialog
        objectName: "pairingDialog"
        anchors.fill: parent
        backdrop: page.backdropSource
        onAnswerRequested: function(kind, value) { page.controller.answerAgent(kind, value) }
    }

    Connections {
        target: page.controller
        function onAgentRequest(kind, device, passkey) { pairingDialog.ask(kind, device, passkey) }
        function onAgentClosed() { pairingDialog.close() }
    }
}
