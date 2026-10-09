import QtQuick
import org.celestina.cuprita 1.0

// What the pairing agent asks the person, in one of three looks by `kind`:
// `pin` takes a code typed here, `confirm` shows the passkey both sides must
// match, `display` shows the passkey to type on the other device. The answer
// leaves through `answerRequested(kind, value)` — the PIN, `yes`, `no` or
// `cancel` — and the field is emptied at once: BlueZ keeps the bond, Cuprita
// never keeps the code. Escape and a click outside mean `cancel`.
CelestinaModalLayer {
    id: layer

    // What the glass samples behind the card.
    required property Item backdrop
    property string kind: ""
    property string device: ""
    property int passkey: 0

    signal answerRequested(string kind, string value)

    // Six digits, zero-padded, as both devices show it.
    readonly property string passkeyText: ("000000" + layer.passkey).slice(-6)

    function ask(kind, device, passkey) {
        layer.kind = kind
        layer.device = device
        layer.passkey = passkey
        pinField.text = ""
        layer.shown = true
        if (kind === "pin")
            pinField.forceActiveFocus()
        else if (kind === "confirm")
            primary.forceActiveFocus()
        else
            cancel.forceActiveFocus()
    }

    // The agent withdrew the request: close without answering.
    function close() {
        pinField.text = ""
        layer.shown = false
    }

    function answer(value) {
        if (!layer.shown)
            return
        layer.answerRequested(layer.kind, value)
        layer.close()
    }

    z: 90
    onDismissRequested: layer.answer("cancel")

    GlassCard {
        id: card
        objectName: "pairingCard"
        anchors.centerIn: parent
        width: Math.min(CelestinaTheme.compMenuWidth + 2 * CelestinaTheme.space3xl,
                        layer.width - CelestinaTheme.space3xl)
        height: buttons.y + buttons.height + CelestinaTheme.spaceLg
        backdropSource: layer.backdrop

        Accessible.role: Accessible.Dialog
        Accessible.name: heading.text

        // A click on the card is not a click outside it.
        MouseArea { anchors.fill: parent }

        Text {
            id: heading
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: CelestinaTheme.spaceLg
            wrapMode: Text.WordWrap
            text: qsTr("Emparejar con «%1»").arg(layer.device)
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        Text {
            id: hint
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: heading.bottom
            anchors.leftMargin: CelestinaTheme.spaceLg
            anchors.rightMargin: CelestinaTheme.spaceLg
            anchors.topMargin: CelestinaTheme.spaceSm
            wrapMode: Text.WordWrap
            text: layer.kind === "pin" ? qsTr("Escribe el código que pide el dispositivo.")
                  : layer.kind === "confirm" ? qsTr("Comprueba que el dispositivo muestra este código.")
                  : qsTr("Escribe este código en el dispositivo.")
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }

        CelestinaTextField {
            id: pinField
            objectName: "pinField"
            visible: layer.kind === "pin"
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: hint.bottom
            anchors.leftMargin: CelestinaTheme.spaceLg
            anchors.rightMargin: CelestinaTheme.spaceLg
            anchors.topMargin: CelestinaTheme.spaceMd
            placeholderText: qsTr("Código")
            // BlueZ's `RequestPinCode` asks for a text PIN (most devices
            // expect digits, some accept letters), so the hint does not
            // restrict it; it stays out of input-method history.
            inputMethodHints: Qt.ImhSensitiveData | Qt.ImhNoPredictiveText
            Accessible.name: qsTr("Código")
            onAccepted: {
                if (pinField.text.length > 0)
                    layer.answer(pinField.text)
            }
        }

        Text {
            id: passkeyLabel
            objectName: "passkeyText"
            visible: layer.kind !== "pin"
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: hint.bottom
            anchors.topMargin: CelestinaTheme.spaceMd
            text: layer.passkeyText
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontDisplay
            font.weight: CelestinaTheme.weightDemiBold
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Código %1").arg(layer.passkeyText)
        }

        Row {
            id: buttons
            anchors.right: parent.right
            anchors.rightMargin: CelestinaTheme.spaceLg
            anchors.top: layer.kind === "pin" ? pinField.bottom : passkeyLabel.bottom
            anchors.topMargin: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceSm

            CelestinaButton {
                objectName: "rejectButton"
                visible: layer.kind === "confirm"
                text: qsTr("No coincide")
                onClicked: layer.answer("no")
            }

            CelestinaButton {
                id: cancel
                objectName: "cancelButton"
                visible: layer.kind !== "confirm"
                text: qsTr("Cancelar")
                onClicked: layer.answer("cancel")
            }

            CelestinaButton {
                id: primary
                objectName: "confirmButton"
                visible: layer.kind !== "display"
                text: layer.kind === "confirm" ? qsTr("Coincide") : qsTr("Aceptar")
                role: CelestinaButton.Primary
                enabled: layer.kind !== "pin" || pinField.text.length > 0
                onClicked: layer.answer(layer.kind === "confirm" ? "yes" : pinField.text)
            }
        }
    }
}
