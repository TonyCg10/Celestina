pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.fluorita 1.0

// What leaving an edit with unsaved changes asks.
//
// The three answers are the editor's two save outcomes, by the names the
// author gave them, and throwing the changes away; nothing else is offered
// and nothing is decided here. «Guardar ambas» is the default: it is the
// answer that loses nothing. Escape and a click outside mean "stay", an
// explicit answer rather than a way past the question — Grafita's unsaved
// dialog is the model.
CelestinaModalLayer {
    id: question

    // A copy beside the original («Guardar ambas»).
    signal copyChosen()
    // The original replaced and sent to the Trash («Guardar solo la editada»).
    signal replaceChosen()
    signal discardChosen()

    dismissOnEscape: true
    dismissOnOutsideClick: true
    onDismissRequested: question.shown = false
    onShownChanged: if (question.shown) keepBoth.forceActiveFocus()

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(question.width - CelestinaTheme.spaceXl * 2,
                        Math.max(words.implicitWidth, buttons.implicitWidth)
                            + CelestinaTheme.spaceLg * 2)
        // Measured from where the buttons end, so a wrapped sentence never
        // pushes them off the card.
        height: buttons.y + buttons.height + CelestinaTheme.spaceLg

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Cambios sin guardar")

        // A click on the card is not a click outside the question.
        MouseArea {
            anchors.fill: parent
        }

        Text {
            id: words

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: CelestinaTheme.spaceLg
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            text: qsTr("Esta imagen tiene cambios sin guardar.")
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody

            Accessible.role: Accessible.StaticText
            Accessible.name: words.text
        }

        Row {
            id: buttons

            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: words.bottom
            anchors.topMargin: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceSm

            CelestinaButton {
                text: qsTr("Descartar")
                onClicked: question.discardChosen()
            }

            CelestinaButton {
                text: qsTr("Guardar solo la editada")
                onClicked: question.replaceChosen()
            }

            CelestinaButton {
                id: keepBoth

                text: qsTr("Guardar ambas")
                role: CelestinaButton.Primary
                onClicked: question.copyChosen()
            }
        }
    }
}
