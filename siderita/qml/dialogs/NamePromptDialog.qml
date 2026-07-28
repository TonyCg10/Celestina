import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── Name prompt (new folder / new file / rename) ─────────────────
Rectangle {
    id: namePrompt
    property var controller
    property var owner
    property var backdrop   // mainPanel: el fondo que difumina el cristal
    anchors.fill: parent
    z: 60
    property bool shown: false
    // Fades rather than pops. Opacity only: a scale transform on a
    // glass surface desyncs its backdrop sampling (see a995619), so the
    // motion here never touches geometry.
    visible: opacity > 0.01
    opacity: shown ? 1 : 0
    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }
    color: CelestinaTheme.scrim

    property string mode: "folder"   // "folder" | "file" | "rename"
    property string targetPath: ""
    property string heading: ""

    function openCreate(kind) {
        namePrompt.mode = kind
        namePrompt.targetPath = ""
        namePrompt.heading = kind === "folder" ? "Nueva carpeta" : "Nuevo archivo"
        promptField.text = ""
        namePrompt.shown = true
        promptField.forceActiveFocus()
    }
    function openRename(path, currentName) {
        namePrompt.mode = "rename"
        namePrompt.targetPath = path
        namePrompt.heading = "Renombrar"
        promptField.text = currentName
        namePrompt.shown = true
        promptField.forceActiveFocus()
        promptField.selectAll()
    }
    function dismiss() {
        namePrompt.shown = false
        promptField.text = ""
        owner.focusView()
    }
    function confirm() {
        const value = promptField.text
        if (value.length === 0) {
            namePrompt.dismiss()
            return
        }
        if (namePrompt.mode === "folder")
            controller.newFolder(value)
        else if (namePrompt.mode === "file")
            controller.newFile(value)
        else
            controller.renamePath(namePrompt.targetPath, value)
        namePrompt.dismiss()
    }

    // Click on the dimmed backdrop cancels.
    MouseArea {
        anchors.fill: parent
        onClicked: namePrompt.dismiss()
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(380, owner.width - 48)
        height: 142
        backdropSource: namePrompt.backdrop
        // (not transform-scaled — a scale transform desynced the glass backdrop)

        // Swallow clicks so they never reach the dismiss backdrop.
        MouseArea { anchors.fill: parent }

        Text {
            id: promptHeading
            x: 18
            y: 16
            text: namePrompt.heading
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        CelestinaTextField {
            id: promptField
            x: 18
            y: promptHeading.y + promptHeading.height + 12
            width: parent.width - 36
            onAccepted: namePrompt.confirm()
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    namePrompt.dismiss()
                    event.accepted = true
                }
            }
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16
            spacing: 8

            CelestinaButton {
                text: "Cancelar"
                onClicked: namePrompt.dismiss()
            }
            CelestinaButton {
                text: "Aceptar"
                primary: true
                onClicked: namePrompt.confirm()
            }
        }
    }
}
