pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// One Bluetooth device: kind, name, state and battery, then its actions — pair
// a stranger, connect or disconnect a paired one, and a menu that forgets it
// (the page owns the menu; the row says where to open it).
Item {
    id: row

    // The DeviceModel row: address, name, kind, paired, connected, battery.
    required property var model
    required property int index

    signal pairRequested(string address)
    signal connectRequested(string address)
    signal disconnectRequested(string address)
    signal menuRequested(string address, Item anchor)

    // Set by the list: this row is current and the list holds the focus.
    property bool focused: false
    // The card's `rowInset`, from whoever places the row in a SectionCard.
    required property int inset

    implicitHeight: CelestinaTheme.rowHeight
    Accessible.role: Accessible.ListItem
    // Kind, name, state and, when the device reports it, the battery.
    Accessible.name: {
        const parts = [row.kindLabel(row.model.kind), row.model.name, stateText.text]
        if (row.model.battery >= 0)
            parts.push(qsTr("batería al %1 %").arg(row.model.battery))
        return parts.join(", ")
    }

    // Enter: pair a stranger, connect or disconnect a paired device.
    function primaryAction() {
        if (!row.model.paired)
            row.pairRequested(row.model.address)
        else if (row.model.connected)
            row.disconnectRequested(row.model.address)
        else
            row.connectRequested(row.model.address)
    }

    // Menu or Shift+F10: a paired device's menu opens beside its button.
    function openMenu() {
        if (row.model.paired)
            row.menuRequested(row.model.address, more)
    }

    function kindLabel(kind) {
        switch (kind) {
        case "audio": return qsTr("Dispositivo de audio")
        case "input": return qsTr("Dispositivo de entrada")
        case "phone": return qsTr("Teléfono")
        case "computer": return qsTr("Ordenador")
        default: return qsTr("Dispositivo")
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

    CelestinaIcon {
        id: glyph
        anchors.left: parent.left
        anchors.leftMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.iconMd
        height: CelestinaTheme.iconMd
        name: row.model.kind === "audio" ? "media-volume"
              : row.model.kind === "input" ? "gamepad-2"
              : row.model.kind === "phone" ? "phone"
              : row.model.kind === "computer" ? "monitor" : "bluetooth"
    }

    Column {
        anchors.left: glyph.right
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: battery.left
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
            objectName: "deviceState"
            width: parent.width
            text: row.model.connected ? qsTr("Conectado")
                  : row.model.paired ? qsTr("Emparejado") : qsTr("No emparejado")
            elide: Text.ElideRight
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }
    }

    Text {
        id: battery
        objectName: "deviceBattery"
        anchors.right: actions.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        visible: row.model.battery >= 0
        text: qsTr("%1 %").arg(row.model.battery)
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
    }

    Row {
        id: actions
        anchors.right: parent.right
        anchors.rightMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        CelestinaIconButton {
            objectName: "pairButton"
            visible: !row.model.paired
            role: CelestinaButton.Ghost
            iconName: "plus"
            helpText: qsTr("Emparejar")
            onClicked: row.pairRequested(row.model.address)
        }

        CelestinaIconButton {
            objectName: "joinButton"
            visible: row.model.paired
            role: CelestinaButton.Ghost
            iconName: row.model.connected ? "unlink" : "link"
            helpText: row.model.connected ? qsTr("Desconectar") : qsTr("Conectar")
            onClicked: row.model.connected ? row.disconnectRequested(row.model.address)
                                           : row.connectRequested(row.model.address)
        }

        CelestinaIconButton {
            id: more
            objectName: "menuButton"
            visible: row.model.paired
            role: CelestinaButton.Ghost
            iconName: "chevron-down"
            helpText: qsTr("Más opciones")
            onClicked: row.menuRequested(row.model.address, more)
        }
    }
}
