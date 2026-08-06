import QtQuick
import org.celestina.siderita 1.0

// ─── PickerOverwriteDialog ───────────────────────────────────────────────────
// The confirmation the save dialog was missing before it overwrote a file.
// Clicking an existing entry fills the name field with its name, so answering
// with that path unasked turns one click into a destroyed file; every ordinary
// portal backend interposes this question.
//
// It decides nothing: the window hands it a path already composed, and already
// checked for existence on the Rust side, and this either emits `confirmed` or
// closes.
// ──────────────────────────────────────────────────────────────────────────────
CelestinaModalLayer {
    id: overwriteDialog

    // What the glass samples behind the card (the picker's content box).
    required property Item backdrop
    // The surface the card sizes itself against.
    required property Item owner

    // The path that was asked about, and the affirmative answer.
    property string targetPath: ""
    readonly property string targetName: {
        const cut = overwriteDialog.targetPath.lastIndexOf("/")
        return cut >= 0 ? overwriteDialog.targetPath.substring(cut + 1)
                        : overwriteDialog.targetPath
    }

    signal confirmed(string path)

    function ask(path) {
        overwriteDialog.targetPath = path
        overwriteDialog.shown = true
    }

    anchors.fill: parent
    z: 70
    shown: false
    onDismissRequested: overwriteDialog.shown = false

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(400, overwriteDialog.owner.width - 48)
        height: 168
        backdropSource: overwriteDialog.backdrop
        Accessible.role: Accessible.Dialog
        Accessible.name: heading.text

        // Swallow clicks so none of them reach the dismissing backdrop.
        MouseArea { anchors.fill: parent }

        Text {
            id: heading
            x: 18
            y: 16
            text: qsTr("Ya existe")
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        Text {
            x: 18
            y: heading.y + heading.height + 10
            width: parent.width - 36
            wrapMode: Text.Wrap
            text: qsTr("«%1» ya existe en esta carpeta. Se reemplazará su contenido.")
                    .arg(overwriteDialog.targetName)
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16
            spacing: 8

            CelestinaButton {
                text: qsTr("Cancelar")
                onClicked: overwriteDialog.shown = false
            }
            CelestinaButton {
                text: qsTr("Reemplazar")
                role: CelestinaButton.Primary
                onClicked: {
                    const path = overwriteDialog.targetPath
                    overwriteDialog.shown = false
                    overwriteDialog.confirmed(path)
                }
            }
        }
    }
}
