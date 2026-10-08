pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"

// The Audio section: the output and input cards, the applications playing,
// and a profile card for each sound card that has more than one profile.
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

        // One profile card per sound card that has more than one profile
        // (the controller lists only those).
        Repeater {
            model: {
                const ids = []
                const list = page.controller.profiles
                for (let i = 0; i < list.length; ++i)
                    if (ids.indexOf(list[i].cardId) < 0)
                        ids.push(list[i].cardId)
                return ids
            }

            delegate: CelestinaSurface {
                id: profileCard
                objectName: "profileCard"

                required property int modelData
                readonly property var choices: page.controller.profiles.filter(
                    p => p.cardId === profileCard.modelData)

                width: cards.width
                height: CelestinaTheme.rowHeight + CelestinaTheme.spaceCardInset * 2
                role: CelestinaSurface.Grouped

                CelestinaSectionLabel {
                    anchors.left: parent.left
                    anchors.leftMargin: CelestinaTheme.spaceCardInset
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Perfil")
                }

                CelestinaButton {
                    id: profileSelector
                    objectName: "profileSelector"
                    anchors.right: parent.right
                    anchors.rightMargin: CelestinaTheme.spaceCardInset
                    anchors.verticalCenter: parent.verticalCenter
                    role: CelestinaButton.Ghost
                    text: {
                        const list = profileCard.choices
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
                        model: profileCard.choices

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
            // Keyboard: Tab enters the list once, the arrows walk it, Enter runs
            // the current row's primary action.
            activeFocusOnTab: true
            keyNavigationEnabled: true
            Keys.onReturnPressed: function(event) { list.primary(); event.accepted = true }
            Keys.onEnterPressed: function(event) { list.primary(); event.accepted = true }

            function primary() {
                if (list.currentItem)
                    (list.currentItem as StreamRow).primaryAction()
            }
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Aplicaciones")

            delegate: StreamRow {
                width: ListView.view.width
                focused: list.activeFocus && ListView.isCurrentItem
                onVolumeRequested: function(id, volume) { page.controller.setVolume(id, volume) }
                onMuteRequested: function(id, muted) { page.controller.setMuted(id, muted) }
            }
        }
    }
}
