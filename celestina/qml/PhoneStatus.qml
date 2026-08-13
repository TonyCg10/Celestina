// The phone, as Magnetita reports it, and the way in to every device it knows.
//
// A real `PanelMenuButton` for the same reason the clock is one: the
// attachment lease resolves the drop's anchor by walking the panel for marked
// openers, and a surface whose opener is not one is deliberately left
// floating. The phone reading grew a hand-rolled `menuRequested` first, and
// its menu opened with no connection to the bar.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

PanelMenuButton {
    id: phoneRoot

    required property bool connected
    required property bool blurAvailable
    required property int battery
    required property bool charging
    // Non-zero only from a real bar; see PanelPill.
    property real barHeight: 0

    attachmentAnchor: phoneIcon
    visible: phoneRoot.connected
    leftPadding: CelestinaTheme.spaceSm
    rightPadding: CelestinaTheme.spaceSm
    implicitWidth: layout.implicitWidth + leftPadding + rightPadding
    Accessible.name: phoneRoot.battery >= 0
                     ? qsTr("Teléfono, batería %1 por ciento%2")
                         .arg(phoneRoot.battery)
                         .arg(phoneRoot.charging ? qsTr(", cargando") : "")
                     : qsTr("Teléfono conectado")
    Accessible.description: qsTr("Abre el menú del móvil")

    // The reading's own capsule, which the rewrite to a real opener dropped:
    // this control sits alone on the flank rather than inside a cluster, so
    // nothing else supplies its glass.
    PanelPill {
        barHeight: phoneRoot.barHeight
        blurAvailable: phoneRoot.blurAvailable
        ink: phoneRoot.ink
    }

    contentItem: Row {
        id: layout

        spacing: CelestinaTheme.spaceXs

        CelestinaIcon {
            id: phoneIcon

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
            font.pixelSize: CelestinaTheme.fontCaption
            font.weight: CelestinaTheme.weightDemiBold
        }
    }
}
