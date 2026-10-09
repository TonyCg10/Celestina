pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// One direction of sound — the outputs or the inputs — as a labelled card:
// which endpoint is the default (a menu beside the selector picks another),
// then its mute, volume and percent. The card's label is its `title`.
SectionCard {
    id: card

    // An EndpointModel of one kind: rows plus defaultId, defaultDescription,
    // defaultPercent and defaultMuted.
    required property var endpoints
    // The item the selector's glass blurs.
    required property Item backdropSource
    property string mutedIcon: "media-volume-muted"
    property string unmutedIcon: "media-volume"
    // False until the controller's first snapshot: a loading row stands in
    // for the selector and the volume.
    property bool loaded: true

    signal defaultRequested(int id)
    signal volumeRequested(int id, real volume)
    signal muteRequested(int id, bool muted)

    LoadingLine {
        objectName: "endpointLoading"
        visible: !card.loaded
        text: qsTr("Leyendo el audio…")
    }

    Item {
        width: parent.width
        height: CelestinaTheme.rowHeight
        visible: card.loaded

        Text {
            anchors.left: parent.left
            anchors.leftMargin: card.rowInset
            anchors.right: selector.left
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.verticalCenter: parent.verticalCenter
            text: qsTr("Dispositivo predeterminado")
            elide: Text.ElideRight
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
        }

        CelestinaButton {
            id: selector
            objectName: "endpointSelector"
            anchors.right: parent.right
            anchors.rightMargin: card.rowInset
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
        visible: card.loaded
        enabled: card.endpoints.defaultId >= 0

        RowDivider { inset: card.rowInset }

        CelestinaIconButton {
            id: mute
            objectName: "muteButton"
            anchors.left: parent.left
            anchors.leftMargin: card.rowInset
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
            anchors.rightMargin: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            text: qsTr("%1 %").arg(card.endpoints.defaultPercent)
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
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
