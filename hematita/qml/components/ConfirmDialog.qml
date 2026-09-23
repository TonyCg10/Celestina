import QtQuick
import org.celestina.hematita 1.0

// The one question the suite asks before something that cannot be undone:
// killing a process, stopping or restarting a unit. The caller writes the
// question and the word on the button and gets its own payload back, so one
// dialog serves every page. It owns the focus while it is up; Escape and a
// click outside both mean "no".
CelestinaModalLayer {
    id: layer

    required property Item backdrop
    property string question: ""
    property string confirmText: ""
    // Whatever the caller needs back to act: a pid, a unit name, an object.
    property var payload: null

    signal confirmed(var payload)

    function ask(question, confirmText, payload) {
        layer.question = question
        layer.confirmText = confirmText
        layer.payload = payload
        layer.shown = true
    }

    z: 90
    dismissOnEscape: true
    dismissOnOutsideClick: true
    onDismissRequested: layer.shown = false
    onShownChanged: if (layer.shown) cancelButton.forceActiveFocus()

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(420, layer.width - CelestinaTheme.space3xl)
        height: buttons.y + buttons.height + CelestinaTheme.spaceLg
        backdropSource: layer.backdrop

        Accessible.role: Accessible.Dialog
        Accessible.name: layer.question

        // A click on the card is not a click outside it.
        MouseArea { anchors.fill: parent }

        Text {
            id: question

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: CelestinaTheme.spaceLg
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            text: layer.question
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody

            Accessible.role: Accessible.StaticText
            Accessible.name: question.text
        }

        Row {
            id: buttons

            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: question.bottom
            anchors.topMargin: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceSm

            CelestinaButton {
                id: cancelButton
                text: qsTr("Cancelar")
                onClicked: layer.shown = false
            }

            CelestinaButton {
                text: layer.confirmText
                role: CelestinaButton.Destructive
                onClicked: {
                    layer.confirmed(layer.payload)
                    layer.shown = false
                }
            }
        }
    }
}
