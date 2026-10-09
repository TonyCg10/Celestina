pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// The connection in use, as one large row: its kind, its name, the kind and
// IPv4 address read-only beneath, and a button that leaves it.
Item {
    id: row

    // The NetworkModel row: id, name, kind, state, address.
    required property var model

    signal disconnectRequested(string id)

    readonly property string kindText: row.model.kind === "wifi" ? qsTr("Wi-Fi") : qsTr("Ethernet")
    // The card's `rowInset`, from whoever places the row in a SectionCard.
    required property int inset

    width: parent ? parent.width : 0
    height: CelestinaTheme.rowHeightLg
    Accessible.role: Accessible.StaticText
    Accessible.name: qsTr("Conectado a %1, %2").arg(row.model.name).arg(detail.text)

    HoverHandler { id: hover }

    CelestinaRowHighlight {
        anchors.fill: parent
        radius: CelestinaTheme.radiusMd
        family: CelestinaRowHighlight.Content
        hovered: hover.hovered
    }

    CelestinaIcon {
        id: glyph
        anchors.left: parent.left
        anchors.leftMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.iconLg
        height: CelestinaTheme.iconLg
        name: row.model.kind === "wifi" ? "wifi" : "link"
        tone: CelestinaIcon.Accent
    }

    Column {
        anchors.left: glyph.right
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: leave.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter

        Text {
            objectName: "connectionName"
            width: parent.width
            text: row.model.name
            elide: Text.ElideRight
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
        }

        Text {
            id: detail
            objectName: "connectionDetail"
            width: parent.width
            text: row.model.address ? row.kindText + " · " + row.model.address : row.kindText
            elide: Text.ElideRight
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }
    }

    CelestinaButton {
        id: leave
        objectName: "disconnectButton"
        anchors.right: parent.right
        anchors.rightMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        role: CelestinaButton.Ghost
        text: qsTr("Desconectar")
        helpText: qsTr("Desconectar de %1").arg(row.model.name)
        onClicked: row.disconnectRequested(row.model.id)
    }
}
