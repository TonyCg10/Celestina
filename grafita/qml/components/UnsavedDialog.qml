import QtQuick
import org.celestina.grafita 1.0

// A document with unsaved work does not close on a keystroke, and the window
// does not quit around it. The question owns the focus while it is up and every
// answer is explicit.
CelestinaModalLayer {
    id: layer

    required property var session
    required property Item backdrop

    /// The user chose to stay. Whoever asked for the close — a tab, or a quit
    /// walking every tab — has to hear that, or one "Cancelar" would leave the
    /// window closing anyway.
    signal cancelled

    z: 90
    shown: session.closePrompt
    // Escape and a click outside both mean "cancel" here — an explicit answer,
    // never a way to dodge the question.
    dismissOnEscape: true
    dismissOnOutsideClick: true
    onDismissRequested: { layer.session.cancelClose(); layer.cancelled() }

    onShownChanged: if (shown) keepButton.forceActiveFocus()

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(420, layer.width - CelestinaTheme.space3xl)
        height: question.height + buttons.height + CelestinaTheme.space3xl
        backdropSource: layer.backdrop

        Accessible.role: Accessible.Dialog
        Accessible.name: "Cambios sin guardar"

        // Clicks on the card never reach the dismissing backdrop.
        MouseArea { anchors.fill: parent }

        Text {
            id: question
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: CelestinaTheme.spaceLg
            anchors.top: parent.top
            anchors.topMargin: CelestinaTheme.spaceLg
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            text: "«" + layer.session.name + "» tiene cambios sin guardar."
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
                text: "Cancelar"
                onClicked: { layer.session.cancelClose(); layer.cancelled() }
            }
            CelestinaButton {
                text: "Descartar"
                onClicked: layer.session.discardAndClose()
            }
            CelestinaButton {
                id: keepButton
                text: "Guardar"
                role: CelestinaButton.Primary
                onClicked: layer.session.saveAndClose()
            }
        }
    }
}
