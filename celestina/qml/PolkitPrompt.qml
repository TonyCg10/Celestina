// What this session asks before it lets something act as another user.
//
// Everything named here came from polkitd: the message, the action's own id
// and the account being asked about. None of it is rewritten, because a shell
// that paraphrased what is being authorized would be deciding for the person
// what they are agreeing to. The only sentences this file writes are the ones
// about the prompt itself.
//
// The action id is shown deliberately, in full and in a quiet weight. It is
// the one string an attacker cannot dress up: a message can claim anything,
// and `org.freedesktop.policykit.exec` is what is actually being asked for.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: prompt

    required property var promptSource
    required property bool reducedMotion
    // polkitd's own strings, shown and never edited.
    required property string actionId
    required property string message
    required property string iconName
    required property string identity

    // What PAM asked for, and what it said went wrong. Both arrive after the
    // surface is up, so both are plain properties rather than construction
    // arguments.
    property string prompt: ""
    property string problem: ""
    property string notice: ""

    property real shellScale: 1.0
    readonly property real surfaceWidth: prompt.width / prompt.shellScale
    readonly property real surfaceHeight: prompt.height / prompt.shellScale

    readonly property int cardWidth: 420
    readonly property int cardHeight: contentColumn.implicitHeight
                                      + CelestinaTheme.spaceMd * 2

    color: CelestinaTheme.clear
    title: qsTr("Autorización")

    BackdropInk {
        id: backdropInk
    }

    Component.onCompleted: {
        prompt.width = Math.round(prompt.cardWidth * prompt.shellScale);
        prompt.height = Math.round(prompt.cardHeight * prompt.shellScale);
        CelestinaTheme.reducedMotion = prompt.reducedMotion;
        field.forceActiveFocus();
    }

    onVisibleChanged: {
        if (visible)
            Qt.callLater(card.reveal);
    }

    function answer() {
        if (field.text.length === 0)
            return;
        prompt.promptSource.respond(field.text);
        field.clear();
    }

    Item {
        id: shellScene
        objectName: "celestina-shell-scene"

        width: prompt.surfaceWidth
        height: prompt.surfaceHeight
        transformOrigin: Item.TopLeft
        scale: prompt.shellScale
    }

    // A click outside the card dismisses the request. Dismissing is a refusal
    // to answer, not a wrong answer: nothing is authorized and polkitd is told
    // the person cancelled.
    MouseArea {
        parent: shellScene
        z: -1
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onPressed: prompt.promptSource.dismiss()
    }

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        onActivated: prompt.promptSource.dismiss()
    }

    SoftOverlayCard {
        parent: shellScene
        id: card

        ink: backdropInk
        width: prompt.cardWidth
        height: prompt.cardHeight
        x: Math.round((prompt.surfaceWidth - width) / 2)
        y: Math.round((prompt.surfaceHeight - height) / 2)
        reducedMotion: prompt.reducedMotion
        accessibleName: qsTr("Autorización")
        surfacePosition: Qt.point(card.x, card.y)

        Column {
            id: contentColumn

            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceMd
            spacing: CelestinaTheme.spaceSm

            MenuHeader {
                width: parent.width
                ink: backdropInk
                title: qsTr("Autorización requerida")
                subtitle: prompt.identity
                iconName: "shield"
                fallbackIcon: "lock"
            }

            MenuSection {
                width: parent.width
                ink: backdropInk
                height: detail.implicitHeight + CelestinaTheme.spaceMd * 2

                Column {
                    id: detail

                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceMd
                    spacing: CelestinaTheme.spaceXs

                    Text {
                        width: parent.width
                        text: prompt.message
                        color: backdropInk.primary
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        wrapMode: Text.WordWrap
                    }

                    Text {
                        width: parent.width
                        visible: prompt.actionId.length > 0
                        text: prompt.actionId
                        color: backdropInk.muted
                        font.family: CelestinaTheme.monoFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        elide: Text.ElideMiddle
                    }
                }
            }

            MenuSection {
                width: parent.width
                ink: backdropInk
                height: entry.implicitHeight + CelestinaTheme.spaceMd * 2

                Column {
                    id: entry

                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceMd
                    spacing: CelestinaTheme.spaceSm

                    BackdropTextField {
                        id: field

                        width: parent.width
                        ink: backdropInk
                        echoMode: TextInput.Password
                        // PAM's own wording when it has arrived, and this
                        // shell's only until it does.
                        placeholderText: prompt.prompt.length > 0
                                         ? prompt.prompt
                                         : qsTr("Contraseña")
                        Accessible.name: qsTr("Contraseña")
                        onAccepted: prompt.answer()
                    }

                    Text {
                        width: parent.width
                        visible: prompt.problem.length > 0
                                 || prompt.notice.length > 0
                        text: prompt.problem.length > 0 ? prompt.problem
                                                        : prompt.notice
                        color: prompt.problem.length > 0 ? CelestinaTheme.danger
                                                         : backdropInk.muted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        wrapMode: Text.WordWrap
                    }

                    Text {
                        width: parent.width
                        text: qsTr("Intro para continuar, Esc para cancelar.")
                        color: backdropInk.muted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                    }
                }
            }
        }
    }
}
