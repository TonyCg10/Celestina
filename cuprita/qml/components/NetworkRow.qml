pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// One network in the networks card: its signal (or its wire), its name and
// security, then the actions it admits — join or leave it, and forget a saved
// Wi-Fi. What a state means is the model's; the words are here. VPNs and the
// connection in use have cards of their own.
Item {
    id: row

    // The NetworkModel row: id, name, kind, state, signal, bars, security, known.
    required property var model
    required property int index

    signal connectRequested(string id)
    signal disconnectRequested(string id)
    signal forgetRequested(string id)

    readonly property bool connected: row.model.state === "connected"
    // Set by the list: this row is current and the list holds the focus.
    property bool focused: false
    // The card's `rowInset`, from whoever places the row in a SectionCard.
    required property int inset

    implicitHeight: CelestinaTheme.rowHeight
    Accessible.role: Accessible.ListItem
    // Kind, name, state and, for Wi-Fi, security and bars of four.
    Accessible.name: row.model.kind === "wifi"
                     ? [row.kindLabel(row.model.kind), row.model.name,
                        row.stateLabel(row.model.state), row.securityLabel(row.model.security),
                        qsTr("%1 de 4 barras").arg(row.model.bars)].join(", ")
                     : [row.kindLabel(row.model.kind), row.model.name,
                        row.stateLabel(row.model.state)].join(", ")

    // Enter: join the network, or leave it.
    function primaryAction() {
        if (row.connected)
            row.disconnectRequested(row.model.id)
        else
            row.connectRequested(row.model.id)
    }

    function kindLabel(kind) {
        return kind === "wifi" ? qsTr("Red Wi-Fi") : qsTr("Red por cable")
    }

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
        case "wep": return qsTr("WEP (no admitida)")
        default: return qsTr("Abierta")
        }
    }

    HoverHandler { id: hover }

    // Hover, and the keyboard's place in the list: the list says which row is
    // current and whether it holds the focus; the plate draws the ring.
    CelestinaRowHighlight {
        anchors.fill: parent
        radius: CelestinaTheme.radiusMd
        family: CelestinaRowHighlight.Content
        hovered: hover.hovered
        focused: row.focused
    }

    SignalBars {
        id: bars
        objectName: "signalBars"
        anchors.left: parent.left
        anchors.leftMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        visible: row.model.kind === "wifi"
        level: row.model.bars
    }

    CelestinaIcon {
        anchors.fill: bars
        visible: row.model.kind !== "wifi"
        name: "link"
    }

    Column {
        anchors.left: bars.right
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

        // A Wi-Fi network reads its security; a state other than "not in
        // use" comes first. A wire reads its state.
        Text {
            objectName: "networkState"
            width: parent.width
            text: row.model.kind !== "wifi" ? row.stateLabel(row.model.state)
                  : row.model.state === "disconnected" ? row.securityLabel(row.model.security)
                  : row.stateLabel(row.model.state) + " · " + row.securityLabel(row.model.security)
            elide: Text.ElideRight
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }
    }

    Row {
        id: actions
        anchors.right: parent.right
        anchors.rightMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        CelestinaIconButton {
            objectName: "joinButton"
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
