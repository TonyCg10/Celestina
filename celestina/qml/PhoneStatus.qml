import CelestinaStyle
import QtQuick

// PANEL-1. A plain root lets a glass pill sit behind this
// reading. As a bare `RowLayout` every child of this file was a cell of that
// row, so the pill was laid out beside the battery instead of underneath the
// whole thing.
Item {
    id: phoneRoot

    required property bool connected
    required property bool blurAvailable
    required property int battery
    required property bool charging
    required property BackdropInk ink
    // Non-zero only from a real bar; see PanelPill.
    property real barHeight: 0

    implicitWidth: layout.implicitWidth
    implicitHeight: layout.implicitHeight
    visible: phoneRoot.connected
    Accessible.role: Accessible.StaticText
    Accessible.name: phoneRoot.battery >= 0
                     ? qsTr("Teléfono, batería %1 por ciento%2")
                         .arg(phoneRoot.battery)
                         .arg(phoneRoot.charging ? qsTr(", cargando") : "")
                     : qsTr("Teléfono conectado")

    Row {
        id: layout

        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceXs

        CelestinaIcon {
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            name: "phone"
            tone: CelestinaIcon.Primary
            tintOverride: phoneRoot.ink.primary
            Accessible.ignored: true
        }

        CelestinaIcon {
            anchors.verticalCenter: parent.verticalCenter
            width: visible ? CelestinaTheme.iconSm : 0
            height: CelestinaTheme.iconSm
            visible: phoneRoot.battery >= 0 && phoneRoot.charging
            name: "battery-charging"
            tone: CelestinaIcon.Primary
            tintOverride: phoneRoot.ink.primary
            Accessible.ignored: true
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: phoneRoot.battery >= 0
            text: phoneRoot.battery + " %"
            color: phoneRoot.ink.primary
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontTitle
            font.weight: CelestinaTheme.weightDemiBold
            Accessible.ignored: true
        }
    }

    PanelPill {
        barHeight: phoneRoot.barHeight
        blurAvailable: phoneRoot.blurAvailable
        ink: phoneRoot.ink
    }

}
