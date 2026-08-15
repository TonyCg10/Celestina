// One output's cover, while the session is locked.
//
// What it shows is the whole of what a locked screen is allowed to know: the
// time, a place to type, and what happened to the last attempt. ADR 0004 is
// explicit about the rest and it is worth restating where the temptation
// lives — no notification bodies, no media titles, no clipboard, no window
// list. A lock screen that renders someone's messages has already failed at
// the one thing it exists for.
//
// It is opaque on purpose. `ext-session-lock` means the compositor has stopped
// showing the session, so there is nothing behind this to see through: a
// transparent material here would be glass over a void, and the shell's dense
// content surface is the right material precisely because it is the one that
// does not depend on a backdrop.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: cover

    // The compositor sizes a lock surface; nothing here decides its geometry.
    color: CelestinaTheme.canvas
    visible: false

    // What the last attempt came back as, as a `LockAuthenticator.Verdict`.
    // Negative until something has been tried.
    property int lastVerdict: -1
    readonly property bool checking: lockAuthenticator.busy

    // The wording lives here, in QML, because the authenticator reports a
    // verdict and never a sentence. A refusal and an unavailable verifier are
    // deliberately different words: one means "that was not the passphrase",
    // the other means "this machine could not ask", and telling a person to
    // retype in the second case would be a lie.
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
            // Cleared whatever the answer: a passphrase left on screen is one
            // somebody can read over a shoulder, and a correct one is about to
            // be irrelevant anyway.
            field.clear();
            if (verdict !== LockAuthenticator.Authenticated)
                field.forceActiveFocus();
        }
    }

    // The lock's language is Spanish by construction, exactly as the panel's
    // is, so the weekday and month are asked for in it rather than inherited
    // from whatever locale this process started with — a lock spawned from a
    // C-locale service otherwise renders "Friday 14 de August", which is what
    // it did before this line existed.
    readonly property var uiLocale: Qt.locale("es_ES")

    // The clock. It ticks to the minute, and only while this is on screen:
    // a locked machine should not be waking for a second hand nobody asked
    // for.
    QtObject {
        id: now

        property date value: new Date()
    }

    Timer {
        interval: 1000
        running: cover.visible
        repeat: true
        onTriggered: now.value = new Date()
    }

    Column {
        anchors.centerIn: parent
        spacing: CelestinaTheme.space3xl
        width: Math.min(parent.width * 0.4, 420)

        Column {
            width: parent.width
            spacing: CelestinaTheme.spaceXs

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: now.value.toLocaleTimeString(cover.uiLocale, "HH:mm")
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.features: CelestinaTheme.fontFeaturesTabular
                font.pixelSize: CelestinaTheme.fontDisplay * 2
                font.weight: CelestinaTheme.weightDemiBold
            }

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: now.value.toLocaleDateString(cover.uiLocale,
                                                   "dddd d 'de' MMMM")
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
            }
        }

        // The prompt, on the shell's own dense material rather than floating
        // loose on the canvas — the same anatomy every card in this shell has.
        GlassSurface {
            width: parent.width
            implicitHeight: prompt.implicitHeight + CelestinaTheme.space2xl * 2
            height: implicitHeight
            cornerRadius: CelestinaTheme.radiusLg
            // No compositor sample is possible here and none is wanted: the
            // surface uses its own readable fill, which is what this mode
            // falls back to.
            backdropMode: GlassSurface.ExternalBackdrop
            externalBackdropReady: false
            captureEnabled: false
            materialRole: GlassSurface.ContentSurface

            Column {
                id: prompt

                anchors.centerIn: parent
                width: parent.width - CelestinaTheme.space2xl * 2
                spacing: CelestinaTheme.spaceMd

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
                    visible: cover.message.length > 0
                    text: cover.message
                    color: cover.lastVerdict === LockAuthenticator.Refused
                           ? CelestinaTheme.danger : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    wrapMode: Text.WordWrap
                }
            }
        }
    }

    Component.onCompleted: field.forceActiveFocus()
}
