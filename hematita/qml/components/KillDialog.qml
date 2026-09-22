import QtQuick
import org.celestina.hematita 1.0

// Killing is not undoable, so it is asked. The question owns the focus while
// it is up; Escape and a click outside both mean "no".
CelestinaModalLayer {
    id: layer

    required property Item backdrop
    property int pid: 0
    property string processName: ""

    signal confirmed(int pid)

    function ask(pid, name) {
        layer.pid = pid
        layer.processName = name
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
        Accessible.name: qsTr("Matar proceso")

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
            text: qsTr("¿Matar «%1» (%2)? El proceso no podrá guardar nada.")
                    .arg(layer.processName).arg(layer.pid)
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
                text: qsTr("Matar")
                role: CelestinaButton.Destructive
                onClicked: {
                    layer.confirmed(layer.pid)
                    layer.shown = false
                }
            }
        }
    }
}
