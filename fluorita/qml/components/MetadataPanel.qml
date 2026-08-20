pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.fluorita 1.0

// What a file says about itself, and what of that can be changed.
//
// One panel for two things that are the same work underneath: correcting a
// track's tags and removing what a photograph carries both replace a
// container's metadata block and copy its stream across untouched. Which of the
// two it shows follows what the item admits, so nothing here decides anything.
//
// It offers the same two outcomes as every other edit — a copy beside the
// original, or a replacement whose original goes to the Trash — because a
// person should not have to learn a second rule for the same question.
CelestinaModalLayer {
    id: panel

    required property FluoritaMetadata metadata

    anchors.fill: parent
    shown: panel.metadata.open
    onDismissRequested: panel.metadata.close()

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(520, panel.width - CelestinaTheme.spaceXl * 2)
        height: Math.min(body.implicitHeight + CelestinaTheme.spaceLg * 2,
                         panel.height - CelestinaTheme.spaceXl * 2)

        // A click inside the card is not a click outside the modal.
        MouseArea {
            anchors.fill: parent
        }

        Column {
            id: body

            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceMd

            CelestinaSectionLabel {
                width: parent.width
                text: panel.metadata.name
                elide: Text.ElideMiddle
            }

            // The tag fields, for a container whose tags can be read at all.
            Column {
                width: parent.width
                spacing: CelestinaTheme.spaceSm
                visible: panel.metadata.correctable
                    || panel.metadata.readOnlyReason.length > 0

                Repeater {
                    model: [
                        { label: qsTr("Título"), value: panel.metadata.title },
                        { label: qsTr("Artista"), value: panel.metadata.artist },
                        { label: qsTr("Álbum"), value: panel.metadata.album },
                        { label: qsTr("Artista del álbum"), value: panel.metadata.albumArtist }
                    ]

                    delegate: Row {
                        id: field

                        required property var modelData
                        required property int index

                        width: parent.width
                        spacing: CelestinaTheme.spaceSm

                        CelestinaSectionLabel {
                            width: Math.round(field.width * 0.32)
                            anchors.verticalCenter: parent.verticalCenter
                            text: field.modelData.label
                        }

                        CelestinaTextField {
                            id: entry

                            width: field.width - Math.round(field.width * 0.32)
                                - CelestinaTheme.spaceSm
                            text: field.modelData.value
                            enabled: panel.metadata.correctable && !panel.metadata.busy
                            // Rebound whenever another file is read, so the
                            // panel never shows the previous item's words.
                            Connections {
                                target: panel.metadata
                                function onKeyChanged() { entry.text = field.modelData.value }
                            }

                            Component.onCompleted: values.register(field.index, entry)
                        }
                    }
                }

                CelestinaSectionLabel {
                    width: parent.width
                    visible: panel.metadata.readOnlyReason.length > 0
                    text: panel.metadata.readOnlyReason
                    wrapMode: Text.WordWrap
                }
            }

            // What a photograph is carrying. An empty list is shown as such:
            // "this picture carries nothing" is an answer, not a blank.
            Column {
                width: parent.width
                spacing: CelestinaTheme.spaceSm
                visible: panel.metadata.strippable

                CelestinaSectionLabel {
                    text: qsTr("Esta imagen lleva")
                }

                Repeater {
                    model: panel.metadata.privateFacts

                    delegate: Row {
                        required property string modelData

                        spacing: CelestinaTheme.spaceSm

                        CelestinaIcon {
                            width: CelestinaTheme.iconSm
                            height: CelestinaTheme.iconSm
                            name: "info"
                            tone: CelestinaIcon.Secondary
                        }

                        CelestinaSectionLabel {
                            anchors.verticalCenter: parent.verticalCenter
                            text: parent.modelData
                        }
                    }
                }

                CelestinaSectionLabel {
                    visible: panel.metadata.privateFacts.length === 0
                    text: qsTr("Nada que quitar")
                }
            }

            CelestinaSectionLabel {
                width: parent.width
                visible: panel.metadata.notice.length > 0
                text: panel.metadata.notice
                wrapMode: Text.WordWrap
            }

            // The cover is chosen through the desktop's own picker, so the
            // panel offers the ask and nothing else.
            CelestinaButton {
                visible: panel.metadata.coverable
                enabled: !panel.metadata.busy
                text: qsTr("Elegir portada…")
                onClicked: panel.metadata.chooseCover(true)
            }

            // The same two outcomes, in the same order, as the editor's.
            Row {
                anchors.right: parent.right
                spacing: CelestinaTheme.spaceSm

                CelestinaButton {
                    text: qsTr("Guardar una copia")
                    role: CelestinaButton.Primary
                    enabled: !panel.metadata.busy && values.actionable
                    onClicked: values.commit(false)
                }

                CelestinaButton {
                    text: qsTr("Reemplazar")
                    enabled: !panel.metadata.busy && values.actionable
                    onClicked: values.commit(true)
                }

                CelestinaButton {
                    text: qsTr("Cerrar")
                    enabled: !panel.metadata.busy
                    onClicked: panel.metadata.close()
                }
            }
        }
    }

    // The typed values, kept in one place so the buttons do not reach into the
    // repeater's delegates to find them.
    QtObject {
        id: values

        property var fields: ({})

        readonly property bool actionable: panel.metadata.correctable
            || (panel.metadata.strippable && panel.metadata.privateFacts.length > 0)

        function register(index, field) {
            values.fields[index] = field
        }

        function text(index) {
            const field = values.fields[index]
            return field ? field.text : ""
        }

        function commit(replace) {
            if (panel.metadata.correctable) {
                panel.metadata.correct(values.text(0), values.text(1),
                                       values.text(2), values.text(3), replace)
            } else if (panel.metadata.strippable) {
                panel.metadata.stripPrivate(replace)
            }
        }
    }
}
