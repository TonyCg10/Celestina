pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"

// The Audio section, in cards: output and input, the applications playing,
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

    PageScroll {
        id: scroll
        anchors.fill: parent

        EndpointCard {
            objectName: "outputCard"
            width: parent.width
            title: qsTr("Salida")
            loaded: page.controller.loaded
            working: page.controller.loaded && page.controller.busy
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
            loaded: page.controller.loaded
            working: page.controller.loaded && page.controller.busy
            endpoints: page.sources
            backdropSource: page.backdropSource
            mutedIcon: "mic-off"
            unmutedIcon: "mic"
            onDefaultRequested: function(id) { page.controller.setDefault(id) }
            onVolumeRequested: function(id, volume) { page.controller.setVolume(id, volume) }
            onMuteRequested: function(id, muted) { page.controller.setMuted(id, muted) }
        }

        SectionCard {
            id: appsCard
            width: parent.width
            title: qsTr("Aplicaciones")
            working: page.controller.loaded && page.controller.busy

            ListView {
                id: list
                objectName: "streamList"
                width: parent.width
                height: list.contentHeight
                interactive: false
                model: page.streams
                // Keyboard: Tab enters the list once, the arrows walk it and
                // Enter runs the current row's primary action; the page
                // scrolls to keep the current row in view.
                activeFocusOnTab: true
                keyNavigationEnabled: true
                Keys.onReturnPressed: function(event) { list.primary(); event.accepted = true }
                Keys.onEnterPressed: function(event) { list.primary(); event.accepted = true }
                onCurrentItemChanged: if (list.activeFocus) scroll.reveal(list.currentItem)
                onActiveFocusChanged: if (list.activeFocus) scroll.reveal(list.currentItem)

                function primary() {
                    if (list.currentItem)
                        (list.currentItem as StreamRow).primaryAction()
                }
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Aplicaciones")

                delegate: StreamRow {
                    inset: appsCard.rowInset
                    width: ListView.view.width
                    focused: list.activeFocus && ListView.isCurrentItem
                    onVolumeRequested: function(id, volume) { page.controller.setVolume(id, volume) }
                    onMuteRequested: function(id, muted) { page.controller.setMuted(id, muted) }
                }
            }

            LoadingLine {
                objectName: "streamsLoading"
                visible: !page.controller.loaded
                text: qsTr("Leyendo el audio…")
            }

            EmptyLine {
                objectName: "noStreams"
                inset: appsCard.rowInset
                visible: page.controller.loaded && list.count === 0
                text: qsTr("Ninguna aplicación suena")
            }
        }

        // One profile card per sound card that has more than one profile
        // (the controller lists only those).
        Repeater {
            model: {
                const ids = []
                const choices = page.controller.profiles
                for (let i = 0; i < choices.length; ++i)
                    if (ids.indexOf(choices[i].cardId) < 0)
                        ids.push(choices[i].cardId)
                return ids
            }

            delegate: SectionCard {
                id: profileCard
                objectName: "profileCard"

                required property int modelData
                readonly property var choices: page.controller.profiles.filter(
                    p => p.cardId === profileCard.modelData)

                width: parent.width
                title: qsTr("Perfil")

                Item {
                    width: parent.width
                    height: CelestinaTheme.rowHeight

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: profileCard.rowInset
                        anchors.right: profileSelector.left
                        anchors.rightMargin: CelestinaTheme.spaceMd
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Perfil de la tarjeta")
                        elide: Text.ElideRight
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowTitle
                    }

                    CelestinaButton {
                        id: profileSelector
                        objectName: "profileSelector"
                        anchors.right: parent.right
                        anchors.rightMargin: profileCard.rowInset
                        anchors.verticalCenter: parent.verticalCenter
                        role: CelestinaButton.Ghost
                        text: {
                            const choices = profileCard.choices
                            for (let i = 0; i < choices.length; ++i)
                                if (choices[i].active)
                                    return choices[i].description
                            return ""
                        }
                        helpText: qsTr("Elegir perfil")
                        onClicked: profileMenu.popupBeside(profileSelector, false)
                    }
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
}
