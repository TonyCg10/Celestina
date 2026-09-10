// The delegates below reach the page's `root` id and take `index` and
// `modelData` from their models, which needs bound component behaviour.
pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.magnetita 1.0

// Scrolls like the devices page: a few paired phones and the plugin card
// already run past the window's minimum height, and rows past the bottom
// edge were rows nothing could click.
ScrollPage {
    id: root

    required property DevicesModel devices
    required property CommandsModel commands

    spacing: 16

    Text {
        width: parent.width
        visible: !root.devices.settingsAvailable
        text: "El servicio Magnetita no está disponible."
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        wrapMode: Text.Wrap
    }

    ListSection {
        width: parent.width
        visible: root.devices.settingsAvailable
        title: "DISPOSITIVOS EMPAREJADOS"

        Text {
            width: parent.width
            leftPadding: CelestinaTheme.spaceLg
            rightPadding: CelestinaTheme.spaceLg
            topPadding: CelestinaTheme.spaceSm
            bottomPadding: CelestinaTheme.spaceSm
            visible: root.devices.pairedNames.length === 0
            text: "Ningún dispositivo emparejado todavía."
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.Wrap
        }

        Repeater {
            model: root.devices.pairedNames

            delegate: PairedDeviceRow {
                required property int index
                required property string modelData

                width: root.width
                deviceName: modelData
                fingerprint: index < root.devices.pairedFingerprints.length
                             ? root.devices.pairedFingerprints[index] : ""
                online: index < root.devices.pairedConnected.length
                        && root.devices.pairedConnected[index] === "true"
                onForgetRequested: root.devices.forgetPaired(index)
            }
        }
    }

    // The commands the phone may run: registered here by name, program and
    // arguments; the phone only ever sees ids and names.
    ListSection {
        width: parent.width
        visible: root.devices.settingsAvailable
        title: "\u00d3RDENES DESDE EL M\u00d3VIL"

        Text {
            width: parent.width
            leftPadding: CelestinaTheme.spaceLg
            rightPadding: CelestinaTheme.spaceLg
            topPadding: CelestinaTheme.spaceSm
            bottomPadding: CelestinaTheme.spaceSm
            visible: root.commands.commandIds.length === 0
            text: "Ninguna orden todav\u00eda. El m\u00f3vil solo ve el nombre; el programa y sus argumentos se quedan aqu\u00ed."
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.Wrap
        }

        Repeater {
            model: root.commands.commandIds

            delegate: CommandRow {
                required property int index
                required property string modelData

                width: root.width
                name: index < root.commands.commandNames.length ? root.commands.commandNames[index] : modelData
                line: index < root.commands.commandLines.length ? root.commands.commandLines[index] : ""
                onRemoveRequested: root.commands.remove(index)
            }
        }

        Row {
            width: parent.width
            leftPadding: CelestinaTheme.spaceLg
            rightPadding: CelestinaTheme.spaceSm
            topPadding: CelestinaTheme.spaceSm
            bottomPadding: CelestinaTheme.spaceSm
            spacing: CelestinaTheme.spaceSm

            CelestinaTextField {
                id: newName
                width: (parent.width - parent.leftPadding - parent.rightPadding - parent.spacing * 3 - addButton.width) * 0.3
                placeholderText: qsTr("Nombre")
            }

            CelestinaTextField {
                id: newProgram
                width: (parent.width - parent.leftPadding - parent.rightPadding - parent.spacing * 3 - addButton.width) * 0.35
                placeholderText: qsTr("Programa")
            }

            CelestinaTextField {
                id: newArgs
                width: (parent.width - parent.leftPadding - parent.rightPadding - parent.spacing * 3 - addButton.width) * 0.35
                placeholderText: qsTr("Argumentos")
                onAccepted: addButton.clicked()
            }

            CelestinaIconButton {
                id: addButton
                iconName: "plus"
                role: CelestinaButton.Primary
                enabled: newName.text.trim().length > 0 && newProgram.text.trim().length > 0
                helpText: qsTr("Registrar la orden")
                onClicked: {
                    root.commands.add(newName.text, newProgram.text, newArgs.text)
                    newName.text = ""
                    newProgram.text = ""
                    newArgs.text = ""
                }
            }
        }
    }

    ListSection {
        width: parent.width
        visible: root.devices.settingsAvailable
        title: "PLUGINS"

        Repeater {
            model: root.devices.pluginLabels

            delegate: PluginRow {
                required property int index
                required property string modelData

                width: root.width
                label: modelData
                enabledFlag: index < root.devices.pluginEnabled.length
                             && root.devices.pluginEnabled[index] === "true"
                onToggleRequested: root.devices.togglePlugin(index)
            }
        }
    }
}
