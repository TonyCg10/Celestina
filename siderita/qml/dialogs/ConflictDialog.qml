import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── Paste conflict dialog (skip / replace / keep both) ───────────
CelestinaModalLayer {
    id: conflictDialog
    property var controller
    property var owner
    property var backdrop   // mainPanel: el fondo que difumina el cristal
    anchors.fill: parent
    z: 62
    shown: controller.conflictPending
    onDismissRequested: controller.cancelConflicts()

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(420, owner.width - 48)
        height: applyToAll.visible ? 214 : 176
        backdropSource: conflictDialog.backdrop
        // (not transform-scaled — a scale transform desynced the glass backdrop)
        Accessible.role: Accessible.Dialog
        Accessible.name: "Conflicto al pegar"

        // Swallow clicks so they never reach the dismiss backdrop.
        MouseArea { anchors.fill: parent }

        Text {
            id: conflictHeading
            x: 18
            y: 16
            text: "Ya existe"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        Text {
            id: conflictBody
            x: 18
            y: conflictHeading.y + conflictHeading.height + 10
            width: parent.width - 36
            wrapMode: Text.Wrap
            text: {
                var base = "«" + controller.conflictName
                           + "» ya existe en esta carpeta."
                if (controller.conflictCount > 1)
                    base += " Quedan " + (controller.conflictCount - 1)
                            + " conflicto(s) después de este."
                return base
            }
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }

        // Each collision is asked about on its own; answering for the
        // whole batch is a choice, not the only option. Unticked again
        // whenever a new batch opens, so a past "all" never decides a
        // future paste.
        CheckBox {
            id: applyToAll
            x: 14
            y: conflictBody.y + conflictBody.height + 10
            visible: controller.conflictCount > 1
            text: "Aplicar a los " + controller.conflictCount + " conflictos"
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
            Accessible.name: text

            contentItem: Text {
                text: applyToAll.text
                font: applyToAll.font
                color: CelestinaTheme.text
                verticalAlignment: Text.AlignVCenter
                leftPadding: applyToAll.indicator.width + 8
            }

            indicator: Rectangle {
                implicitWidth: CelestinaTheme.compCheckboxIndicatorSize
                implicitHeight: CelestinaTheme.compCheckboxIndicatorSize
                x: applyToAll.leftPadding
                y: applyToAll.height / 2 - height / 2
                radius: CelestinaTheme.radiusSm
                color: applyToAll.checked ? CelestinaTheme.accent
                                          : CelestinaTheme.inputFill
                border.width: CelestinaTheme.borderHairline
                border.color: applyToAll.checked ? CelestinaTheme.accent
                                                 : CelestinaTheme.inputBorder

                CelestinaIcon {
                    anchors.centerIn: parent
                    width: parent.width - CelestinaTheme.spaceXs
                    height: width
                    visible: applyToAll.checked
                    name: "check"
                    fallbackName: "check"
                    tone: CelestinaIcon.OnAccent
                }
            }
        }

        Connections {
            target: controller
            // A fresh batch starts unticked.
            function onOpRunningChanged() {
                if (controller.opRunning)
                    applyToAll.checked = false
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
                onClicked: {
                    applyToAll.checked = false
                    controller.cancelConflicts()
                }
            }
            CelestinaButton {
                text: "Omitir"
                onClicked: controller.resolveConflict("skip", applyToAll.checked)
            }
            CelestinaButton {
                text: "Conservar ambos"
                onClicked: controller.resolveConflict("keepboth", applyToAll.checked)
            }
            CelestinaButton {
                text: "Reemplazar"
                role: CelestinaButton.Primary
                onClicked: controller.resolveConflict("replace", applyToAll.checked)
            }
        }
    }
}
