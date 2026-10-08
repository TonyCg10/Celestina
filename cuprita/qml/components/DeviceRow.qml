pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// One Bluetooth device: kind, name, state and battery, then its actions — pair
// a stranger, connect or disconnect a paired one, forget it.
Item {
    id: row

    // The DeviceModel row: address, name, kind, paired, connected, battery.
    required property var model
    required property int index

    signal pairRequested(string address)
    signal connectRequested(string address)
    signal disconnectRequested(string address)
    signal forgetRequested(string address)

    implicitHeight: CelestinaTheme.rowHeight
    Accessible.role: Accessible.ListItem
    Accessible.name: row.model.name + ", " + stateText.text

    CelestinaIcon {
        id: glyph
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.iconMd
        height: CelestinaTheme.iconMd
        name: row.model.kind === "audio" ? "media-volume"
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
            objectName: "forgetButton"
            visible: row.model.paired
            role: CelestinaButton.Ghost
            iconName: "user-trash"
            helpText: qsTr("Olvidar")
            onClicked: row.forgetRequested(row.model.address)
        }
    }
}
