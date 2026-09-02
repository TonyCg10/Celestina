pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// What can be done to a handful of files at once.
//
// It appears when something is picked out and goes when nothing is, so it is
// never furniture. Only the operations that mean the same thing on every
// picture are here: turning, mirroring, and forgetting what a photograph
// carries. A crop measured on one photograph names a different part of the
// next, and a word belongs where it was written — so neither is offered.
Item {
    id: bar

    required property FluoritaBatch batch
    // The keys picked out, in the order they were picked.
    required property var keys

    signal dismissed()

    implicitHeight: row.implicitHeight + CelestinaTheme.spaceMd * 2
    implicitWidth: row.implicitWidth + CelestinaTheme.spaceLg * 2

    Accessible.role: Accessible.ToolBar
    Accessible.name: qsTr("Acciones sobre la selección")

    GlassSurface {
        anchors.fill: parent
        cornerRadius: CelestinaTheme.radiusPill
    }

    // The pill floats over the grid, whose cards take hover and every button:
    // without this a click on the count or a divider opened the card beneath,
    // and resting on the pill over a film started its preview.
    CelestinaInputShield { }

    RowLayout {
        id: row

        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceMd

        CelestinaSectionLabel {
            text: bar.batch.running
                ? qsTr("%1 de %2").arg(bar.batch.done + bar.batch.skipped + bar.batch.failed)
                                  .arg(bar.batch.total)
                : qsTr("%1 elegidos").arg(bar.keys.length)
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            Layout.topMargin: CelestinaTheme.spaceXs
            Layout.bottomMargin: CelestinaTheme.spaceXs
            color: CelestinaTheme.divider
        }

        Repeater {
            model: [
                { operation: "turn-left", icon: "rotate-ccw", label: qsTr("Girar a la izquierda") },
                { operation: "turn-right", icon: "rotate-ccw", label: qsTr("Girar a la derecha") },
                { operation: "mirror-h", icon: "symlink", label: qsTr("Voltear en horizontal") },
                { operation: "forget", icon: "eye-off", label: qsTr("Quitar datos personales") }
            ]

            delegate: CelestinaIconButton {
                required property var modelData

                iconName: modelData.icon
                helpText: modelData.label
                // An operation the whole selection would skip is not offered:
                // a button that reports "0 changed" is a button that wasted a
                // decision.
                enabled: !bar.batch.running && bar.batch.admits(bar.keys, modelData.operation)
                onClicked: bar.batch.run(bar.keys, modelData.operation, false)
            }
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            Layout.topMargin: CelestinaTheme.spaceXs
            Layout.bottomMargin: CelestinaTheme.spaceXs
            color: CelestinaTheme.divider
        }

        CelestinaIconButton {
            iconName: "x"
            helpText: bar.batch.running ? qsTr("Detener") : qsTr("Dejar de elegir")
            onClicked: {
                if (bar.batch.running) {
                    bar.batch.cancel()
                } else {
                    bar.dismissed()
                }
            }
        }
    }
}
