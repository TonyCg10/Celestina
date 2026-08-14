// One output's cover, while the session is locked.
//
// R6-B builds the protocol lifecycle underneath this; what is here is the
// minimum a person needs to get back in — the prompt, and what happened to
// their last attempt. R6-C gives it the shell's own material and its clock.
//
// It shows nothing about the session it is covering. ADR 0004 is explicit
// about that and it is worth restating where the temptation lives: no
// notification bodies, no media titles, no clipboard, no window list. A lock
// screen that renders someone's messages has already failed at the one thing
// it exists for.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: cover

    // The compositor sizes a lock surface and this window follows; nothing
    // here decides its geometry.
    color: CelestinaTheme.canvas
    visible: false

    // What the last attempt came back as. The wording lives here, in QML,
    // because the authenticator reports a verdict and never a sentence.
    property int lastVerdict: -1
    readonly property bool checking: lockAuthenticator.busy

    readonly property string message: {
        if (cover.checking)
            return qsTr("Comprobando…");
        if (cover.lastVerdict === LockAuthenticator.Refused)
            return qsTr("Contraseña incorrecta.");
        if (cover.lastVerdict === LockAuthenticator.Unavailable)
            return qsTr("No se pudo comprobar la contraseña.");
        return "";
    }

    Connections {
        target: lockAuthenticator

        function onAnswered(verdict) {
            cover.lastVerdict = verdict;
            // The field is cleared whatever the answer: a refused passphrase
            // left on screen is one somebody can read over a shoulder.
            field.clear();
            if (verdict !== LockAuthenticator.Authenticated)
                field.forceActiveFocus();
        }
    }

    Column {
        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceLg
        width: Math.min(parent.width * 0.4, 420)

        CelestinaTextField {
            id: field

            objectName: "celestina-lock-passphrase"
            width: parent.width
            echoMode: TextInput.Password
            enabled: !cover.checking
            placeholderText: qsTr("Contraseña")
            onAccepted: {
                if (text.length > 0)
                    lockAuthenticator.authenticate(text);
            }
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: cover.message
            color: cover.lastVerdict === LockAuthenticator.Refused
                   ? CelestinaTheme.danger : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            wrapMode: Text.WordWrap
        }
    }

    Component.onCompleted: field.forceActiveFocus()
}
