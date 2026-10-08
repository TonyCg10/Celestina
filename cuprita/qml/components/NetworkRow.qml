pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// One network: its kind, name and state, then the actions it admits. A VPN is
// switched on and off; any other network is joined or left, and a saved one
// can be forgotten. What a state means is the model's; the words are here.
Item {
    id: row

    // The NetworkModel row: id, name, kind, state, signal, bars, security, known.
    required property var model
    required property int index

    signal connectRequested(string id)
    signal disconnectRequested(string id)
    signal forgetRequested(string id)
    signal vpnRequested(string id, bool on)

    readonly property bool connected: row.model.state === "connected"

    implicitHeight: CelestinaTheme.rowHeight
    Accessible.role: Accessible.ListItem
    Accessible.name: row.model.name + ", " + stateText.text

    function stateLabel(state) {
        switch (state) {
        case "connected": return qsTr("Conectado")
        case "connecting": return qsTr("Conectando…")
        case "failed": return qsTr("Error al conectar")
        default: return qsTr("Desconectado")
        }
    }

    function securityLabel(key) {
        switch (key) {
        case "psk": return qsTr("Protegida")
        case "enterprise": return qsTr("Empresarial")
        default: return qsTr("Abierta")
        }
    }

    CelestinaIcon {
        id: glyph
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.iconMd
        height: CelestinaTheme.iconMd
        name: row.model.kind === "wifi" ? "wifi"
              : row.model.kind === "vpn" ? "key" : "link"
        // A weak Wi-Fi reads fainter; the bar count is the model's.
        opacity: row.model.kind === "wifi"
                 ? Math.max(CelestinaTheme.mutedContentOpacity, row.model.bars / 4) : 1
    }

    Column {
        anchors.left: glyph.right
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: actions.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: parent.width
            text: row.model.name
            elide: Text.ElideRight
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
        }

        Text {
            id: stateText
            objectName: "networkState"
            width: parent.width
            text: row.model.kind === "wifi"
                  ? row.stateLabel(row.model.state) + " · " + row.securityLabel(row.model.security)
                  : row.stateLabel(row.model.state)
            elide: Text.ElideRight
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }
    }

    Row {
        id: actions
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        CelestinaSwitch {
            objectName: "vpnSwitch"
            visible: row.model.kind === "vpn"
            anchors.verticalCenter: parent.verticalCenter
            checked: row.connected
            Accessible.name: qsTr("Activar %1").arg(row.model.name)
            onToggled: row.vpnRequested(row.model.id, checked)
        }

        CelestinaIconButton {
            objectName: "joinButton"
            visible: row.model.kind !== "vpn"
            role: CelestinaButton.Ghost
            iconName: row.connected ? "unlink" : "link"
            helpText: row.connected ? qsTr("Desconectar") : qsTr("Conectar")
            onClicked: row.connected ? row.disconnectRequested(row.model.id)
                                     : row.connectRequested(row.model.id)
        }

        CelestinaIconButton {
            objectName: "forgetButton"
            visible: row.model.known && row.model.kind === "wifi"
            role: CelestinaButton.Ghost
            iconName: "user-trash"
            helpText: qsTr("Olvidar")
            onClicked: row.forgetRequested(row.model.id)
        }
    }
}
