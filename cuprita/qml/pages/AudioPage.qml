pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"

// The Audio section: the output and input cards, the applications playing,
// and the sound card's profile when it has more than one.
Item {
    id: page

    // AudioController: profiles and the commands.
    required property var controller
    // EndpointModels of kind "sink" and "source", and the StreamModel.
    required property var sinks
    required property var sources
    required property var streams
    // The item the menus' glass blurs.
    required property Item backdropSource

    Accessible.name: qsTr("Audio")

    Column {
        id: cards
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: CelestinaTheme.spaceCardGap

        EndpointCard {
            objectName: "outputCard"
            width: parent.width
            title: qsTr("Salida")
            endpoints: page.sinks
            backdropSource: page.backdropSource
            onDefaultRequested: function(id) { page.controller.setDefault(id) }
            onVolumeRequested: function(id, volume) { page.controller.setVolume(id, volume) }
            onMuteRequested: function(id, muted) { page.controller.setMuted(id, muted) }
        }

        EndpointCard {
            objectName: "inputCard"
            width: parent.width
            title: qsTr("Entrada")
            endpoints: page.sources
            backdropSource: page.backdropSource
            mutedIcon: "mic-off"
            unmutedIcon: "mic"
            onDefaultRequested: function(id) { page.controller.setDefault(id) }
            onVolumeRequested: function(id, volume) { page.controller.setVolume(id, volume) }
            onMuteRequested: function(id, muted) { page.controller.setMuted(id, muted) }
        }

        CelestinaSurface {
            objectName: "profileCard"
            width: parent.width
            height: CelestinaTheme.rowHeight + CelestinaTheme.spaceCardInset * 2
            visible: page.controller.profiles.length > 0
            role: CelestinaSurface.Grouped

            CelestinaSectionLabel {
                anchors.left: parent.left
                anchors.leftMargin: CelestinaTheme.spaceCardInset
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Perfil")
            }

            CelestinaButton {
                id: profileSelector
                anchors.right: parent.right
                anchors.rightMargin: CelestinaTheme.spaceCardInset
                anchors.verticalCenter: parent.verticalCenter
                role: CelestinaButton.Ghost
                text: {
                    const list = page.controller.profiles
                    for (let i = 0; i < list.length; ++i)
                        if (list[i].active)
                            return list[i].description
                    return ""
                }
                helpText: qsTr("Elegir perfil")
                onClicked: profileMenu.popupBeside(profileSelector, false)
            }

            GlassContextMenu {
                id: profileMenu
                backdropSource: page.backdropSource

                Instantiator {
                    model: page.controller.profiles

                    delegate: GlassMenuItem {
                        id: profileChoice

                        required property var modelData
                        text: profileChoice.modelData.description
                        choice: true
                        current: profileChoice.modelData.active
                        onTriggered: page.controller.setProfile(profileChoice.modelData.cardId,
                                                                profileChoice.modelData.id)
                    }

                    onObjectAdded: function(index, object) { profileMenu.insertItem(index, object) }
                    onObjectRemoved: function(index, object) { profileMenu.removeItem(object) }
                }
            }
        }
    }

    CelestinaSurface {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: cards.bottom
        anchors.topMargin: CelestinaTheme.spaceCardGap
        anchors.bottom: parent.bottom
        role: CelestinaSurface.Grouped

        ListView {
            id: list
            objectName: "streamList"
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceCardInset
            clip: true
            model: page.streams
            boundsBehavior: Flickable.StopAtBounds
            CelestinaWheelScroll { view: list }
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Aplicaciones")

            delegate: StreamRow {
                width: ListView.view.width
                onVolumeRequested: function(id, volume) { page.controller.setVolume(id, volume) }
                onMuteRequested: function(id, muted) { page.controller.setMuted(id, muted) }
            }
        }
    }
}
