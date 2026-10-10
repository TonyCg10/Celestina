pragma ComponentBehavior: Bound
import QtQuick
import org.celestina.selenita 1.0
import "components"

// The capture card (design §5.3): what to take (the screen, a window or a
// region), the output when there are several, the delay, the two
// destinations and the button. The choices live in the controller; this card
// only shows them and asks for changes.
SectionCard {
    id: card

    // Whether the window is on screen: the controller hides it for a capture
    // that would include it.
    property bool windowShown: true
    // Seconds left in the delay under way; 0 when none.
    property int secondsLeft: 0

    function capture() {
        SelenitaController.capture(card.windowShown, String(CelestinaTheme.accent),
                                   String(CelestinaTheme.scrim))
    }

    title: qsTr("Captura")

    Connections {
        target: SelenitaController

        function onCountdown(seconds: int) {
            card.secondsLeft = seconds
        }

        function onBusyChanged() {
            if (!SelenitaController.busy)
                card.secondsLeft = 0
        }
    }

    // The three targets, one wide button each.
    Row {
        id: targets
        x: card.rowInset
        width: parent.width - card.rowInset * 2
        height: CelestinaTheme.controlHeightXl + CelestinaTheme.spaceSm * 2
        spacing: CelestinaTheme.spaceSm

        Repeater {
            model: [
                { kind: "screen", label: qsTr("Pantalla"), name: "targetScreen" },
                { kind: "window", label: qsTr("Ventana"), name: "targetWindow" },
                { kind: "region", label: qsTr("Región"), name: "targetRegion" }
            ]

            delegate: CelestinaButton {
                required property var modelData
                required property int index

                objectName: modelData.name
                // The key that chooses this target (design §5.3).
                Accessible.description: qsTr("Tecla %1").arg(index + 1)
                y: CelestinaTheme.spaceSm
                width: (targets.width - targets.spacing * 2) / 3
                height: CelestinaTheme.controlHeightXl
                density: CelestinaButton.Prominent
                checkable: true
                checked: SelenitaController.target === modelData.kind
                text: modelData.label
                onClicked: {
                    SelenitaController.target = modelData.kind
                    checked = Qt.binding(() => SelenitaController.target === modelData.kind)
                }
            }
        }
    }

    // The output a screen capture takes, when there is more than one.
    Item {
        id: outputRow
        objectName: "outputRow"
        width: parent.width
        height: CelestinaTheme.rowHeight
        visible: SelenitaController.target === "screen" && SelenitaController.outputs.length > 1

        RowDivider { inset: card.rowInset }

        Row {
            x: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            spacing: CelestinaTheme.spaceXs

            Repeater {
                model: [""].concat(SelenitaController.outputs)

                delegate: CelestinaButton {
                    required property string modelData

                    checkable: true
                    checked: SelenitaController.screenOutput === modelData
                    text: modelData.length > 0 ? modelData : qsTr("Todas")
                    helpText: modelData.length > 0 ? qsTr("Pantalla %1").arg(modelData)
                                                   : qsTr("Todas las pantallas")
                    onClicked: {
                        SelenitaController.screenOutput = modelData
                        checked = Qt.binding(() => SelenitaController.screenOutput === modelData)
                    }
                }
            }
        }
    }

    // The delay before the picture is taken.
    Item {
        width: parent.width
        height: CelestinaTheme.rowHeight

        RowDivider { inset: card.rowInset }

        Text {
            anchors.left: parent.left
            anchors.leftMargin: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            text: qsTr("Retraso")
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            spacing: CelestinaTheme.spaceXs

            Repeater {
                model: [0, 3, 5, 10]

                delegate: CelestinaButton {
                    required property int modelData

                    objectName: "delay" + modelData
                    checkable: true
                    checked: SelenitaController.delay === modelData
                    text: modelData === 0 ? qsTr("Ninguno") : qsTr("%1 s").arg(modelData)
                    helpText: modelData === 0 ? qsTr("Sin retraso")
                                              : qsTr("Retraso de %1 segundos").arg(modelData)
                    onClicked: {
                        SelenitaController.delay = modelData
                        checked = Qt.binding(() => SelenitaController.delay === modelData)
                    }
                }
            }
        }
    }

    SettingRow {
        objectName: "clipboardRow"
        inset: card.rowInset
        separated: true
        label: qsTr("Copiar al portapapeles")
        // niri copies a window to the clipboard itself, whatever is asked.
        readonly property bool byNiri: SelenitaController.target === "window"
        enabled: !byNiri
        hint: byNiri ? qsTr("niri copia la ventana al portapapeles") : ""
        checked: byNiri || SelenitaController.toClipboard
        onToggled: on => SelenitaController.toClipboard = on
    }

    SettingRow {
        objectName: "fileRow"
        inset: card.rowInset
        separated: true
        label: qsTr("Guardar en la carpeta %1").arg(SelenitaController.folderName)
        checked: SelenitaController.toFile
        onToggled: on => SelenitaController.toFile = on
    }

    // The button, and the countdown while a delay runs.
    Item {
        width: parent.width
        height: CelestinaTheme.controlHeightLg + CelestinaTheme.spaceSm * 2

        RowDivider { inset: card.rowInset }

        Text {
            objectName: "countdownText"
            anchors.left: parent.left
            anchors.leftMargin: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            visible: card.secondsLeft > 0
            text: qsTr("Captura en %1 s").arg(card.secondsLeft)
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        CelestinaButton {
            objectName: "captureButton"
            anchors.right: parent.right
            anchors.rightMargin: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            height: CelestinaTheme.controlHeightLg
            role: CelestinaButton.Primary
            density: CelestinaButton.Regular
            enabled: !SelenitaController.busy
                     && (SelenitaController.toClipboard || SelenitaController.toFile
                         || SelenitaController.target === "window")
            text: qsTr("Capturar")
            Accessible.description: qsTr("Intro")
            onClicked: card.capture()
        }
    }
}
