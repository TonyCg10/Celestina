pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── The password of an encrypted archive ──────────────────────────
    // An extraction stops when the archive is protected, and this asks the one
    // thing the domain cannot work out for itself. The password reaches the
    // controller only as the answer is given: it is not stored, not carried
    // from one archive to the next, and the field is emptied on every close.
    //
    // The rest of the batch is still queued behind this question, so skipping
    // cancels nothing else — it passes over this archive and carries on.
CelestinaModalLayer {
    id: password
    property var controller
    property var owner
    property var backdrop   // mainPanel: the surface the glass samples
    anchors.fill: parent
    z: 63
    shown: password.controller.passwordPending
    onDismissRequested: password.skip()

    function submit() {
        const value = field.text
        if (value.length === 0)
            return
        field.text = ""
        password.controller.answerPassword(value)
        password.owner.focusView()
    }
    function skip() {
        field.text = ""
        password.controller.cancelPassword()
        password.owner.focusView()
    }

    // The field takes focus every time the question is put, including the
    // repeat after a wrong password.
    onShownChanged: {
        if (password.shown) {
            field.text = ""
            field.forceActiveFocus()
        }
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(420, password.owner.width - 48)
        height: 196
        backdropSource: password.backdrop
        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Archivo protegido con contraseña")

        // Swallow clicks so they never reach the dismiss backdrop.
        MouseArea { anchors.fill: parent }

        Text {
            id: heading
            x: 18
            y: 16
            text: password.controller.passwordRetry
                  ? qsTr("Contraseña incorrecta")
                  : qsTr("Archivo protegido")
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        Text {
            id: body
            x: 18
            y: heading.y + heading.height + 10
            width: parent.width - 36
            wrapMode: Text.Wrap
            text: password.controller.passwordRetry
                  ? qsTr("«%1» no se abre con esa contraseña. Inténtalo otra vez.")
                    .arg(password.controller.passwordArchive)
                  : qsTr("«%1» necesita una contraseña para extraerse.")
                    .arg(password.controller.passwordArchive)
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
        }

        CelestinaTextField {
            id: field
            x: 18
            y: body.y + body.height + 12
            width: parent.width - 36
            echoMode: TextInput.Password
            placeholderText: qsTr("Contraseña")
            Accessible.name: qsTr("Contraseña del archivo")
            onAccepted: password.submit()
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16
            spacing: 8

            CelestinaButton {
                text: qsTr("Omitir")
                onClicked: password.skip()
            }
            CelestinaButton {
                text: qsTr("Extraer")
                role: CelestinaButton.Primary
                enabled: field.text.length > 0
                onClicked: password.submit()
            }
        }
    }
}
