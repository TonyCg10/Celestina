import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── Paste conflict dialog (skip / replace / keep both) ───────────
Rectangle {
    id: conflictDialog
    property var controller
    property var owner
    anchors.fill: parent
    z: 62
    readonly property bool shown: controller.conflictPending
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
    color: Qt.rgba(0, 0, 0, 0.45)

    // Clicking the dimmed backdrop cancels the whole paste.
    MouseArea {
        anchors.fill: parent
        onClicked: controller.cancelConflicts()
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            controller.cancelConflicts()
            event.accepted = true
        }
    }
    focus: conflictDialog.shown

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(420, owner.width - 48)
        height: applyToAll.visible ? 214 : 176
        backdropSource: mainPanel
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
            font.pixelSize: CelestinaTheme.fontCallout
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
            font.pixelSize: CelestinaTheme.fontLabel
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
            font.pixelSize: CelestinaTheme.fontLabel
            Accessible.name: text

            contentItem: Text {
                text: applyToAll.text
                font: applyToAll.font
                color: CelestinaTheme.text
                verticalAlignment: Text.AlignVCenter
                leftPadding: applyToAll.indicator.width + 8
            }

            indicator: Rectangle {
                implicitWidth: 18
                implicitHeight: 18
                x: applyToAll.leftPadding
                y: applyToAll.height / 2 - height / 2
                radius: CelestinaTheme.radiusXs
                color: applyToAll.checked ? CelestinaTheme.accent
                                          : CelestinaTheme.inputFill
                border.width: 1
                border.color: applyToAll.checked ? CelestinaTheme.accent
                                                 : CelestinaTheme.inputBorder

                Text {
                    anchors.centerIn: parent
                    visible: applyToAll.checked
                    text: "✓"
                    color: CelestinaTheme.canvas
                    font.pixelSize: 12
                    font.weight: CelestinaTheme.weightDemiBold
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

            PillButton {
                text: "Cancelar"
                onClicked: {
                    applyToAll.checked = false
                    controller.cancelConflicts()
                }
            }
            PillButton {
                text: "Omitir"
                onClicked: controller.resolveConflict("skip", applyToAll.checked)
            }
            PillButton {
                text: "Conservar ambos"
                onClicked: controller.resolveConflict("keepboth", applyToAll.checked)
            }
            PillButton {
                text: "Reemplazar"
                primary: true
                onClicked: controller.resolveConflict("replace", applyToAll.checked)
            }
        }
    }
}
