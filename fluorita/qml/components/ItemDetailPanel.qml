import QtQuick
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// What the library knows about one item.
//
// Built on the suite's shared modal exactly like Siderita's dialogs:
// `CelestinaModalLayer` owns the scrim, the fade, focus containment and
// restoration and the dismissal semantics, and `GlassCard` is the frosted card.
// Only the fields are Fluorita's.
//
// Every value is already a display string from the adapter, and filling them
// opened no file: properties must not cost what playing costs. A field the
// catalogue never learned — a duration nobody probed — is absent rather than
// shown as a zero it would be wrong to believe.
CelestinaModalLayer {
    id: panel

    required property FluoritaLibrary library
    // The item to blur behind the card, handed over by the host. The modal
    // never reaches for it itself.
    required property Item backdrop

    anchors.fill: parent
    z: 60
    shown: panel.library.detailOpen
    onDismissRequested: panel.library.closeDetail()

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(520, panel.width - CelestinaTheme.spaceLg * 3)
        height: fields.implicitHeight + CelestinaTheme.spaceLg * 2
        backdropSource: panel.backdrop

        // Swallows clicks so they never reach the dismiss layer behind.
        MouseArea { anchors.fill: parent }

        ColumnLayout {
            id: fields

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceSm

            CelestinaSectionLabel {
                Layout.fillWidth: true
                text: qsTr("Propiedades")
            }

            Text {
                Layout.fillWidth: true
                text: panel.library.detailName
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideMiddle
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            Repeater {
                model: [
                    { label: qsTr("Tipo"), value: panel.library.detailKind },
                    { label: qsTr("Carpeta"), value: panel.library.detailFolder },
                    { label: qsTr("Tamaño"), value: panel.library.detailSize },
                    { label: qsTr("Modificado"), value: panel.library.detailModified },
                    { label: qsTr("Duración"), value: panel.library.detailDuration },
                    { label: qsTr("Ubicación"), value: panel.library.detailLocation }
                ]

                RowLayout {
                    required property var modelData

                    Layout.fillWidth: true
                    spacing: CelestinaTheme.spaceMd
                    // A field the catalogue never learned is absent, not blank.
                    visible: modelData.value.length > 0

                    Text {
                        Layout.preferredWidth: CelestinaTheme.spaceLg * 5
                        text: parent.modelData.label
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowSecondary
                        Accessible.role: Accessible.StaticText
                        Accessible.name: text
                    }

                    Text {
                        Layout.fillWidth: true
                        text: parent.modelData.value
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowSecondary
                        elide: Text.ElideMiddle
                        Accessible.role: Accessible.StaticText
                        Accessible.name: qsTr("%1: %2").arg(parent.modelData.label).arg(text)
                    }
                }
            }

            Text {
                Layout.fillWidth: true
                visible: panel.library.detailNotice.length > 0
                text: panel.library.detailNotice
                color: CelestinaTheme.warning
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            CelestinaIconButton {
                id: closeAction

                Layout.alignment: Qt.AlignRight
                activeFocusOnTab: true
                role: CelestinaButton.Primary
                iconName: "x"
                helpText: qsTr("Cerrar")
                onClicked: panel.library.closeDetail()
            }
        }
    }

    // The layer contains and restores focus; this only points it at the one
    // action the card has, so Escape and the button agree.
    onShownChanged: if (panel.shown) closeAction.forceActiveFocus()

    Accessible.role: Accessible.Dialog
    Accessible.name: qsTr("Propiedades de %1").arg(panel.library.detailName)
}
