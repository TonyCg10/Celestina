pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"
import "../dialogs"

// The Bluetooth section, in two cards: the adapter switch and a search
// button that spins while the adapter searches, then the devices the adapter
// knows or sees.
// What the pairing agent asks arrives as the controller's `agentRequest` and
// is answered in PairingDialog; the page reads its controller and model from
// the window, so tests can hand it stand-ins.
Item {
    id: page

    // BluetoothController: powered, discovering and the commands.
    required property var controller
    // DeviceModel: one row per device.
    required property var devices
    // What the pairing dialog's and the row menu's glass samples.
    required property Item backdropSource

    // The device the row menu was opened for.
    property string menuAddress: ""

    Accessible.name: qsTr("Bluetooth")

    PageScroll {
        id: scroll
        anchors.fill: parent

        // Two rows with a hairline between them: the adapter switch, then
        // the search. One row with both read as a switch and a button that
        // competed for the same words.
        SectionCard {
            id: adapterCard
            width: parent.width
            title: qsTr("Bluetooth")

            SettingRow {
                objectName: "poweredRow"
                inset: adapterCard.rowInset
                width: parent.width
                label: qsTr("Bluetooth")
                checked: page.controller.powered
                // Airplane mode keeps the adapter off until it ends.
                enabled: !page.controller.airplane
                onToggled: function(on) { page.controller.setPowered(on) }
            }

            Item {
                id: searchRow


                width: parent.width
                height: CelestinaTheme.rowHeight

                RowDivider { inset: adapterCard.rowInset }

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: adapterCard.rowInset
                    anchors.right: spinner.left
                    anchors.rightMargin: CelestinaTheme.spaceMd
                    anchors.verticalCenter: parent.verticalCenter
                    text: page.controller.discovering ? qsTr("Buscando dispositivos…")
                                                      : qsTr("Dispositivos cercanos")
                    elide: Text.ElideRight
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                }

                Spinner {
                    id: spinner
                    objectName: "searchSpinner"
                    anchors.right: searchButton.left
                    anchors.rightMargin: CelestinaTheme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    visible: page.controller.discovering
                }

                CelestinaButton {
                    id: searchButton
                    objectName: "searchButton"
                    anchors.right: parent.right
                    anchors.rightMargin: adapterCard.rowInset
                    anchors.verticalCenter: parent.verticalCenter
                    enabled: page.controller.powered
                    text: page.controller.discovering ? qsTr("Detener búsqueda") : qsTr("Buscar")
                    onClicked: page.controller.setDiscovering(!page.controller.discovering)
                }
            }
        }

        SectionCard {
            id: devicesCard
            width: parent.width
            title: qsTr("Dispositivos")
            working: page.controller.loaded && page.controller.busy

            ListView {
                id: list
                objectName: "deviceList"
                width: parent.width
                height: list.contentHeight
                interactive: false
                model: page.devices
                // Keyboard: Tab enters the list once, the arrows walk it and
                // Enter runs the current row's primary action; the page
                // scrolls to keep the current row in view.
                activeFocusOnTab: true
                keyNavigationEnabled: true
                Keys.onReturnPressed: function(event) { list.primary(); event.accepted = true }
                Keys.onEnterPressed: function(event) { list.primary(); event.accepted = true }
                // Menu or Shift+F10 opens the current row's menu beside its button.
                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Menu
                            || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                        if (list.currentItem) {
                            // A wheel-scrolled current row may sit off screen:
                            // show it first, so the menu opens beside a
                            // visible button.
                            scroll.reveal(list.currentItem);
                            (list.currentItem as DeviceRow).openMenu()
                        }
                        event.accepted = true
                    }
                }
                onCurrentItemChanged: if (list.activeFocus) scroll.reveal(list.currentItem)
                onActiveFocusChanged: if (list.activeFocus) scroll.reveal(list.currentItem)

                function primary() {
                    if (list.currentItem)
                        (list.currentItem as DeviceRow).primaryAction()
                }
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Dispositivos")

                delegate: DeviceRow {
                    inset: devicesCard.rowInset
                    width: ListView.view.width
                    focused: list.activeFocus && ListView.isCurrentItem
                    onPairRequested: function(address) { page.controller.pair(address) }
                    onConnectRequested: function(address) { page.controller.connect(address) }
                    onDisconnectRequested: function(address) { page.controller.disconnect(address) }
                    onMenuRequested: function(address, anchor) {
                        page.menuAddress = address
                        deviceMenu.popupBeside(anchor, false)
                    }
                }
            }

            LoadingLine {
                objectName: "devicesLoading"
                visible: !page.controller.loaded
                text: qsTr("Buscando dispositivos…")
            }

            EmptyLine {
                objectName: "noDevices"
                inset: devicesCard.rowInset
                visible: page.controller.loaded && list.count === 0
                text: qsTr("Ningún dispositivo")
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
