pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

// The QR a phone scans to pair over Magnetita's own wire.
//
// The daemon arms a two-minute window and hands back the text; this draws it
// as plain rectangles from the rows the controller projects, so no image
// plugin is involved, and counts the window down. One QR admits one phone:
// when it arrives the code gives way to its name, and when the window runs
// out the code gives way to a fresh try.
Item {
    id: root

    required property DevicesModel devices
    signal dismissRequested

    readonly property bool arrived: root.devices.pairingArrived.length > 0
    readonly property var rows: root.devices.pairingMatrix.length > 0
                                ? root.devices.pairingMatrix.split("\n") : []
    property int secondsLeft: 0
    readonly property bool expired: !root.arrived && root.rows.length > 0 && root.secondsLeft <= 0

    implicitHeight: sheet.implicitHeight

    // A fresh code restarts the count; the controller sets the window before
    // the rows, so the value is right by the time the rows arrive.
    onRowsChanged: root.secondsLeft = root.devices.pairingWindow

    // One rectangle per run of dark modules in a row: a few hundred items for
    // any code this text produces, and nothing an image plugin has to decode.
    function runs(row) {
        var out = []
        var start = -1
        for (var x = 0; x <= row.length; x++) {
            var dark = x < row.length && row.charAt(x) === "#"
            if (dark && start < 0)
                start = x
            else if (!dark && start >= 0) {
                out.push({ x: start, w: x - start })
                start = -1
            }
        }
        return out
    }

    Timer {
        interval: 1000
        running: root.visible && root.rows.length > 0 && root.secondsLeft > 0
        repeat: true
        onTriggered: root.secondsLeft -= 1
    }

    Column {
        id: sheet
        width: parent.width
        spacing: CelestinaTheme.spaceSm

        CelestinaSectionLabel { text: qsTr("Vincular un teléfono") }

        // The code on the theme's sheet: scanners want dark modules on a light
        // ground whatever the scheme, so the card is the icon sheet and the
        // modules the dark scheme's canvas; expired, both fall to the card
        // and its faint ink, no opacity involved.
        CelestinaSurface {
            width: parent.width
            height: root.rows.length > 0 && !root.arrived ? code.height + CelestinaTheme.spaceLg * 2 : 0
            visible: height > 0
            role: CelestinaSurface.Grouped

            Rectangle {
                anchors.centerIn: parent
                width: code.width + CelestinaTheme.spaceMd * 2
                height: width
                radius: CelestinaTheme.radiusMd
                color: root.expired ? CelestinaTheme.card : CelestinaTheme.iconSheet

                Item {
                    id: code
                    anchors.centerIn: parent
                    readonly property int modules: root.rows.length
                    readonly property real cell: modules > 0 ? Math.floor(220 / modules) : 0
                    width: cell * modules
                    height: width

                    Repeater {
                        model: root.rows
                        delegate: Item {
                            id: line
                            required property int index
                            required property string modelData
                            y: index * code.cell
                            width: code.width
                            height: code.cell

                            Repeater {
                                model: root.runs(line.modelData)
                                delegate: Rectangle {
                                    required property var modelData
                                    x: modelData.x * code.cell
                                    width: modelData.w * code.cell
                                    height: code.cell
                                    color: root.expired ? CelestinaTheme.textFaint
                                                        : CelestinaTheme.schemeDark.canvas
                                }
                            }
                        }
                    }
                }
            }
        }

        RowLayout {
            width: parent.width
            spacing: CelestinaTheme.spaceSm

            Text {
                Layout.fillWidth: true
                text: root.arrived
                      ? qsTr("%1 ya está vinculado").arg(root.devices.pairingArrived)
                      : root.expired
                        ? qsTr("El código caducó. Genera otro.")
                        : qsTr("Escanéalo con Magnetita en el móvil · %1 s").arg(root.secondsLeft)
                color: root.arrived ? CelestinaTheme.accent : CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                wrapMode: Text.WordWrap
            }

            CelestinaIconButton {
                iconName: "link"
                visible: root.expired
                role: CelestinaButton.Primary
                helpText: qsTr("Generar otro código")
                onClicked: root.devices.startPairing()
            }

            CelestinaIconButton {
                iconName: "x"
                helpText: qsTr("Cerrar")
                onClicked: root.dismissRequested()
            }
        }
    }
}
