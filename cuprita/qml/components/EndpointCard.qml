pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// One direction of sound — the outputs or the inputs: which endpoint is the
// default (a menu beside the selector picks another), its volume and its mute.
CelestinaSurface {
    id: card

    required property string title
    // An EndpointModel of one kind: rows plus defaultId, defaultDescription,
    // defaultPercent and defaultMuted.
    required property var endpoints
    // The item the selector's glass blurs.
    required property Item backdropSource
    property string mutedIcon: "media-volume-muted"
    property string unmutedIcon: "media-volume"

    signal defaultRequested(int id)
    signal volumeRequested(int id, real volume)
    signal muteRequested(int id, bool muted)

    role: CelestinaSurface.Grouped
    implicitHeight: content.implicitHeight + CelestinaTheme.spaceCardInset * 2

    Column {
        id: content
        anchors.fill: parent
        anchors.margins: CelestinaTheme.spaceCardInset
        spacing: CelestinaTheme.spaceSm

        Item {
            width: parent.width
            height: CelestinaTheme.rowHeight

            CelestinaSectionLabel {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: card.title
            }

            CelestinaButton {
                id: selector
                objectName: "endpointSelector"
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                role: CelestinaButton.Ghost
                text: card.endpoints.defaultDescription.length > 0
                      ? card.endpoints.defaultDescription : qsTr("Ninguno")
                helpText: qsTr("Elegir %1").arg(card.title)
                onClicked: menu.popupBeside(selector, false)
            }
        }

        Item {
            width: parent.width
            height: CelestinaTheme.rowHeight
            enabled: card.endpoints.defaultId >= 0

            CelestinaIconButton {
                id: mute
                objectName: "muteButton"
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                role: CelestinaButton.Ghost
                iconName: card.endpoints.defaultMuted ? card.mutedIcon : card.unmutedIcon
                helpText: card.endpoints.defaultMuted ? qsTr("Activar sonido") : qsTr("Silenciar")
                onClicked: card.muteRequested(card.endpoints.defaultId, !card.endpoints.defaultMuted)
            }

            VolumeSlider {
                anchors.left: mute.right
                anchors.leftMargin: CelestinaTheme.spaceMd
                anchors.right: percent.left
                anchors.rightMargin: CelestinaTheme.spaceMd
                anchors.verticalCenter: parent.verticalCenter
                value: card.endpoints.defaultPercent
                Accessible.name: qsTr("Volumen: %1").arg(card.title)
                onRequested: function(value) {
                    card.volumeRequested(card.endpoints.defaultId, value / 100)
                }
            }

            Text {
                id: percent
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("%1 %").arg(card.endpoints.defaultPercent)
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
            }
        }
    }

    GlassContextMenu {
        id: menu
        objectName: "endpointMenu"
        backdropSource: card.backdropSource

        Instantiator {
            model: card.endpoints

            delegate: GlassMenuItem {
                id: choice

                required property var model
                text: choice.model.description
                choice: true
                current: choice.model.isDefault
                onTriggered: card.defaultRequested(choice.model.id)
            }

            onObjectAdded: function(index, object) { menu.insertItem(index, object) }
            onObjectRemoved: function(index, object) { menu.removeItem(object) }
        }
    }
}
