import QtQuick
import org.celestina.cuprita 1.0

// Asks for the passphrase of a protected Wi-Fi network that has no saved
// profile. The passphrase leaves through `connectRequested` and the field is
// emptied at once: Cuprita never keeps it; NetworkManager stores the profile.
// Escape, a click outside and «Cancelar» all mean "do not join".
CelestinaModalLayer {
    id: layer

    // What the glass samples behind the card.
    required property Item backdrop
    property string networkId: ""
    property string networkName: ""
    // Set by the first «Conectar»: a second press sends nothing.
    property bool submitted: false

    signal connectRequested(string id, string password)

    function ask(id, name) {
        layer.networkId = id
        layer.networkName = name
        passwordField.text = ""
        reveal.shown = false
        layer.submitted = false
        layer.shown = true
        passwordField.forceActiveFocus()
    }

    function close() {
        passwordField.text = ""
        layer.shown = false
    }

    function confirm() {
        if (passwordField.text.length === 0 || layer.submitted)
            return
        layer.submitted = true
        layer.connectRequested(layer.networkId, passwordField.text)
        layer.close()
    }

    z: 90
    onDismissRequested: layer.close()

    GlassCard {
        id: card
        objectName: "wifiPasswordCard"
        anchors.centerIn: parent
        // A menu's width with a wide margin each side, never wider than the
        // window allows.
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
            text: qsTr("Contraseña de «%1»").arg(layer.networkName)
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        CelestinaTextField {
            id: passwordField
            objectName: "passwordField"
            anchors.left: parent.left
            anchors.right: reveal.left
            anchors.top: heading.bottom
            anchors.leftMargin: CelestinaTheme.spaceLg
            anchors.rightMargin: CelestinaTheme.spaceXs
            anchors.topMargin: CelestinaTheme.spaceMd
            echoMode: reveal.shown ? TextInput.Normal : TextInput.Password
            placeholderText: qsTr("Contraseña")
            Accessible.name: qsTr("Contraseña")
            onAccepted: layer.confirm()
        }

        CelestinaIconButton {
            id: reveal
            objectName: "revealButton"
            property bool shown: false
            anchors.right: parent.right
            anchors.rightMargin: CelestinaTheme.spaceLg
            anchors.verticalCenter: passwordField.verticalCenter
            role: CelestinaButton.Ghost
            iconName: reveal.shown ? "eye-off" : "eye"
            helpText: reveal.shown ? qsTr("Ocultar contraseña") : qsTr("Mostrar contraseña")
            onClicked: reveal.shown = !reveal.shown
        }

        Row {
            id: buttons
            anchors.right: parent.right
            anchors.rightMargin: CelestinaTheme.spaceLg
            anchors.top: passwordField.bottom
            anchors.topMargin: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceSm

            CelestinaButton {
                objectName: "cancelButton"
                text: qsTr("Cancelar")
                onClicked: layer.close()
            }

            CelestinaButton {
                objectName: "confirmButton"
                text: qsTr("Conectar")
                role: CelestinaButton.Primary
                enabled: passwordField.text.length > 0 && !layer.submitted
                onClicked: layer.confirm()
            }
        }
    }
}
